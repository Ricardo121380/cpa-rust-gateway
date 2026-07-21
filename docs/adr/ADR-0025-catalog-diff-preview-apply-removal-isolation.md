# ADR-0025: Catalog diff Preview/Apply and removal isolation

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-21` |
| Task / Matrix / Contract references | `P4-03`; `E20`, `G28`, `L09`, `L10`, `L33`; [BC-CATALOG-003](../contracts/BC-CATALOG-003-catalog-diff-preview-apply-removal-isolation.md) |

## Context

P4-02 retains immutable successful discovery snapshots per exact Endpoint/Credential target, but
deliberately does not infer that a model disappeared. A transient upstream list omission must not
silently remove route eligibility or discard last-success evidence. The frozen architecture requires
three consecutive successful absences and at least 24 hours of isolation before a removal.

The control plane also needs to show an operator a deterministic diff before it changes retained
discovery evidence. A stale preview must not overwrite a newer applied discovery for the same target.

## Decision

- `CatalogDiffRegistry` is an in-memory, exact-`ModelCatalogTarget` registry. It accepts only a
  successful `CatalogSnapshot`; a source failure has no Preview/Apply input and cannot increment a
  missing counter or clear a model.
- `preview` is immutable. It emits deterministic `Added`, `SuspectedRemoved`, and `Removed` events
  without mutating state. `apply` is a target-local generation compare-and-set: it accepts only a
  preview built from the current generation and rejects an older plan atomically.
- The first successful snapshot emits `Added` for its normalized discovered models. A missing model
  becomes `SuspectedRemoved` on its first successful absence. It becomes `Removed` only at the
  third consecutive successful absence whose explicit observation time is at least 24 hours after
  the first absence. Reappearance before removal clears the suspicion; reappearance after removal
  is a new `Added` event.
- The registry never touches static/manual Catalog entries, URLs, Credential material, HTTP,
  SQLite, RouteSnapshot publication, health, quota, public model visibility, or event persistence.

## Consequences

Removal evidence remains credential-isolated: one Credential's model list cannot add, suspect, or
remove a model for a sibling Credential at the same Endpoint. The state machine is deterministic
with explicit Unix milliseconds, so Preview/Apply can be tested without network or wall-clock time.

P4-03 records discovery evidence only. A future control-plane integration must make any action on a
`Removed` event explicit; this Task does not auto-delete configuration, Public Models, aliases,
mappings, or routes.

## Alternatives considered

- Remove on first missing list entry: rejected because a transient discovery failure or partial list
  is not evidence of entitlement removal.
- Count failed discovery attempts as absences: rejected because failures preserve last success under
  the model-discovery contract.
- Apply a preview without version/generation validation: rejected because an older diff could erase
  evidence from a newer successful snapshot.
- Share a diff map by Endpoint only: rejected because different Credentials can expose different
  models and permissions at the same Endpoint.

## Validation and rollback

Synthetic tests prove preview non-mutation, stale-preview rejection, three-successful-miss plus
24-hour removal, reappearance reset/add behavior, and same-Endpoint Credential isolation. They use
no Provider request, URL, Credential value, database, route selection, or real time.

Rollback removes the diff registry and its documentation. It changes no persisted record, deployed
endpoint, RouteSnapshot, static/manual model, health/quota state, P4-02 last-success snapshot, or
P4-01 scheduler behavior.
