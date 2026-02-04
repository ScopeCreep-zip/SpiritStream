---
name: Secure File Upload
description: Use when implementing file upload functionality, image processing, or document handling. Covers validation, storage, and serving files safely.
allowed-tools:
  - Read
  - Grep
  - Glob
  - Edit
  - Write
---

# Secure File Upload Guide

Apply these practices when handling file uploads.

## Validation Requirements

### 1. Extension Validation (Whitelist)

```python
ALLOWED_EXTENSIONS = {'.jpg', '.jpeg', '.png', '.gif', '.pdf'}

def is_allowed_extension(filename):
    ext = os.path.splitext(filename)[1].lower()
    return ext in ALLOWED_EXTENSIONS
```

### 2. Magic Bytes Validation

| Type | Magic Bytes (hex) |
|------|-------------------|
| JPEG | `FF D8 FF` |
| PNG | `89 50 4E 47 0D 0A 1A 0A` |
| GIF | `47 49 46 38` |
| PDF | `25 50 44 46` |
| ZIP/DOCX/XLSX | `50 4B 03 04` |
| WebP | `52 49 46 46 ... 57 45 42 50` |

```python
MAGIC_BYTES = {
    '.jpg': [b'\xFF\xD8\xFF'],
    '.jpeg': [b'\xFF\xD8\xFF'],
    '.png': [b'\x89PNG\r\n\x1a\n'],
    '.gif': [b'GIF87a', b'GIF89a'],
    '.pdf': [b'%PDF'],
    '.webp': [b'RIFF'],
}

def validate_magic_bytes(file_content, extension):
    valid_signatures = MAGIC_BYTES.get(extension.lower(), [])
    for sig in valid_signatures:
        if file_content.startswith(sig):
            return True
    return False
```

### 3. Content Validation

```python
from PIL import Image

def validate_image(file_path):
    try:
        with Image.open(file_path) as img:
            img.verify()  # Verify it's a valid image

        # Re-open to check dimensions
        with Image.open(file_path) as img:
            width, height = img.size
            if width > 10000 or height > 10000:
                return False, "Image too large"
            if width * height > 100_000_000:  # 100MP limit
                return False, "Image resolution too high"

        return True, None
    except Exception as e:
        return False, f"Invalid image: {e}"
```

### 4. Size Limits

```python
MAX_FILE_SIZE = 10 * 1024 * 1024  # 10MB

def validate_size(file):
    file.seek(0, 2)  # Seek to end
    size = file.tell()
    file.seek(0)  # Reset

    if size > MAX_FILE_SIZE:
        raise ValidationError(f"File too large. Max: {MAX_FILE_SIZE} bytes")
```

## Common Attack Vectors

### Extension Bypass

| Attack | Example | Prevention |
|--------|---------|------------|
| Double extension | `shell.php.jpg` | Check final extension after stripping |
| Null byte | `shell.php%00.jpg` | Remove null bytes |
| Case variation | `shell.PHP` | Lowercase before checking |
| Trailing spaces | `shell.php ` | Strip whitespace |
| Unicode confusion | `shell.ⓟⓗⓟ` | Normalize unicode |

```python
def safe_extension(filename):
    # Remove null bytes
    filename = filename.replace('\x00', '')
    # Strip whitespace
    filename = filename.strip()
    # Normalize unicode
    filename = unicodedata.normalize('NFKC', filename)
    # Get extension
    ext = os.path.splitext(filename)[1].lower()
    return ext
```

### Polyglot Files

Files valid as multiple types (JPEG that's also JavaScript):

```python
def strict_image_validation(file_path):
    # Re-encode the image to strip any embedded content
    with Image.open(file_path) as img:
        # Convert to RGB (removes alpha channel tricks)
        if img.mode in ('RGBA', 'P'):
            img = img.convert('RGB')

        # Save to new file (strips metadata and embedded content)
        clean_path = file_path + '.clean'
        img.save(clean_path, 'JPEG', quality=95)

    os.replace(clean_path, file_path)
```

### SVG XSS

SVG files can contain JavaScript:

```xml
<svg xmlns="http://www.w3.org/2000/svg" onload="alert('XSS')">
  <script>alert('XSS')</script>
</svg>
```

**Prevention:**
```python
# Option 1: Don't allow SVG
if extension == '.svg':
    raise ValidationError("SVG not allowed")

# Option 2: Sanitize SVG
import defusedxml.ElementTree as ET

def sanitize_svg(svg_content):
    # Parse without executing
    tree = ET.fromstring(svg_content)

    # Remove dangerous elements and attributes
    dangerous_tags = {'script', 'foreignObject', 'use'}
    dangerous_attrs = {'onload', 'onclick', 'onerror', 'onmouseover'}

    for elem in tree.iter():
        if elem.tag.split('}')[-1].lower() in dangerous_tags:
            elem.getparent().remove(elem)
        for attr in list(elem.attrib):
            if attr.lower() in dangerous_attrs or attr.lower().startswith('on'):
                del elem.attrib[attr]

    return ET.tostring(tree, encoding='unicode')
```

### ZIP Slip

Malicious ZIP files with path traversal:

```python
import zipfile

def safe_extract(zip_path, dest_dir):
    with zipfile.ZipFile(zip_path) as zf:
        for member in zf.namelist():
            # Resolve the full path
            member_path = os.path.realpath(os.path.join(dest_dir, member))
            dest_path = os.path.realpath(dest_dir)

            # Ensure it's under destination
            if not member_path.startswith(dest_path + os.sep):
                raise SecurityError(f"Path traversal in ZIP: {member}")

        zf.extractall(dest_dir)
```

### XML in Office Documents

DOCX/XLSX contain XML that can have XXE:

```python
def process_office_doc(file_path):
    import zipfile
    import defusedxml.ElementTree as ET

    with zipfile.ZipFile(file_path) as zf:
        for name in zf.namelist():
            if name.endswith('.xml'):
                with zf.open(name) as f:
                    # Parse with defusedxml
                    tree = ET.parse(f)
```

## Secure Storage

### Rename Files

```python
import uuid

def generate_safe_filename(original_name):
    ext = safe_extension(original_name)
    if ext not in ALLOWED_EXTENSIONS:
        raise ValidationError("Invalid extension")

    # Use UUID for filename
    return f"{uuid.uuid4()}{ext}"
```

### Store Outside Webroot

```
/var/www/app/          # Application root
/var/www/app/public/   # Webroot (served by nginx)
/var/uploads/          # File storage (NOT under webroot)
```

### Directory Structure

```python
import hashlib

def get_storage_path(filename):
    # Use hash prefix for distribution
    hash_prefix = hashlib.sha256(filename.encode()).hexdigest()[:4]

    # Creates structure like: /uploads/ab/cd/filename.jpg
    return os.path.join(
        UPLOAD_DIR,
        hash_prefix[:2],
        hash_prefix[2:4],
        filename
    )
```

## Serving Files Securely

### Response Headers

```python
from flask import send_file

@app.route('/files/<file_id>')
def serve_file(file_id):
    file_path = get_file_path(file_id)

    response = send_file(file_path)

    # Force download (don't render in browser)
    response.headers['Content-Disposition'] = f'attachment; filename="{safe_filename}"'

    # Prevent MIME sniffing
    response.headers['X-Content-Type-Options'] = 'nosniff'

    # Set correct content type
    response.headers['Content-Type'] = get_mime_type(file_path)

    return response
```

### Separate Domain for User Content

```
Main app: https://app.example.com
User files: https://cdn.example-content.com
```

This prevents:
- Cookie theft from user-uploaded JavaScript
- Same-origin attacks

### Access Control

```python
@app.route('/files/<file_id>')
@require_auth
def serve_file(file_id):
    file_record = db.files.get(file_id)

    # Verify ownership
    if file_record.owner_id != current_user.id:
        abort(404)

    return send_file(file_record.path)
```

## Complete Upload Handler

```python
import os
import uuid
import hashlib
from PIL import Image
from werkzeug.utils import secure_filename

class SecureUploader:
    ALLOWED_EXTENSIONS = {'.jpg', '.jpeg', '.png', '.gif', '.pdf'}
    MAX_SIZE = 10 * 1024 * 1024  # 10MB
    MAGIC_BYTES = {
        '.jpg': [b'\xFF\xD8\xFF'],
        '.jpeg': [b'\xFF\xD8\xFF'],
        '.png': [b'\x89PNG\r\n\x1a\n'],
        '.gif': [b'GIF87a', b'GIF89a'],
        '.pdf': [b'%PDF'],
    }

    def __init__(self, upload_dir):
        self.upload_dir = upload_dir

    def upload(self, file, owner_id):
        # 1. Check size
        file.seek(0, 2)
        if file.tell() > self.MAX_SIZE:
            raise ValidationError("File too large")
        file.seek(0)

        # 2. Get and validate extension
        original_name = secure_filename(file.filename)
        ext = os.path.splitext(original_name)[1].lower()
        if ext not in self.ALLOWED_EXTENSIONS:
            raise ValidationError("File type not allowed")

        # 3. Validate magic bytes
        content = file.read(16)
        file.seek(0)
        if not self._validate_magic(content, ext):
            raise ValidationError("File content doesn't match extension")

        # 4. Generate safe filename
        new_filename = f"{uuid.uuid4()}{ext}"

        # 5. Determine storage path
        file_path = self._get_storage_path(new_filename)
        os.makedirs(os.path.dirname(file_path), exist_ok=True)

        # 6. Save file
        file.save(file_path)

        # 7. Additional validation for images
        if ext in {'.jpg', '.jpeg', '.png', '.gif'}:
            if not self._validate_image(file_path):
                os.remove(file_path)
                raise ValidationError("Invalid image")

        # 8. Store record
        return self._create_record(new_filename, original_name, owner_id)

    def _validate_magic(self, content, ext):
        signatures = self.MAGIC_BYTES.get(ext, [])
        return any(content.startswith(sig) for sig in signatures)

    def _validate_image(self, path):
        try:
            with Image.open(path) as img:
                img.verify()
            return True
        except:
            return False

    def _get_storage_path(self, filename):
        hash_prefix = hashlib.sha256(filename.encode()).hexdigest()[:4]
        return os.path.join(
            self.upload_dir,
            hash_prefix[:2],
            hash_prefix[2:4],
            filename
        )

    def _create_record(self, filename, original, owner_id):
        return {
            'id': str(uuid.uuid4()),
            'filename': filename,
            'original_name': original,
            'owner_id': owner_id
        }
```

## Checklist

- [ ] Extension validated against whitelist
- [ ] Magic bytes verified
- [ ] File size limited
- [ ] Images parsed and verified
- [ ] Files renamed to random UUIDs
- [ ] Stored outside webroot
- [ ] Content-Disposition header set
- [ ] X-Content-Type-Options: nosniff set
- [ ] Access control enforced on download
- [ ] SVG sanitized or blocked
- [ ] ZIP extraction validates paths
- [ ] Office documents parsed with safe XML parser
