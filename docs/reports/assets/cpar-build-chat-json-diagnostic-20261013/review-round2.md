# Second preparation review, retained before startup repair

Fixed base810d210f, targeta56c02fc. Standards PASS with cumulative37/37 scope: prior26,23 unchanged objects,3 re-read objects plus complete14-file delta. Both initial controller P2 defects are closed, verified byte cache and local flock protected through exception/stop.

Spec-control FAIL with complete8/8: remote-start.py:149–156 retried URLError including HTTP401/403; remote-start.py:107–135 and165–166 did not bind actual startup listener sockets to the frozen PID. The startup management request must execute once after kernel listener readiness, and both127.0.0.1:19180/19181 sockets must belong to the frozen process before and after the65-second silent observation. Offline RED2 then GREEN2 are retained in start-checks.json.

Spec-execution PASS for all4 unchanged formal prepare/pin/chat/exit scripts:704 lines/hunks, hashes and comparison with fixedc23d2c35 driver verified.

Spec-evidence PARTIAL:25/25 files fully read,18 texts/hunks and7 gzip logs; freeze16,tar17,binary/manifest SHA,assembled argv SHA and log hashes were verified. The helper then incorrectly indexed approved_launch (a list) as a dictionary and stopped. Recipe/options, manifest2 rows and initial GH body/AC/dependencies require a fresh narrow independent completion; PARTIAL is not PASS.

No qualification observation, upload, remote epoch, start, Pin or Chat has occurred. Fresh review of startup repair and remaining evidence checks is required before execution.
