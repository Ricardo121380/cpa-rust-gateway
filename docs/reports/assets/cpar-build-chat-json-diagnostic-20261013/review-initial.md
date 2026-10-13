# Pre-execution review retained before repair

Fixed base810d210f, target29931b34. Standards FAIL with complete26/26 file coverage; Spec PARTIAL because the scout was interrupted without a final verdict. Neither is a pre-execution PASS.

Standards P2 at tools/local-control.py:88,92–93,107–109: public qualification import and observation script reads were not the payload bytes verified against the freeze. The actual isolated controller test reproduced an unreviewed public observation reaching transport. The repair executes the previously verified private payload byte cache and verifies/caches the upload script.

Standards P2 at tools/local-control.py:146–150,159–167: independent local load/execute/save paths lacked a lock; a late successful writer could overwrite stopped state. The isolated process test reproduced a non-owner entering state mutation while another process owned the execution lock. The repair holds one exclusive nonblocking local lock through the complete action and exception/stop path; a non-owner exits before state writes and transport.

The Standards scout fully read all26 changed files (23 texts,1868 new lines and all hunks;3 decompressed logs), inspected4 commits and verified payload16/16, tar17/17, log/freeze/controller/uploader hashes and AGENTS preservation. An optional final digest script had SyntaxError; its output is not evidence.

Both behavioral regressions are retained RED2/2 then GREEN2/2. No qualification observation, remote epoch, upload, temporary start, Pin or Chat was performed before these repairs. Fresh independent review is required for execution.
