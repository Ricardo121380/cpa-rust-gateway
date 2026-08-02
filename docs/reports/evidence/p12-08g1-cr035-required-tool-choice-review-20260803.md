# P12-08G1 CR-035 required Tool choice review

| Field | Value-free result |
|---|---|
| Resume receipt | Tuple 10 passed; tuple 11 stopped with local `http_5xx`; G1 v2 is 10/12 |
| Attempt boundary | Tuple 11 produced no upstream Attempt |
| Root cause | Forced Messages Tool choice had no reviewed Messages-to-Responses root-extension mapping |
| Runtime change | Forced Tool selection maps across all three protocols only when Tools are present |
| Preserved rejection | Automatic, malformed, foreign and Tool-less forced choices remain fail closed |
| Focused verification | 22 router transform tests, F2 loopback matrix and affected-package Clippy passed |
| Full gate | `./scripts/check.sh full` passed, including all Rust tests, source policy, tracked secret scan, dependency policy and RustSec |
| Deployment | Not performed by this receipt; a new exact-SHA signed artifact is required |

Review found no request, response, credential, endpoint, model, identifier, timestamp, token value or
fingerprint in the retained evidence. The prior ten passing tuples must not be resent. After the new
ARM64 artifact is independently verified and deployed, only tuple eleven may resume; tuple twelve
remains conditional on tuple eleven passing.
