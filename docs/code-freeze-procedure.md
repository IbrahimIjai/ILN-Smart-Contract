# Code Freeze Procedure

**Document Version:** 1.0  
**Status:** Active  
**Applies To:** All contributors to `ILN-Smart-Contract`

This document defines the process for entering and maintaining a code freeze before a formal security audit. The goal is to give the audit firm a stable, known-good snapshot of the codebase while keeping the team unblocked for non-contract work.

---

## 1. What Is a Code Freeze?

A code freeze means **no changes to contract source code** are merged to the audit branch from the moment the audit firm begins review until they deliver their final report. This ensures:

- The audit firm reviews exactly what will be deployed.
- Findings map 1:1 to the audited code — no "that was already fixed" confusion.
- The audit commit SHA is a reliable reference for the final report.

A freeze does **not** block:
- Documentation updates (non-contract `docs/` changes).
- CI/CD configuration changes that do not affect contract logic.
- Off-chain tooling (`scripts/`, `gen-abi.ts`, etc.).
- Dependency bumps in dev-only crates (requires maintainer approval).

---

## 2. Branch Naming Convention

| Branch | Purpose |
|--------|---------|
| `main` | Active development — continues normally during freeze |
| `audit/v<major>.<minor>` | Frozen audit branch — **no contract changes after freeze announcement** |
| `audit/v1.0` | First formal audit (this document's target) |
| `hotfix/audit-<issue>` | Emergency exception branch (see Section 5) |

The audit branch is created from `main` at the moment the freeze is declared. All subsequent audit-prep work (checklist items, doc fixes) is merged to `main` first, then cherry-picked to the audit branch only if they are non-contract changes.

**Example:**
```bash
# Create the audit branch from main at freeze time
git checkout main
git pull origin main
git checkout -b audit/v1.0
git push -u origin audit/v1.0
```

---

## 3. Pre-Freeze Checklist

Complete the [Pre-Audit Security Checklist](pre-audit-checklist.md) before declaring the freeze. The freeze announcement must not go out until a maintainer has signed off on that document.

Minimum gates before freeze:
- [ ] All CI jobs pass on `main` (green badge).
- [ ] `cargo tarpaulin` coverage ≥ 95% on `invoice_liquidity`.
- [ ] `cargo clippy --all-targets -- -D warnings` exits 0.
- [ ] `cargo deny check` passes.
- [ ] Pre-audit checklist maintainer sign-off is complete.
- [ ] Audit firm has confirmed their start date.

---

## 4. Freeze Announcement

When the above gates are met, the lead maintainer posts the freeze announcement in the team's primary communication channel (Slack / Discord / GitHub Discussion) and pins it.

**Announcement template:**

> **🔒 Code Freeze — Audit Branch `audit/v1.0`**
>
> Effective immediately, the `audit/v1.0` branch is frozen for the formal security audit.
>
> - **Freeze commit:** `<SHA>`
> - **Audit firm:** `<Firm Name>`
> - **Audit window:** `<Start Date>` → `<Expected End Date>`
>
> **What this means:**
> - No PRs touching `contracts/` source files will be merged to `audit/v1.0` during this window.
> - Development on `main` continues as normal.
> - All new issues discovered during the audit are tracked but fixes are queued for post-audit.
>
> **Emergency exceptions** require approval from two maintainers. See the [Code Freeze Procedure](docs/code-freeze-procedure.md) for the exception process.
>
> Questions? Ping `@<lead-maintainer>`.

The freeze commit SHA must also be recorded in the [Pre-Audit Checklist](pre-audit-checklist.md) under "Maintainer Sign-Off".

---

## 5. Emergency Exception Process

An emergency exception allows a critical fix to be applied to the audit branch during the freeze. This is reserved for:

- A **critical security vulnerability** discovered after freeze that would make the audit meaningless if left unpatched (e.g., a fund-draining bug).
- A **build-breaking regression** that prevents the audit firm from compiling the code.

It does **not** cover:
- Feature additions.
- Non-critical bug fixes.
- Documentation changes (those are always allowed).
- "Nice to have" improvements.

### Exception Workflow

1. **Raise an issue** tagged `freeze-exception` describing the problem and proposed fix.
2. **Two maintainers must approve** the exception in the issue thread before any code is written.
3. **Create a hotfix branch** from the audit branch:
   ```bash
   git checkout audit/v1.0
   git checkout -b hotfix/audit-<issue-number>
   ```
4. **Implement the minimal fix** — no refactoring, no unrelated changes.
5. **Open a PR** targeting `audit/v1.0` with the `freeze-exception` label.
6. **Notify the audit firm** immediately. They must acknowledge the change and confirm whether it affects their in-progress findings before the PR is merged.
7. **Merge to `main` as well** — the fix must not diverge between branches.
8. **Update the freeze commit SHA** in the Pre-Audit Checklist to the new HEAD of `audit/v1.0`.
9. **Post an update** to the freeze announcement thread with the new SHA and a summary of the change.

### Exception Approval Record

All approved exceptions must be recorded here:

| Date | Issue | Description | Approvers | New Audit SHA |
|------|-------|-------------|-----------|---------------|
| — | — | — | — | — |

---

## 6. During the Freeze

### What maintainers do

- Continue merging PRs to `main` as normal.
- Triage audit findings as they arrive (track in GitHub Issues, do not fix on audit branch).
- Respond to audit firm questions within one business day.
- Keep the audit firm informed of any emergency exceptions (Section 5).

### What contributors do

- Do not open PRs targeting `audit/v1.0` unless they are documentation-only or an approved exception.
- If you discover a security issue during the freeze, report it privately to the lead maintainer first — do not open a public issue.
- Tag any issues that are audit findings with `audit-finding` for tracking.

### Branch protection rules for `audit/v1.0`

Apply the following GitHub branch protection settings to `audit/v1.0` before the freeze announcement:

- **Require pull request reviews before merging:** 2 approvals required.
- **Require status checks to pass:** `test`, `clippy`, `coverage`.
- **Restrict who can push:** Maintainers only (no direct pushes).
- **Do not allow force pushes.**
- **Do not allow deletions.**

---

## 7. Post-Audit

When the audit firm delivers their final report:

1. **Triage findings** — classify each finding as Critical / High / Medium / Low / Informational.
2. **Create issues** for each finding tagged `audit-finding` and the severity label.
3. **Prioritize fixes** — Critical and High findings must be fixed before mainnet deployment.
4. **Implement fixes on `main`** — do not modify `audit/v1.0` post-audit (it is the historical record).
5. **Re-audit scope** — if fixes are substantial, schedule a re-audit or at minimum a fix review with the audit firm.
6. **Publish the report** — once all Critical/High findings are resolved, publish the audit report in `docs/audits/` and link it from `README.md` and `SECURITY.md`.
7. **Tag the release** — create a git tag `v1.0.0-audited` on the commit that incorporates all audit fixes.

**Report storage convention:**
```
docs/audits/
  <YYYY-MM>-<firm-name>-audit-report.pdf
  <YYYY-MM>-<firm-name>-fix-review.pdf   # if applicable
```

---

## 8. Roles and Responsibilities

| Role | Responsibility |
|------|---------------|
| **Lead Maintainer** | Declares freeze, posts announcement, coordinates with audit firm, approves exceptions |
| **Security Reviewer** | Signs off on pre-audit checklist, reviews exception PRs |
| **All Maintainers** | Enforce freeze on PRs, triage audit findings, implement fixes post-audit |
| **Contributors** | Respect freeze, report security issues privately, tag audit-related issues correctly |
| **Audit Firm** | Reviews frozen codebase, reports findings, acknowledges any emergency exceptions |

---

## 9. Quick Reference

```
Pre-freeze
  └─ Complete pre-audit-checklist.md
  └─ All CI green on main
  └─ Maintainer sign-off

Freeze declaration
  └─ git checkout -b audit/v1.0 from main
  └─ Apply branch protection rules
  └─ Post freeze announcement with SHA
  └─ Record SHA in pre-audit-checklist.md

During freeze
  └─ main: normal development continues
  └─ audit/v1.0: docs-only changes allowed
  └─ Exceptions: 2-maintainer approval + audit firm notification

Post-audit
  └─ Triage findings → GitHub Issues
  └─ Fix Critical/High on main
  └─ Publish report in docs/audits/
  └─ Tag v1.0.0-audited
```

---

## Related Documents

- [Pre-Audit Security Checklist](pre-audit-checklist.md)
- [Threat Model](threat-model.md)
- [Upgrade Guide](upgrade-guide.md)
- [Contributing Guide](../CONTRIBUTING.md)
