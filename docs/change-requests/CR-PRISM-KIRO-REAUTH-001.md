# Kiro device reauthorization

Status: accepted and implemented by Codex under joint ownership authorization.

The existing start/poll/cancel inputs add optional `replace_existing` (default false). Replacement requires an existing bearer credential owned by the selected upstream. Its observed revision participates in session scope; poll completion uses CAS, retains ID, disabled status and bindings, and returns 200 (first creation remains 201). No placeholder, token exposure, or automatic write replay.

Authoritative OpenAPI synchronized to Prism. Expanded injected-exchange HTTP regression covers creation, replacement with disabled account and existing binding, and stale completion conflict. Real official login remains a separately identified human acceptance step.
