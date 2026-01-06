# Themes

This document explains SpiritStream themes and how to create custom ones.

## Overview

- Themes are JSON or JSONC files (JSON with comments).
- Each theme must provide **all tokens** for both `light` and `dark`.
- The built-in theme is **Spirit** and lives in `src-frontend/styles/tokens.css`.
- Custom themes are loaded from the app data themes folder and reloaded dynamically.

## Theme File Format

```jsonc
{
  "id": "sunset",
  "name": "Sunset",
  "tokens": {
    "light": {
      "--bg-base": "#FAF8F5",
      "--primary": "#FF6A3D"
      // ...all required tokens
    },
    "dark": {
      "--bg-base": "#0F0A14",
      "--primary": "#FF8A65"
      // ...all required tokens
    }
  }
}
```

Notes:
- `id` must be lowercase alphanumeric with `-` or `_` only.
- `id` **must not** be `spirit` (reserved).
- JSONC supports `//` and `/* */` comments.
- Trailing commas are not supported.

## Required Tokens

The required token list is derived from `src-frontend/styles/tokens.css`.
All tokens in that file must appear in both `tokens.light` and `tokens.dark`.

## Where Themes Live

Custom themes are stored in the app data directory:

- Windows: `%APPDATA%\\com.spiritstream.desktop\\themes`
- macOS: `~/Library/Application Support/com.spiritstream.desktop/themes`
- Linux: `~/.local/share/com.spiritstream.desktop/themes`

The app watches this folder and reloads themes automatically.

## Installing a Theme

1. Create a `.json` or `.jsonc` file.
2. Open **Settings > Themes**.
3. Click **Install Theme** and select the file.

If validation fails, the theme is rejected and not copied.

## Reference and Template

- `themes/spirit.jsonc` is a reference export of the built-in Spirit tokens.
  - It cannot be installed as-is because the `id` is reserved.
- `themes/theme-template.jsonc` is a template with placeholders.
  - Replace every `REPLACE_ME` value before installing.
