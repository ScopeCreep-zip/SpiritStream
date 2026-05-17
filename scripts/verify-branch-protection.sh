#!/usr/bin/env bash
# Audit the GitHub repository settings and branch protection that
# back our CI/CD hardening posture. Read-only: this script only
# reports drift, never modifies settings.
#
# Required: gh CLI authenticated with a token that has admin:read
# on the repository.
#
# Authority: docs/08-security/branch-protection.md

set -euo pipefail

REPO="${SPIRITSTREAM_REPO:-ScopeCreep-zip/SpiritStream}"
echo "Auditing branch protection on ${REPO}"
echo

fail=0
note() { printf '  ✓ %s\n' "$*"; }
miss() { printf '  ✗ %s\n' "$*"; fail=1; }

# ── repo-level toggles ───────────────────────────────────────────────
echo "── Repository security settings ──"

repo_json=$(gh api "repos/${REPO}" 2>/dev/null)
sec_json=$(gh api "repos/${REPO}/properties/values" 2>/dev/null || echo '[]')

vuln_alerts=$(gh api "repos/${REPO}/vulnerability-alerts" --include 2>&1 | head -1)
if echo "$vuln_alerts" | grep -q "204"; then
  note "Dependabot vulnerability alerts: enabled"
else
  miss "Dependabot vulnerability alerts: NOT enabled"
fi

secret_scan_status=$(echo "$repo_json" | jq -r '.security_and_analysis.secret_scanning.status // "missing"')
if [ "$secret_scan_status" = "enabled" ]; then
  note "Secret scanning: enabled"
else
  miss "Secret scanning: $secret_scan_status (expected enabled)"
fi

secret_push_status=$(echo "$repo_json" | jq -r '.security_and_analysis.secret_scanning_push_protection.status // "missing"')
if [ "$secret_push_status" = "enabled" ]; then
  note "Secret scanning push protection: enabled"
else
  miss "Secret scanning push protection: $secret_push_status (expected enabled)"
fi

private_vuln_status=$(echo "$repo_json" | jq -r '.security_and_analysis.private_vulnerability_reporting.status // "missing"')
if [ "$private_vuln_status" = "enabled" ]; then
  note "Private vulnerability reporting: enabled"
else
  miss "Private vulnerability reporting: $private_vuln_status (SECURITY.md links here)"
fi

dependabot_security_updates=$(echo "$repo_json" | jq -r '.security_and_analysis.dependabot_security_updates.status // "missing"')
if [ "$dependabot_security_updates" = "enabled" ]; then
  note "Dependabot security updates: enabled"
else
  miss "Dependabot security updates: $dependabot_security_updates (expected enabled)"
fi

echo

# ── Actions policy ───────────────────────────────────────────────────
echo "── Actions policy ──"

actions_json=$(gh api "repos/${REPO}/actions/permissions" 2>/dev/null || echo '{}')
allowed_actions=$(echo "$actions_json" | jq -r '.allowed_actions // "missing"')
case "$allowed_actions" in
  selected) note "Allowed actions: 'selected' (custom allow-list — good)" ;;
  local_only|all) miss "Allowed actions: '$allowed_actions' — should be 'selected' (allow-list our pinned publishers only)" ;;
  *) miss "Allowed actions: $allowed_actions" ;;
esac

workflow_permissions=$(gh api "repos/${REPO}/actions/permissions/workflow" 2>/dev/null || echo '{}')
default_perm=$(echo "$workflow_permissions" | jq -r '.default_workflow_permissions // "missing"')
if [ "$default_perm" = "read" ]; then
  note "Default workflow token permissions: read"
else
  miss "Default workflow token permissions: $default_perm (expected 'read')"
fi
can_approve=$(echo "$workflow_permissions" | jq -r '.can_approve_pull_request_reviews // false')
if [ "$can_approve" = "false" ]; then
  note "Actions can approve PRs: disabled"
else
  miss "Actions can approve PRs: enabled (expected disabled)"
fi

echo

# ── Branch rulesets ──────────────────────────────────────────────────
echo "── Branch rulesets ──"

rulesets=$(gh api "repos/${REPO}/rulesets" 2>/dev/null || echo '[]')
main_ruleset_id=$(echo "$rulesets" | jq -r '.[] | select(.conditions.ref_name.include // [] | any(. == "refs/heads/main")) | .id' | head -1)
if [ -z "$main_ruleset_id" ]; then
  miss "No ruleset targeting refs/heads/main"
else
  note "main ruleset id: $main_ruleset_id"
  main_rules=$(gh api "repos/${REPO}/rulesets/$main_ruleset_id" 2>/dev/null)
  enforcement=$(echo "$main_rules" | jq -r '.enforcement')
  [ "$enforcement" = "active" ] && note "  enforcement: active" || miss "  enforcement: $enforcement (expected active)"

  rule_types=$(echo "$main_rules" | jq -r '.rules[].type' | sort -u)
  for required in required_signatures pull_request required_status_checks required_linear_history non_fast_forward deletion update; do
    if echo "$rule_types" | grep -qx "$required"; then
      note "  rule present: $required"
    else
      miss "  rule MISSING: $required"
    fi
  done

  pr_rule=$(echo "$main_rules" | jq -r '.rules[] | select(.type == "pull_request") | .parameters')
  if [ -n "$pr_rule" ]; then
    require_owners=$(echo "$pr_rule" | jq -r '.require_code_owner_review // false')
    [ "$require_owners" = "true" ] && note "  pull_request: requires CODEOWNERS review" || miss "  pull_request: CODEOWNERS review NOT required"
    dismiss_stale=$(echo "$pr_rule" | jq -r '.dismiss_stale_reviews_on_push // false')
    [ "$dismiss_stale" = "true" ] && note "  pull_request: dismisses stale reviews" || miss "  pull_request: does NOT dismiss stale reviews"
  fi

  status_rule=$(echo "$main_rules" | jq -r '.rules[] | select(.type == "required_status_checks") | .parameters.required_status_checks[].context' | sort -u)
  for check in \
    "CI / pnpm-audit" \
    "CI / a11y-axe" \
    "CodeQL / Analyze (javascript-typescript)" \
    "CodeQL / Analyze (rust)" \
    "cargo-deny / cargo-deny (advisories)" \
    "cargo-deny / cargo-deny (bans)" \
    "cargo-deny / cargo-deny (licenses)" \
    "cargo-deny / cargo-deny (sources)" \
    "Dependency Review / dependency-review" \
    "Secret Scan / gitleaks" \
    "PR Quality / commitlint" \
    "PR Quality / ai-disclosure" \
    "PR Quality / issue-link" \
    "PR Quality / diff-size"
  do
    if echo "$status_rule" | grep -qx "$check"; then
      note "  required status check: $check"
    else
      miss "  required status check MISSING: $check"
    fi
  done
fi

echo

# ── Tag protection ───────────────────────────────────────────────────
echo "── Tag protection ──"
tag_rulesets=$(echo "$rulesets" | jq -r '.[] | select(.target == "tag")')
if [ -n "$tag_rulesets" ]; then
  note "Tag ruleset present (releases protected)"
else
  miss "No tag ruleset — anyone with write access can push v* release tags"
fi

echo
if [ "$fail" -eq 0 ]; then
  echo "✅ All checks passed."
  exit 0
else
  echo "❌ Drift detected. Update settings per docs/08-security/branch-protection.md and re-run."
  exit 1
fi
