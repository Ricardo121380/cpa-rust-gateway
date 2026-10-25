# Local integration of #68, #72 and #74

Date: 2026-10-25 (Asia/Shanghai). Branch: `feature/parallel-68-72-74-integration`.
Common ancestor: `444558a8de930f5f1febd5a40ec5bdbbc30a15fb`.

Integration sources: #68 `3bf4ffef6badcb172f64784d4d16b4b24702dfc6`,
#72 `60073e72e8abd22f533178f6ec1b9fe97ce36c4c`,
#74 `709aa4111726fb0c8efcc2f02ee740cdf6b5d60b` through its security successor
`d78f44c76c16c4b18b740646aa0cb9389e04136c`.

Validation is pending on this integrated source; historical branch PASS results are not integration results.
The final local receipt records commit/tree, lockfile, build commands and binary hashes separately.

The dependency union retains #68's tokenizer, #72's direct serde edge and one upgraded P-256 chain.
DPoP retains fallible key generation and all three production error mappings. Wire/JWK, RFC6979,
wrong-key, expiry, tampering and cache regressions are preserved. The redundant #74 wrong-key test
is covered by #68's combined wrong-key/expiry test with the same KeyMismatch assertion.

#72 retains ADR-0128; #74 moves from branch-local ADR-0128 to ADR-0129 without changing its
Accepted decision. Historical [#68](crypto-bigint-security-fix-20261025-issue68.md) and
[#72](crypto-bigint-security-fix-20261025-issue72.md) security reports retain their original bytes.
The old report path is a navigation index. Original failures and evidence identities remain historical.

No push, remote issue changes, deployment or real Provider calls are authorized or performed.
