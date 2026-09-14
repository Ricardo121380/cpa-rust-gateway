# Schema28 compatibility standby

Based on signed915983b. Adds only the schema28 request-key index migration and Claude OAuth optional email/token endpoint compatibility. Keeps existing request_finished handling and manual-model permission guard. No downgrade or old-state restore is needed; retain latest rotated credentials and administrator state. Final signed-artifact and isolated production-copy verification must precede any deployment. This is not the main delivery branch.
