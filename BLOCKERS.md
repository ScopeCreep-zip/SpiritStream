# Deferred work — known debt with explicit decisions

Items below are intentionally not in the main work queue. Each entry
names the smell, the reason it's deferred, and the conditions that
would unblock it.

For Veilid-transport contract gaps specifically, see
[`crates/transport-veilid/BLOCKERS.md`](./crates/transport-veilid/BLOCKERS.md).

---

## 1. `packages/validation/` — decoupling from openapi-ts codegen

The `packages/validation` workspace member ships 39 auto-generated JSON
Schemas derived from utoipa annotations on the HTTP transport. None of
them are imported anywhere in `apps/`. The original intent was
form-level UX validation in the frontend, but the typed-REST contract
landed first: every backend handler returns `Result<Json<T>, ApiError>`
and `CoreError::ValidationFailed { reasons: Vec<ValidationIssue> }`
already carries field-level errors over the wire with a stable schema.
Re-validating client-side would buy zero defense-in-depth value for the
threat model and would let the frontend drift from the backend's
authoritative validation.

**Why it's still around**: `scripts/regenerate-validation.sh` is the
same script the api-client's `@hey-api/openapi-ts` codegen invokes —
it fetches `openapi.json` from the live server and splits the schemas
out. Deleting `packages/validation` requires refactoring that script
to keep the openapi.json fetch but drop the schema-splitting half,
then dropping the workspace dep from `packages/api-client/package.json`.

**Unblock criteria**: a follow-up PR that
1. Splits `scripts/regenerate-validation.sh` into a clean
   `scripts/regenerate-openapi.sh` (fetch only) used by api-client.
2. Removes the workspace dep at `packages/api-client/package.json:19`.
3. Deletes `packages/validation/` entirely.
4. Removes the package from the workspace member list.

Until then the package is harmless deadweight — it builds, ships zero
code into any consumer, and the codegen pipeline keeps working.

**Threat-model note**: a future runtime validator at the api-client
boundary IS a defensible "compromised-server can't poison frontend"
posture, but the threat model for this app (compromised server *is*
the maintainer's machine in the offline-signing model) makes that low
priority. Revisit if the deployment model shifts toward multi-tenant
cloud where the server and client trust each other less.

---

## 2. `packages/ui/` is untracked — primitives not yet committed

The whole `packages/ui/` directory (`ConfirmDialog`, `FormField`,
`PasswordInput`, `useFormState`, `useFormValidation`) sits in the
working tree but is **not in git**. The work landed locally but
was never staged. Active consumers in `apps/web/src` import the
primitives, so the build depends on the working-tree state.

**Unblock**: a commit that adds `packages/ui/` and its `package.json`
to the tree. (Probably the same commit that lands this BLOCKERS file.)

---

## 3. `FormField` — design decision pending

`packages/ui/src/FormField.tsx` exists but no `apps/web` component
uses it; `Input` and `Select` in `apps/web/src/components/ui/` own
their own label/helper/error rendering. Two valid shapes:

- **A. FormField canonical**: refactor `Input` and `Select` to render
  just the control. Wrap every use in `<FormField>` for the label /
  ARIA linkage. ~30 call sites touched.
- **B. Input/Select self-contained**: `FormField` becomes a niche
  escape-hatch for custom controls (textareas, control composites)
  that don't exist in this codebase today.

The audit defaulted to (B) but the user pushed back on deleting
FormField, so it stays. Decide which shape lands when the design
wraps formally.

---

## 4. Updater public key not yet generated

`apps/tauri/src-tauri/tauri.conf.json:84` contains the placeholder
`REPLACE_WITH_OUTPUT_OF__tauri_signer_generate`. The release pipeline
fails loudly if this isn't replaced (`validate-version` job in
`.github/workflows/release.yml`), so no broken release can ship — but
the auto-updater path won't work in any deployed build until the
maintainer runs the one-time setup in
[`docs/07-deployment/04-self-update-ritual.md`](./docs/07-deployment/04-self-update-ritual.md).
