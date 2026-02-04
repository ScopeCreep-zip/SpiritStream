---
name: Secure Access Control
description: Use when implementing authorization, role-based access, resource permissions, or multi-tenant data access. Prevents IDOR and privilege escalation.
allowed-tools:
  - Read
  - Grep
  - Glob
  - Edit
  - Write
---

# Secure Access Control Guide

Apply these practices when implementing authorization and resource access.

## Core Principles

1. **Verify ownership on every request** - Never trust client-side data
2. **Check at the data layer** - Not just route/middleware level
3. **Fail closed** - Deny access by default
4. **Use non-guessable IDs** - UUIDs over sequential integers

## Authorization Check Pattern

```python
# VULNERABLE - trusts user input
def get_document(doc_id):
    return db.documents.find(doc_id)

# SECURE - verifies ownership
def get_document(doc_id, current_user):
    doc = db.documents.find(doc_id)

    if not doc:
        raise NotFoundError()  # Don't reveal existence

    if doc.owner_id != current_user.id:
        if not current_user.has_org_access(doc.org_id):
            raise NotFoundError()  # 404, not 403

    return doc
```

## Insecure Direct Object Reference (IDOR)

**Vulnerable Patterns:**
```
GET /api/users/123/profile      # Sequential ID
GET /api/invoices?id=456        # Guessable parameter
POST /api/transfer {"to": 789}  # User-controlled reference
```

**Prevention:**

| Strategy | Implementation |
|----------|----------------|
| UUIDs | Use UUIDv4 for all resource IDs |
| Ownership check | Verify `resource.owner == current_user` |
| Indirect reference | Map user-facing ID to internal ID |
| Scope queries | `WHERE user_id = current_user.id` |

**Always Check Parent Resources:**
```python
# Accessing a comment? Verify user owns the parent post
def get_comment(comment_id, current_user):
    comment = db.comments.find(comment_id)
    post = db.posts.find(comment.post_id)

    if post.owner_id != current_user.id:
        raise NotFoundError()

    return comment
```

## Role-Based Access Control (RBAC)

**Implementation:**
```python
PERMISSIONS = {
    'admin': ['read', 'write', 'delete', 'manage_users'],
    'editor': ['read', 'write'],
    'viewer': ['read']
}

def require_permission(permission):
    def decorator(func):
        def wrapper(current_user, *args, **kwargs):
            user_permissions = PERMISSIONS.get(current_user.role, [])
            if permission not in user_permissions:
                raise ForbiddenError()
            return func(current_user, *args, **kwargs)
        return wrapper
    return decorator
```

**RBAC Security Rules:**
- [ ] Roles assigned server-side only
- [ ] Role changes require re-authentication or session refresh
- [ ] Role checked on every request (not cached client-side)
- [ ] Sensitive role changes logged and auditable

## Multi-Tenant Security

**Data Isolation Strategies:**

1. **Query Scoping:**
```python
# Every query MUST include tenant filter
def get_all_documents(tenant_id):
    return db.documents.find(tenant_id=tenant_id)
```

2. **Row-Level Security (PostgreSQL):**
```sql
CREATE POLICY tenant_isolation ON documents
    USING (tenant_id = current_setting('app.tenant_id')::uuid);
```

3. **Schema-per-Tenant:**
```python
# Separate schema for each tenant
connection.execute(f"SET search_path TO tenant_{tenant_id}")
```

**Cross-Tenant Attack Prevention:**
- [ ] Tenant ID derived from authenticated session, never from request
- [ ] All queries filtered by tenant
- [ ] Bulk operations validate tenant ownership for each item
- [ ] Admin endpoints require explicit tenant context switch

## Privilege Escalation Prevention

**Vertical Escalation (User → Admin):**
```python
# VULNERABLE - trusts client data
def update_user(user_id, data):
    user = db.users.find(user_id)
    user.update(data)  # Attacker sends {"role": "admin"}

# SECURE - whitelist allowed fields
ALLOWED_FIELDS = ['name', 'email', 'avatar']
def update_user(user_id, data, current_user):
    if user_id != current_user.id and current_user.role != 'admin':
        raise ForbiddenError()

    safe_data = {k: v for k, v in data.items() if k in ALLOWED_FIELDS}
    db.users.find(user_id).update(safe_data)
```

**Horizontal Escalation (User A → User B):**
- Always verify resource ownership
- Never use user-provided user IDs for sensitive operations

## Mass Assignment Prevention

**Vulnerable:**
```python
user = User(**request.json)  # Accepts any field
user.save()
```

**Secure:**
```python
# Explicit field whitelist
allowed = ['name', 'email']
data = {k: v for k, v in request.json.items() if k in allowed}
user = User(**data)
```

## Account Lifecycle Security

**When User Removed from Organization:**
- [ ] Immediately revoke all access tokens
- [ ] Terminate active sessions
- [ ] Remove from cached permission sets
- [ ] Audit log the removal

**When Account Deleted/Deactivated:**
- [ ] Invalidate all sessions
- [ ] Revoke all API keys
- [ ] Remove OAuth grants
- [ ] Consider data retention policies

## Response Codes

| Scenario | Response | Reason |
|----------|----------|--------|
| Resource doesn't exist | 404 | Standard |
| Resource exists, no access | 404 | Prevent enumeration |
| Authenticated, wrong role | 403 | After confirming existence is OK |
| Not authenticated | 401 | Prompt for auth |

## API Authorization Patterns

**REST:**
```python
# Middleware checks auth, route checks authorization
@app.route('/api/documents/<doc_id>')
@require_auth
def get_document(doc_id):
    doc = Document.query.get_or_404(doc_id)
    if not current_user.can_access(doc):
        abort(404)
    return doc.to_dict()
```

**GraphQL:**
```python
# Field-level authorization
@strawberry.type
class Document:
    @strawberry.field
    def content(self, info) -> str:
        if not info.context.user.can_access(self):
            raise PermissionError()
        return self._content
```

## Checklist

- [ ] Every endpoint verifies user can access requested resource
- [ ] Resource IDs are UUIDs (not sequential)
- [ ] Parent resource ownership checked for nested resources
- [ ] Role/permission changes require re-auth
- [ ] Multi-tenant queries always scoped
- [ ] Mass assignment prevented with field whitelists
- [ ] 404 returned for both "not found" and "no access"
- [ ] Session/tokens invalidated on account removal
