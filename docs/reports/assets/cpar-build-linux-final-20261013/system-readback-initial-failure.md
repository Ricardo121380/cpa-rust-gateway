# Retained independent readback FAIL

The initial read-only helper returned1 at remote line39, comparing the native final-receipt.json to the public stdout projection. remote-exit.py deliberately omits private_files and adds private_file_count/private_inventory_sha256 before stdout. Thus exact native-vs-public equality is invalid.

The FAIL receipt system-readback.json and private system-readback stderr/executed source remain unchanged; source archived as tools/system-readback-initial.py. This is a new helper defect, not a system run. No launch, stop or Provider intent occurred. No A/B/C/D frozen/runtime byte changes; their evidence remains applicable only to the original fixed system bundle f67a5091. The corrected helper must prove the exact declared projection plus native inventory digest in a separate read-only readback, before qualification.
