// Generated from docs/openapi/management-v1.json by scripts/gen-types.mjs. Do not edit.
export interface paths {
    "/admin/billing/price-source/refresh": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** Read public USD prices from fixed models.dev source; does not modify catalogs */
        post: operations["refreshPriceSource"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/upstreams/{upstream_id}/kiro-authorization/start": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Kiro AWS device authorization start
         * @description Revision-bound Kiro AWS device authorization in an owned draft; no placeholder account or browser Provider secret.
         */
        post: operations["startKiroEnrollment"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/upstreams/{upstream_id}/kiro-authorization/poll": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Kiro AWS device authorization poll
         * @description Revision-bound Kiro AWS device authorization in an owned draft; no placeholder account or browser Provider secret.
         */
        post: operations["pollKiroEnrollment"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/upstreams/{upstream_id}/kiro-authorization/cancel": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Kiro AWS device authorization cancel
         * @description Revision-bound Kiro AWS device authorization in an owned draft; no placeholder account or browser Provider secret.
         */
        post: operations["cancelKiroEnrollment"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/upstreams/{upstream_id}/kimi-authorization/start": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Start Kimi Coding device authorization
         * @description Revision-bound Kimi device authorization for one dedicated Kimi upstream. It returns only the user code and verified Kimi URL; no placeholder account, provider selector, endpoint selector, or browser secret.
         */
        post: operations["startKimiEnrollment"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/upstreams/{upstream_id}/kimi-authorization/poll": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Poll Kimi Coding device authorization
         * @description Polls only the revision-bound Kimi session after its provider interval. Pending, slow-down, denied, expired, cancel and completed states are explicit; a late response never changes another session.
         */
        post: operations["pollKimiEnrollment"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/upstreams/{upstream_id}/kimi-authorization/cancel": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Cancel Kimi Coding device authorization
         * @description Cancels the exact bound Kimi device session without creating or changing an account.
         */
        post: operations["cancelKimiEnrollment"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/account-channels/kimi/prepare-target": {
        parameters: {
            query?: {
                /** @description Explicit existing Kimi Coding service; unknown or foreign owner is rejected. */
                upstream_id?: string;
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Resolve or prepare Kimi Coding account target
         * @description Resolves the explicitly selected Kimi Coding service, otherwise requires exactly one Kimi-owned Responses target or creates the canonical Kimi Coding egress policy, upstream and Responses endpoint in the current draft. It never selects compatible relays, internal IDs or array positions.
         */
        post: operations["prepareKimiAccountTarget"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/account-channels/kiro/prepare-target": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Resolve or prepare Kiro account target
         * @description Resolves one region-owned Kiro target or creates the standard Kiro egress policy, upstream and Messages endpoint in the current draft. Region is an optional advanced input; the browser never chooses a Provider or Endpoint.
         */
        post: operations["prepareKiroAccountTarget"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/account-channels/codex/prepare-target": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Resolve or prepare Codex account target
         * @description Resolves the canonical Codex / ChatGPT Responses target or creates its fixed egress policy, upstream and endpoint in the current draft. The browser never chooses a Provider or Endpoint.
         */
        post: operations["prepareCodexAccountTarget"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/account-channels/claude/prepare-target": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Resolve or prepare Claude account target
         * @description Resolves the canonical Claude Messages target or creates its fixed egress policy, upstream and endpoint in the current draft. The browser never chooses a Provider or Endpoint.
         */
        post: operations["prepareClaudeAccountTarget"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/upstreams/{upstream_id}/account-imports/{import_id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getAccountImportReceipt"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/native-account-imports/{batch_id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["getNativeAccountImportReceipt"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/native-accounts": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listNativeAccounts"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/native-accounts/import": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["importNativeAccount"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/native-account-authorizations": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["startNativeAccountAuthorization"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/native-account-authorizations/{session_id}/poll": {
        parameters: {
            query?: never;
            header?: never;
            path: {
                session_id: string;
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["pollNativeAccountAuthorization"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/native-account-authorizations/{session_id}": {
        parameters: {
            query?: never;
            header?: never;
            path: {
                session_id: string;
            };
            cookie?: never;
        };
        /**
         * Read the original Grok Build device result without exchanging or applying
         * @description Actor-bound and process-local, within the original device TTL. Excludes device codes and URLs; a missing, foreign, expired or restarted session remains unconfirmed. A saved result identifies the exact account and credential revision separately from runtime application. No Provider I/O, credential mutation or runtime apply.
         */
        get: operations["getNativeAccountAuthorizationReceipt"];
        put?: never;
        post?: never;
        delete: operations["cancelNativeAccountAuthorization"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/account-channels": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listAccountChannels"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/upstreams/{upstream_id}/account-import": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Identical normalized material in the same Upstream returns the existing account instead of duplicating it. */
        post: operations["importChannelAccount"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/credentials/{credential_id}/status": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        /** @description Draft-only administrative status change. Retains ciphertext and identity; checks both config and credential revisions. Never accepts secret material. */
        patch: operations["updateCredentialStatus"];
        trace?: never;
    };
    "/admin/auth/login": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Exact configured Origin and actual management peer required; no key or preexisting CSRF. 10 verifications per minute, 2 concurrent. Returns a memory-only opaque session. Initial sessions can only change password/logout. */
        post: operations["loginAdministrator"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/auth/password": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Opaque session and X-Management-CSRF-Token required, even without Origin. Password change revokes every session and requires login again. */
        post: operations["changeAdministratorPassword"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/auth/logout": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Revoke the presented session. Always requires session CSRF. */
        post: operations["logoutAdministrator"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/config-versions/{config_version_id}/fork": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                config_version_id: components["parameters"]["ConfigVersionId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Fork the complete active configuration into a guarded draft. Copies every resource and re-seals version-bound credentials/proxy secrets inside the server. Requires exact source revision. Later source changes or publication/rollback cause draft activation to fail with 409. Does not apply the draft to serving. No secrets are returned. */
        post: operations["forkConfigVersion"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/config-versions/{config_version_id}/diff": {
        parameters: {
            query?: never;
            header?: never;
            path: {
                config_version_id: components["parameters"]["ConfigVersionId"];
            };
            cookie?: never;
        };
        /** @description Compare persisted resources without returning field values. Ciphertext differences do not imply plaintext differences. Cursor pins both identities and revisions; changes return 409. Uses a bounded blocking reader and one read-only SQLite transaction. */
        get: operations["compareConfigVersions"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/resource-audit-events": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        /** @description Bounded newest-first resource mutation audit for the selected version. before_id is an exclusive append ID; later appends never enter continuation pages. ID strings preserve SQLite precision. No secrets or mutation payloads are returned. */
        get: operations["listManagementResourceAuditEvents"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/config-versions": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listConfigVersions"];
        put?: never;
        post: operations["createConfigVersion"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/config-versions/{config_version_id}": {
        parameters: {
            query?: never;
            header?: never;
            path: {
                config_version_id: components["parameters"]["ConfigVersionId"];
            };
            cookie?: never;
        };
        get: operations["getConfigVersion"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/config-versions/{config_version_id}/validate": {
        parameters: {
            query?: never;
            header?: never;
            path: {
                config_version_id: components["parameters"]["ConfigVersionId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["validateConfigVersion"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/config-versions/{config_version_id}/publish": {
        parameters: {
            query?: never;
            header: {
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
                /** @description JSON string of expected active configuration ID, or null. Mismatch returns 409 before mutation. Optional for legacy callers. */
                "X-Expected-Active-Version"?: string;
                /** @description Latest publication or rollback append ID observed in audit, or 0 if absent. Prevents active-version ABA changes during confirmation; mismatch returns 409. */
                "X-Expected-Lifecycle-Event"?: number;
                /** @description Explicit confirmation of a current nonempty restoration review; stale or missing confirmation fails closed in the activation transaction. */
                "X-Account-Restoration-Review"?: string;
            };
            path: {
                config_version_id: components["parameters"]["ConfigVersionId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["publishConfigVersion"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/config-versions/rollback": {
        parameters: {
            query?: never;
            header: {
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
                /** @description JSON string of expected active configuration ID, or null. Mismatch returns 409 before mutation. Optional for legacy callers. */
                "X-Expected-Active-Version"?: string;
                /** @description Latest publication or rollback append ID observed in audit, or 0 if absent. Prevents active-version ABA changes during confirmation; mismatch returns 409. */
                "X-Expected-Lifecycle-Event"?: number;
                /** @description Explicit confirmation of a current nonempty restoration review; stale or missing confirmation fails closed in the activation transaction. */
                "X-Account-Restoration-Review"?: string;
            };
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["rollbackConfigVersion"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/egress-policies": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        get: operations["listEgressPolicies"];
        put?: never;
        post: operations["createEgressPolicy"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/egress-policies/{egress_policy_id}": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                egress_policy_id: components["parameters"]["EgressPolicyId"];
            };
            cookie?: never;
        };
        get: operations["getEgressPolicy"];
        put?: never;
        post?: never;
        delete: operations["deleteEgressPolicy"];
        options?: never;
        head?: never;
        patch: operations["updateEgressPolicy"];
        trace?: never;
    };
    "/admin/upstreams": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        get: operations["listUpstreams"];
        put?: never;
        post: operations["createUpstream"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/upstreams/{upstream_id}": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        get: operations["getUpstream"];
        put?: never;
        post?: never;
        delete: operations["deleteUpstream"];
        options?: never;
        head?: never;
        patch: operations["updateUpstream"];
        trace?: never;
    };
    "/admin/upstreams/{upstream_id}/endpoints": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["createEndpoint"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/endpoints/{endpoint_id}": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                endpoint_id: components["parameters"]["EndpointId"];
            };
            cookie?: never;
        };
        get: operations["getEndpoint"];
        put?: never;
        post?: never;
        delete: operations["deleteEndpoint"];
        options?: never;
        head?: never;
        patch: operations["updateEndpoint"];
        trace?: never;
    };
    "/admin/upstreams/{upstream_id}/credentials": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["createCredential"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/credentials/{credential_id}": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Exact reviewed impact digest. A stale supplied review is rejected before deletion. */
                "X-Deletion-Impact-Review"?: string;
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        get: operations["getCredential"];
        put?: never;
        post?: never;
        delete: operations["deleteCredential"];
        options?: never;
        head?: never;
        patch: operations["updateCredential"];
        trace?: never;
    };
    "/admin/endpoints/{endpoint_id}/credential-bindings": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                endpoint_id: components["parameters"]["EndpointId"];
            };
            cookie?: never;
        };
        get: operations["listEndpointCredentialBindings"];
        put?: never;
        post: operations["createEndpointCredentialBinding"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/endpoints/{endpoint_id}/test": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                endpoint_id: components["parameters"]["EndpointId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["testEndpoint"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/endpoints/{endpoint_id}/models/discover-preview": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                endpoint_id: components["parameters"]["EndpointId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["previewCatalogDiscovery"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/endpoints/{endpoint_id}/models/discover-apply": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                endpoint_id: components["parameters"]["EndpointId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["applyCatalogDiscovery"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/public-models": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        get: operations["listPublicModels"];
        put?: never;
        post: operations["createPublicModel"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/public-models/{public_model_id}": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                public_model_id: components["parameters"]["PublicModelId"];
            };
            cookie?: never;
        };
        get: operations["getPublicModel"];
        put?: never;
        post?: never;
        delete: operations["deletePublicModel"];
        options?: never;
        head?: never;
        patch: operations["updatePublicModel"];
        trace?: never;
    };
    "/admin/public-models/{public_model_id}/aliases": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                public_model_id: components["parameters"]["PublicModelId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["createModelAlias"];
        /** @description Delete one exact alias owned by the specified public model. Does not delete the model, routes, candidates or access grants. Revision conflict never replays a write. */
        delete: operations["deleteModelAlias"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/public-models/{public_model_id}/routes": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                public_model_id: components["parameters"]["PublicModelId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["createRoute"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/accounts/inventory": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Authenticated unified configured/native account projection. Search matches observed email, phone, username and actual provider/category over the complete bounded inventory, not loaded browser pages. Stable sorting and cursor bind filters, configuration revision, resource audit and native generation. No Provider I/O or secret response. Up to 10000 records per bounded blocking query; capacity returns 503 without partial totals. Historical/missing identity stays empty. Status describes administration/authentication, not runtime health or entitlement. A provider filter selects configured credentials only. */
        get: operations["listAccountInventory"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/credentials": {
        parameters: {
            query?: {
                limit?: number;
                cursor?: string;
                upstream_id?: string;
                q?: string;
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        /** @description Complete managed inventory, including unbound and disabled resources. Literal server-side search over identity, owner and kind/adapter. Cursor binds filters, resource, configuration revision and resource-audit sequence (including credential rotations). Restart enumeration on 409. Identity metadata is projected server-side from at most 100 sealed credentials in the same storage snapshot; tokens never enter the response. Search remains over technical identity, owner and kind, not encrypted profile fields. Runs in bounded blocking admission. */
        get: operations["listManagedCredentials"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/endpoints": {
        parameters: {
            query?: {
                limit?: number;
                cursor?: string;
                upstream_id?: string;
                q?: string;
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        /** @description Complete managed inventory, including unbound and disabled resources. Literal server-side search over identity, owner and kind/adapter. Cursor binds filters, resource, configuration revision and resource-audit sequence (including credential rotations). Restart enumeration on 409. Identity metadata is projected server-side from at most 100 sealed credentials in the same storage snapshot; tokens never enter the response. Search remains over technical identity, owner and kind, not encrypted profile fields. Runs in bounded blocking admission. */
        get: operations["listManagedEndpoints"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/routes": {
        parameters: {
            query?: {
                limit?: number;
                cursor?: string;
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        /** @description Complete bounded enumeration including unbound drafts. Cursor binds resource kind, configuration ID and revision; changes return 409 and require restarting enumeration. No secrets are returned. */
        get: operations["listRoutes"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/route-candidates": {
        parameters: {
            query?: {
                limit?: number;
                cursor?: string;
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        /** @description Complete bounded enumeration including unbound drafts. Cursor binds resource kind, configuration ID and revision; changes return 409 and require restarting enumeration. No secrets are returned. */
        get: operations["listRouteCandidates"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/model-aliases": {
        parameters: {
            query?: {
                limit?: number;
                cursor?: string;
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        /** @description Complete bounded enumeration including unbound drafts. Cursor binds resource kind, configuration ID and revision; changes return 409 and require restarting enumeration. No secrets are returned. */
        get: operations["listModelAliases"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/routes/{route_id}": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                route_id: components["parameters"]["RouteId"];
            };
            cookie?: never;
        };
        get: operations["getRoute"];
        put?: never;
        post?: never;
        delete: operations["deleteRoute"];
        options?: never;
        head?: never;
        patch: operations["updateRoute"];
        trace?: never;
    };
    "/admin/routes/{route_id}/candidates": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                route_id: components["parameters"]["RouteId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["createRouteCandidate"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/routes/{route_id}/candidates/{candidate_id}": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                route_id: components["parameters"]["RouteId"];
                candidate_id: components["schemas"]["Id"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        /** @description Delete one Candidate in an exact draft revision, retaining the Route and its grants. Revalidate the Route before publication. */
        delete: operations["deleteRouteCandidate"];
        options?: never;
        head?: never;
        /** @description Replace mutable Candidate fields in an exact draft revision. Body id must match the path; owning Route is immutable. Mutation and audit commit atomically. */
        patch: operations["updateRouteCandidate"];
        trace?: never;
    };
    "/admin/routes/{route_id}/validate": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                route_id: components["parameters"]["RouteId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["validateRoute"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/routes/{route_id}/explain": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                route_id: components["parameters"]["RouteId"];
            };
            cookie?: never;
        };
        get: operations["explainRoute"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/access-groups": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        get: operations["listAccessGroups"];
        put?: never;
        post: operations["createAccessGroup"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/access-groups/{access_group_id}": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                access_group_id: components["parameters"]["AccessGroupId"];
            };
            cookie?: never;
        };
        get: operations["getAccessGroup"];
        put?: never;
        post?: never;
        delete: operations["deleteAccessGroup"];
        options?: never;
        head?: never;
        patch: operations["updateAccessGroup"];
        trace?: never;
    };
    "/admin/access-groups/{access_group_id}/routes": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                access_group_id: components["parameters"]["AccessGroupId"];
            };
            cookie?: never;
        };
        get: operations["listAccessGroupRoutes"];
        put?: never;
        post: operations["grantAccessGroupRoute"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/client-keys": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        get: operations["listClientKeys"];
        put?: never;
        post: operations["issueClientKey"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/client-keys/{client_key_id}": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                client_key_id: components["parameters"]["ClientKeyId"];
            };
            cookie?: never;
        };
        get: operations["getClientKey"];
        put?: never;
        post?: never;
        delete: operations["revokeClientKey"];
        options?: never;
        head?: never;
        patch: operations["updateClientKey"];
        trace?: never;
    };
    "/admin/upstreams/{upstream_id}/codex-authorization/start": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Revision-bound first Codex authorization in an owned draft; no placeholder account or browser Provider secret. */
        post: operations["startCodexEnrollment"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/upstreams/{upstream_id}/codex-authorization/cancel": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Revision-bound first Codex authorization in an owned draft; no placeholder account or browser Provider secret. */
        post: operations["cancelCodexEnrollment"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/upstreams/{upstream_id}/codex-authorization/callback": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Revision-bound first Codex authorization in an owned draft; no placeholder account or browser Provider secret. */
        post: operations["completeCodexEnrollment"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/upstreams/{upstream_id}/claude-authorization/start": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Revision-bound Claude first authorization or explicit reauthorization in an owned draft; provider secrets remain on the server. */
        post: operations["startClaudeEnrollment"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/upstreams/{upstream_id}/claude-authorization/cancel": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Revision-bound Claude first authorization or explicit reauthorization in an owned draft; provider secrets remain on the server. */
        post: operations["cancelClaudeEnrollment"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/upstreams/{upstream_id}/claude-authorization/callback": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Revision-bound Claude first authorization or explicit reauthorization in an owned draft; provider secrets remain on the server. */
        post: operations["completeClaudeEnrollment"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/credentials/{credential_id}/oauth/start": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["startCredentialOAuth"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/credentials/{credential_id}/oauth/status": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        get: operations["getCredentialOAuthStatus"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/credentials/{credential_id}/oauth/cancel": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["cancelCredentialOAuth"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/credentials/{credential_id}/oauth/callback": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["completeCredentialOAuth"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/credentials/{credential_id}/oauth/refresh": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["refreshCredentialOAuth"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/credentials/{credential_id}/export": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["exportCredential"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/credentials/{credential_id}/metadata": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        get: operations["getCredentialMetadata"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/operations/billing-processing": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Read the process-local billing worker status without config headers or storage I/O. Watermarks are from one successful durable observation. On failure they retain their original observed_at_ms; null means unobserved, not zero. */
        get: operations["getBillingProcessingStatus"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/models/effective": {
        parameters: {
            query?: {
                access_group_id?: components["schemas"]["Id"];
                client_key_id?: components["schemas"]["Id"];
                limit?: number;
                cursor?: string;
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        /** @description Serving-only exact authorized models and safe provenance. Supply exactly one existing Access Group ID or Client Key ID, never a secret. Cursor binds authorization context and full projection; changes return 409. projection_id is a safe projection fingerprint, not a catalog revision. No draft revision ETag is emitted. */
        get: operations["listEffectiveModels"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/catalog/status": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        /** @description Protected exact Endpoint/Credential model-catalog status. Includes durable last-success version, freshness, refresh-due state, retained model count, and only a bounded safe failure class; no model IDs, URLs, response bodies, or credentials are returned. */
        get: operations["getCatalogStatus"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/runtime/availability": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        get: operations["getRuntimeAvailability"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/runtime/quota/reset": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["requestQuotaRecovery"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/operations/account-pools": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        get: operations["listOperationalAccountPools"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/operations/provider-egress-status": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        /** @description Read-only, provider-specific runtime projection. Egress, session, and clearance are independent domains; this operation never starts recovery or refresh and never exposes endpoint, credential, cookie, proxy, or request values. */
        get: operations["listProviderEgressStatus"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/compatible-proxy-pools": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        get: operations["listCompatibleProxyPools"];
        put?: never;
        post: operations["createCompatibleProxyPool"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/compatible-proxy-pools/{pool_id}": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                pool_id: components["parameters"]["CompatibleProxyPoolId"];
            };
            cookie?: never;
        };
        get: operations["getCompatibleProxyPool"];
        put?: never;
        post?: never;
        delete: operations["deleteCompatibleProxyPool"];
        options?: never;
        head?: never;
        patch: operations["updateCompatibleProxyPool"];
        trace?: never;
    };
    "/admin/compatible-proxy-nodes": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        get: operations["listCompatibleProxyNodes"];
        put?: never;
        post: operations["createCompatibleProxyNode"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/compatible-proxy-nodes/{node_id}": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                node_id: components["parameters"]["CompatibleProxyNodeId"];
            };
            cookie?: never;
        };
        get: operations["getCompatibleProxyNode"];
        put?: never;
        post?: never;
        delete: operations["deleteCompatibleProxyNode"];
        options?: never;
        head?: never;
        patch: operations["updateCompatibleProxyNode"];
        trace?: never;
    };
    "/admin/compatible-egress-bindings": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        get: operations["listCompatibleEgressBindings"];
        put?: never;
        post: operations["createCompatibleEgressBinding"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/compatible-egress-bindings/{endpoint_id}/{credential_id}": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                endpoint_id: components["parameters"]["EndpointId"];
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        get: operations["getCompatibleEgressBinding"];
        put?: never;
        post?: never;
        delete: operations["deleteCompatibleEgressBinding"];
        options?: never;
        head?: never;
        patch: operations["updateCompatibleEgressBinding"];
        trace?: never;
    };
    "/admin/operations/channel-pin": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["executeChannelPin"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/operations/provider-account-pools": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listProviderAccountPools"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/operations/provider-account-pools/actions": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["applyProviderAccountPoolAction"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/operations/provider-account-pools/failures": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        get: operations["listProviderAccountFailures"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/operations/usage": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Production reads stream complete filtered usage groups. Newly issued opaque cursors pin the durable event ordinal upper bound; legacy cursors without that bound remain accepted and start a new snapshot. observed_through_ms covers the full filtered snapshot, not only the returned page. By default conflicting Usage lineages fail closed. allow_partial=true explicitly excludes immutable conflicting Usage groups, retaining all source events. Missing lineage or malformed data still fails closed. Exclusion counts cover the full filtered snapshot before pagination; a nonzero count means incomplete totals, never zero consumption. */
        get: operations["listOperationalUsage"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/operations/billing": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listOperationalBilling"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/billing/catalogs": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        get: operations["listBillingCatalogs"];
        put?: never;
        /** @description Appends an immutable global catalog under the selected draft revision. The shared 256-catalog management capacity is checked atomically with insertion, revision and audit. Capacity exhaustion returns 400 invalid_management_request without changing existing catalogs, the draft revision or audit; duplicate identities remain 409. */
        post: operations["importBillingCatalog"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/billing/catalogs/{catalog_version_id}/rollback": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                catalog_version_id: components["parameters"]["BillingCatalogVersionId"];
            };
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Appends an immutable global catalog under the selected draft revision. The shared 256-catalog management capacity is checked atomically with insertion, revision and audit. Capacity exhaustion returns 400 invalid_management_request without changing existing catalogs, the draft revision or audit; duplicate identities remain 409. */
        post: operations["rollbackBillingCatalog"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/billing/routing-price-policy": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        get: operations["getRoutingPricePolicy"];
        put: operations["setRoutingPricePolicy"];
        post?: never;
        delete: operations["clearRoutingPricePolicy"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/requests/{request_id}/attempts": {
        parameters: {
            query?: never;
            header?: never;
            path: {
                request_id: components["parameters"]["RequestId"];
            };
            cookie?: never;
        };
        get: operations["listRequestAttempts"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/observability/metrics": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Read-only bounded Prometheus text exposition of background telemetry and recording health. gateway_recording_accepting_requests reports capacity to attempt confirmed admission; each request still waits for durable commit. gateway_recording_state: 0 starting, 1 ready, 2 storage unavailable, 3 closed. Also exposes pending_required, last_commit_ms (0 means unobserved), confirmation_failures_total and recovered_unknown. Also exposes gateway_recording_queue_capacity/used/remaining. gateway_storage_* gauges report observed (0 means no successful sample yet), observed_at_ms, collection_failed, database_bytes, wal_bytes, available_bytes, total_bytes, disk_low and wal_high from a minute-scale background sample. Unknown sizes are omitted, failures retain the last timestamp; disk_low means less than 1 GiB or 5 percent available, wal_high means over 256 MiB. These are warnings, not deletion or admission policies. Billing materialization health remains in operations/billing-processing. Carries no request-scoped or target-scoped label. */
        get: operations["getObservabilityMetrics"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/audit-events": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listManagementAuditEvents"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/backups/preflight": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["previewBackup"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/restores/preflight": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["previewRestore"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/restores": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["restoreBackup"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/native-accounts/{account_id}/identity": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Explicit bounded provider identity read with credential revision CAS. Uses management authentication and CSRF; never rotates a token or calls inference. */
        post: operations["refreshNativeAccountIdentity"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/native-accounts/{account_id}/deletion-impact": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Review exact native account deletion impact
         * @description Reads selected configuration and bounded native metadata without opening credentials or contacting Providers. Shows matching native connections and routed models. Remaining accounts are observed metadata, never a replacement guarantee. Reviewed DELETE rechecks the token and target revision.
         */
        get: operations["getNativeAccountDeletionImpact"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/native-accounts/{account_id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        /** @description Remove one exact authorization, retaining request, billing and independent maintenance history. No automatic cascade to unrelated accounts. */
        delete: operations["deleteNativeAccount"];
        options?: never;
        head?: never;
        /** @description Administrative enable/disable under exact account revision CAS. The durable receipt distinguishes saved changes from runtime application. */
        patch: operations["updateNativeAccount"];
        trace?: never;
    };
    "/admin/native-accounts/{account_id}/credential": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        /** @description Replace Web or Console SSO only. Collect identity at submission; reject a different observed account, apply exact revision CAS and never return material. */
        put: operations["replaceNativeAccountCredential"];
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/native-accounts/{account_id}/usage": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Read bounded native account quota at the supplied account revision */
        get: operations["getNativeAccountUsage"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/native-accounts/{account_id}/audit": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Last 100 append-only account maintenance records, including removed accounts. */
        get: operations["listNativeAccountAudit"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/operations/runtime/apply": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Reconcile already persisted active configuration and native accounts. Does not publish a draft or invoke Provider transports during compilation. Configured background workers keep their existing policies. Failed reconciliation pauses new inference admission while keeping management and in-flight requests available. */
        post: operations["applyRuntimeConfiguration"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/system": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Safe metadata of the running binary and elapsed service uptime. No environment values, paths, credentials or provider requests. Unavailable without deployment attachment. */
        get: operations["getSystemInformation"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/catalog/models": {
        parameters: {
            query: {
                endpoint_id: components["schemas"]["Id"];
                credential_id: components["schemas"]["Id"];
                limit?: number;
                cursor?: string;
                q?: string;
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        /** @description Read saved target-local model evidence, including models not configured in routes. No Provider calls or secrets. Reads under bounded blocking admission with storage-side keyset filtering and LIMIT. Cursor binds configuration revision, target identity, snapshot version and observation time; changed sources return 409. Missing successful target returns 404; successful empty catalogs return an empty page. Expired evidence remains inspectable and never grants access. Retained successful omissions are identified explicitly. */
        get: operations["listCatalogModels"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/catalog/refresh": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Explicit metadata-only refresh for one exact active Endpoint/Credential. Server-owned credentials and admitted egress; never opens models or grants access. Bounded request, old successful evidence retained on failure. */
        post: operations["refreshCatalogModels"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/requests": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Snapshot-bound accepted requests. Historical missing terminal/timing evidence stays unknown. No bodies or secrets are included. */
        get: operations["listRequests"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/requests/summary": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Complete request-level summary and time buckets over the same event snapshot. Retries are counted separately. Success rate excludes unknown records and includes cancellations in its denominator. Percentiles use nearest rank. */
        get: operations["summarizeRequests"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/account-authorization/{session_id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Authenticated actor- and configuration-bound process-local readback only. No Provider I/O, retry or credential write. Terminal receipts retain for 15 minutes, bounded to 512 records; missing or after restart is unconfirmed. Success revision is the credential persistence receipt, not configuration application. */
        get: operations["getAccountAuthorizationReceipt"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/config-versions/{config_version_id}/account-restoration": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Reads all locally deleted account identities that this exact historical graph would restore. Performs no lifecycle write or upstream call. The digest pins target, active revisions and deletion audit clock. */
        get: operations["getAccountRestorationReview"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/admin/credentials/{credential_id}/deletion-impact": {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Exact reviewed impact digest. A stale supplied review is rejected before deletion. */
                "X-Deletion-Impact-Review"?: string;
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        /** @description Read-only exact local removal impact. Removes only this credential and its bindings, retaining shared endpoints/routes/client grants, billing and request history. Does not revoke the upstream account. */
        get: operations["getCredentialDeletionImpact"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
}
export type webhooks = Record<string, never>;
export interface components {
    schemas: {
        AdministratorLoginInput: {
            username: string;
            /** Format: password */
            password: string;
        };
        AdministratorPasswordInput: {
            /** Format: password */
            current_password: string;
            /**
             * Format: password
             * @description At most 512 UTF-8 bytes. Not all whitespace or identical to current password; never trimmed.
             */
            new_password: string;
        };
        AdministratorSession: {
            username: string;
            session_token: string;
            csrf_token: string;
            expires_at_ms: number;
            password_change_required: boolean;
        };
        ConfigurationDiffVersion: {
            id: components["schemas"]["Id"];
            revision: string;
        };
        ConfigurationResourceChange: {
            resource_kind: string;
            /** @description JSON primary-key identity tuple; excludes configuration ID. */
            resource_key: string;
            /** @enum {string} */
            change: "added" | "removed" | "changed";
            changed_fields: string[];
        };
        ConfigurationDiffPage: {
            base: components["schemas"]["ConfigurationDiffVersion"];
            target: components["schemas"]["ConfigurationDiffVersion"];
            items: components["schemas"]["ConfigurationResourceChange"][];
            next_cursor: string | null;
        };
        ResourceAuditPage: {
            items: {
                id: string;
                action: string;
                actor: string;
                config_version_id: string;
                resource_kind: string;
                resource_id: string;
                occurred_at_ms: number;
            }[];
            next_before_id: string | null;
        };
        Id: string;
        ConfigVersionInput: {
            id: components["schemas"]["Id"];
            parent_id?: components["schemas"]["Id"] | null;
            description: string;
        };
        ConfigVersion: {
            id: components["schemas"]["Id"];
            parent_id?: components["schemas"]["Id"] | null;
            /** @enum {string} */
            status: "draft" | "active" | "archived";
            revision: string;
            created_at_ms: number;
            description: string;
        };
        EgressPolicyInput: {
            id: components["schemas"]["Id"];
            name: string;
            allowed_schemes: "https"[];
            allowed_hosts: string[];
            allowed_ports: number[];
            allowed_cidrs: string[];
            /** @enum {string} */
            redirect_mode: "deny" | "revalidate";
            max_redirects: number;
        };
        EgressPolicy: components["schemas"]["EgressPolicyInput"];
        UpstreamInput: {
            id: components["schemas"]["Id"];
            name: string;
            kind: string;
            enabled: boolean;
            tags: string[];
            egress_policy_id?: components["schemas"]["Id"] | null;
        };
        Upstream: components["schemas"]["UpstreamInput"];
        EndpointInput: {
            id: components["schemas"]["Id"];
            adapter_id: string;
            api_format: string;
            /** Format: uri */
            base_url: string;
            inference_path: string;
            models_path?: string | null;
            /** @enum {string} */
            transport: "https";
            enabled: boolean;
        };
        Endpoint: {
            id: components["schemas"]["Id"];
            upstream_id: components["schemas"]["Id"];
            adapter_id: string;
            api_format: string;
            /** Format: uri */
            base_url: string;
            inference_path: string;
            models_path?: string | null;
            /** @enum {string} */
            transport: "https";
            enabled: boolean;
        };
        CredentialInput: {
            id: components["schemas"]["Id"];
            kind: string;
            secret: string;
            /** @enum {string} */
            status: "active" | "disabled" | "revoked";
        };
        Credential: {
            id: components["schemas"]["Id"];
            upstream_id: components["schemas"]["Id"];
            kind: string;
            /** @enum {string} */
            status: "active" | "disabled" | "revoked" | "cooling" | "unauthorized";
            revision: number;
            secret_present: boolean;
        };
        KimiMetadataFailure: {
            /** @enum {string} */
            code: "not_observed" | "egress_denied" | "transport" | "timeout" | "http" | "response_too_large" | "invalid_json" | "empty_response" | "unrecognized_response";
            /** @description Only supplied for an upstream HTTP rejection. No body, headers, or URL. */
            status?: number;
        };
        KimiAccountObservation: {
            /** Format: int64 */
            observed_at_ms: number;
            profile_available: boolean;
            quota_available: boolean;
            identity: components["schemas"]["AccountDisplayIdentity"];
            plan: string | null;
            quota_windows: {
                /** @enum {string} */
                window: "limit_5h" | "limit_7d" | "limit_month_total" | "limit_month_code";
                used_ratio: number;
                reset_at: string | null;
            }[];
            profile_error?: components["schemas"]["KimiMetadataFailure"] | null;
            quota_error?: components["schemas"]["KimiMetadataFailure"] | null;
        };
        AccountQuotaObservation: {
            /** @enum {string} */
            source: "codex" | "claude" | "kiro" | "grok_build" | "grok_console" | "grok_web";
            /** Format: int64 */
            observed_at_ms: number;
            windows: {
                name: string;
                used_percent: number | null;
                reset_at: string | null;
                /** Format: int64 */
                reset_at_ms: number | null;
                duration_seconds: number | null;
                used?: number | null;
                limit?: number | null;
                unit?: string | null;
            }[];
            email?: string | null;
            plan?: string | null;
            partial?: boolean;
        };
        CredentialMetadata: {
            credential_id: components["schemas"]["Id"];
            kind: string;
            revision: number;
            plan?: string | null;
            quota?: string | null;
            platform?: string | null;
            email?: string | null;
            /** @enum {string|null} */
            source_format?: "cpa" | "sub2api" | "direct_oauth" | null;
            kimi?: components["schemas"]["KimiAccountObservation"] | null;
            /** @enum {string|null} */
            kimi_error?: "not_connected" | "configuration_changed" | "unavailable" | "busy" | "timeout" | null;
            live_quota?: components["schemas"]["AccountQuotaObservation"] | null;
            /** @enum {string|null} */
            live_quota_error?: "not_implemented_or_not_connected" | "configuration_changed" | "unavailable" | "unauthorized" | "forbidden" | "egress_denied" | "invalid_response" | "busy" | null;
        };
        KimiDeviceAuthorization: {
            /** @enum {string} */
            state: "pending" | "completed" | "cancelled" | "denied" | "expired" | "failed";
            session_id?: string;
            user_code?: string;
            /** Format: uri */
            verification_uri?: string;
            expires_at_ms?: number;
            interval_ms?: number;
            credential_id?: components["schemas"]["Id"];
        };
        KimiPreparedTarget: {
            upstream_id: components["schemas"]["Id"];
            endpoint_id: components["schemas"]["Id"];
            prepared: boolean;
        };
        BindingInput: {
            credential_id: components["schemas"]["Id"];
            enabled: boolean;
            priority: number;
            weight: number;
            concurrency: number;
        };
        Binding: {
            endpoint_id: components["schemas"]["Id"];
            upstream_id: components["schemas"]["Id"];
            credential_id: components["schemas"]["Id"];
            enabled: boolean;
            priority: number;
            weight: number;
            concurrency: number;
        };
        CompatibleProxyPoolInput: {
            id: components["schemas"]["Id"];
            upstream_id: components["schemas"]["Id"];
            name: string;
            enabled: boolean;
        };
        CompatibleProxyPool: components["schemas"]["CompatibleProxyPoolInput"];
        CompatibleProxyNodeCreateInput: {
            id: components["schemas"]["Id"];
            upstream_id: components["schemas"]["Id"];
            pool_id?: components["schemas"]["Id"] | null;
            name: string;
            /** Format: uri */
            proxy_endpoint: string;
            enabled: boolean;
            weight: number;
            maximum_concurrency: number;
        };
        CompatibleProxyNodeUpdateInput: {
            id: components["schemas"]["Id"];
            upstream_id: components["schemas"]["Id"];
            pool_id?: components["schemas"]["Id"] | null;
            name: string;
            /** @description Omitted or null preserves the existing sealed endpoint; a string rotates it. */
            proxy_endpoint?: string | null;
            enabled: boolean;
            weight: number;
            maximum_concurrency: number;
        };
        CompatibleProxyNode: {
            id: components["schemas"]["Id"];
            upstream_id: components["schemas"]["Id"];
            pool_id: components["schemas"]["Id"] | null;
            name: string;
            enabled: boolean;
            weight: number;
            maximum_concurrency: number;
            proxy_configured: boolean;
        };
        CompatibleEgressBindingInput: {
            endpoint_id: components["schemas"]["Id"];
            credential_id: components["schemas"]["Id"];
            /** @enum {string} */
            target_kind: "direct" | "fixed_proxy" | "proxy_pool";
            target_id?: components["schemas"]["Id"] | null;
            /** @enum {string} */
            failure_scope: "endpoint" | "credential" | "egress_node";
            /** @enum {string} */
            stickiness: "none" | "credential" | "credential_and_egress";
            pre_submit_max_attempts: number;
        };
        CompatibleEgressBinding: components["schemas"]["CompatibleEgressBindingInput"];
        EndpointTestInput: {
            /** @enum {string} */
            mode: "non_streaming" | "sse";
        };
        ChannelPinInput: {
            provider_id: components["schemas"]["Id"];
            channel_id: components["schemas"]["Id"];
            route_id: components["schemas"]["Id"];
            credential_id: components["schemas"]["Id"];
            requested_model: string;
            /** @enum {string} */
            protocol: "openai_chat_completions" | "openai_responses" | "anthropic_messages";
            /** @enum {string} */
            mode: "json" | "sse";
            /** @description Optional Key ID for its current permission graph; this does not verify possession of its secret. */
            client_key_id: components["schemas"]["Id"];
            credential_revision?: number;
        };
        EndpointTest: {
            /** @enum {string} */
            outcome: "pass" | "rejected" | "transport_failed" | "protocol_failed";
            /** @enum {string} */
            status_class?: "2xx" | "4xx" | "5xx" | "other";
            canonical_lifecycle?: boolean;
        };
        ChannelPin: {
            request_id: components["schemas"]["Id512"];
            config_version_id: components["schemas"]["Id"];
            config_revision: number;
            provider_id: components["schemas"]["Id"];
            channel_id: components["schemas"]["Id"];
            route_id: components["schemas"]["Id"];
            credential_id: components["schemas"]["Id"];
            requested_model: string;
            /** @enum {string} */
            protocol: "openai_chat_completions" | "openai_responses" | "anthropic_messages";
            /** @enum {string} */
            mode: "json" | "sse";
            /** @enum {string} */
            outcome: "succeeded" | "rejected" | "failed";
            upstream_sent: boolean;
            attempt_count: number;
            response_started: boolean;
            observed_at_ms: number;
            /** @enum {string} */
            stage?: "request_conversion" | "egress_admission" | "http_transport" | "http_status" | "content_type" | "body_read" | "decoder" | "sse_bootstrap";
            client_key_id?: string | null;
            credential_revision?: number;
            connection_revision?: number;
            runtime_build?: string | null;
            /** @enum {string} */
            permission_basis?: "management_key_projection" | "administrator_diagnostic";
            /** @description Nonsecret identity of this management process. Old invocation evidence is historical after restart. */
            server_instance?: string;
            runtime_credential_revision?: number;
            /** @description Actual attempt facts captured from the selected lease and transport. Absence means unknown; management and native account revisions remain separate. */
            execution?: components["schemas"]["ChannelPinExecution"];
        };
        PublicModelInput: {
            id: components["schemas"]["Id"];
            model_name: string;
            /** @enum {string} */
            status: "active" | "disabled";
            display_name: string;
            capabilities: {
                [key: string]: boolean;
            };
        };
        PublicModel: components["schemas"]["PublicModelInput"];
        AliasInput: {
            alias: string;
        };
        BillingProcessingStatus: {
            /** @enum {string} */
            state: "disabled" | "starting" | "current" | "catching_up" | "needs_repair" | "failed" | "stopped";
            observed_at_ms: number | null;
            source_ordinal: number | null;
            checkpoint_ordinal: number | null;
            checkpoint_updated_at_ms: number | null;
            unresolved_failures: number | null;
            /** @enum {string|null} */
            failure_code: "batch_unavailable" | null;
            /** @description Subset of unresolved_failures with immutable conflicting Usage payloads. Retained for investigation and excluded from automatic retries; null before observation. Does not imply repair or deletion. */
            quarantined_failures: number | null;
        };
        ModelCatalogEvidence: {
            credential_id: components["schemas"]["Id"];
            version: number;
            observed_at_ms: number;
            stale_at_ms: number;
            expires_at_ms: number;
            catalog_eligible: boolean;
        };
        ExecutionEgress: {
            /** @constant */
            kind: "unknown";
        } | {
            /** @constant */
            kind: "not_selected";
        } | {
            /** @constant */
            kind: "direct";
        } | {
            /** @constant */
            kind: "process_proxy";
            transport_fingerprint: string;
        } | {
            /** @constant */
            kind: "configured_proxy";
            target_id: components["schemas"]["Id512"];
            node_id: components["schemas"]["Id512"];
            config_version_id: components["schemas"]["Id512"];
            config_revision: number;
        };
        AttemptExecutionIdentity: {
            /** @enum {string} */
            channel: "openai-compatible" | "anthropic-compatible" | "codex" | "claude" | "grok.build" | "grok.console" | "grok.web" | "grok.official" | "kimi-coding" | "kimi-api" | "kiro";
            credential_revision: number;
            config_version_id: components["schemas"]["Id512"];
            config_revision: number;
            egress: components["schemas"]["ExecutionEgress"];
            capabilities: components["schemas"]["EffectiveCapabilities"];
        };
        ChannelPinExecution: {
            attempt_id: components["schemas"]["Id512"];
            route_candidate_id: components["schemas"]["Id512"];
            upstream_model: string;
            identity: components["schemas"]["AttemptExecutionIdentity"];
        };
        CapabilityEvidence: {
            /** @enum {string} */
            source: "serving_snapshot";
            config_version_id: components["schemas"]["Id"];
            config_revision: number;
            capabilities: components["schemas"]["EffectiveCapabilities"];
        };
        /** @description Complete compiled gateway declaration. Missing capability_evidence means unknown. Continuation includes declared StoredResponse, Compaction, WebSocketSession, or Grok Build OwnedReasoning; protocol applicability is evaluated separately. */
        EffectiveCapabilities: {
            reasoning: boolean;
            parallel: boolean;
            stored: boolean;
            continuation: boolean;
            compact: boolean;
            websocket: boolean;
        };
        EffectiveModelSource: {
            candidate_id: components["schemas"]["Id"];
            endpoint_id: components["schemas"]["Id"];
            upstream_id: components["schemas"]["Id"];
            api_format: string;
            /** @enum {string} */
            catalog_admission: "manual" | "fresh" | "stale" | "expired" | "allowed_unlisted";
            /** @description Empty when no durable discovery observation is pinned. Never inferred from entitlement. */
            catalog_evidence: components["schemas"]["ModelCatalogEvidence"][];
            capability_evidence?: components["schemas"]["CapabilityEvidence"];
        };
        EffectiveModel: {
            id: string;
            public_model_id: components["schemas"]["Id"];
            public_model_name: string;
            route_id: components["schemas"]["Id"];
            sources: components["schemas"]["EffectiveModelSource"][];
        };
        EffectiveModelPage: {
            config_version: components["schemas"]["Id"];
            access_group_id: components["schemas"]["Id"];
            client_key_id: components["schemas"]["Id"] | null;
            projection_id: string;
            observed_at_ms: number;
            items: components["schemas"]["EffectiveModel"][];
            next_cursor: string | null;
        };
        RouteListItem: {
            id: components["schemas"]["Id"];
            public_model_id: components["schemas"]["Id"];
            /** @enum {string} */
            policy: "smooth_weighted_round_robin" | "round_robin" | "priority_failover";
            max_attempts: number;
            bootstrap_timeout_ms: number;
        };
        /** @description Human-readable identity evidence from stored credential metadata. Nullable; never inferred from a digest, import batch or opaque subject. Display only, not verified authorization evidence. */
        AccountDisplayIdentity: {
            email: string | null;
            phone: string | null;
            username: string | null;
        };
        ManagedCredentialItem: {
            credential: components["schemas"]["Credential"];
            binding_count: number;
            identity: components["schemas"]["AccountDisplayIdentity"];
            /** @enum {string} */
            category: "api" | "codex" | "claude" | "kimi" | "kiro";
            provider: string;
            connections: {
                id: string;
                api_format: string;
                enabled: boolean;
                host: string | null;
            }[];
            plan: string | null;
            plan_source: string | null;
            authentication: ("api_key" | "oauth") | null;
        };
        UnifiedAccountItem: {
            id: components["schemas"]["Id"];
            native: boolean;
            identity: components["schemas"]["AccountDisplayIdentity"];
            name: string;
            /** @enum {string} */
            category: "api" | "codex" | "claude" | "kimi" | "kiro" | "grok";
            provider: string;
            /** @enum {string} */
            status: "enabled" | "disabled" | "reauth_required";
            operations: ("details" | "update_credential" | "enable" | "disable" | "remove" | "models" | "reauthorize")[];
            managed: components["schemas"]["ManagedCredentialItem"] | null;
            native_account: {
                id: string;
                /** @enum {string} */
                provider: "grok_build" | "grok_console" | "grok_web";
                auth_status: string;
                enabled: boolean;
                /** Format: int64 */
                revision: number;
                import_batch_id: string;
                identity: components["schemas"]["AccountDisplayIdentity"];
                /** @description Adapter supported IDs only, not upstream observation or account entitlement; null for dynamic-catalog channels. */
                adapter_models?: string[] | null;
            } | null;
            plan: string | null;
            plan_source: string | null;
        };
        UnifiedAccountPage: {
            config_version: components["schemas"]["Id"];
            revision: string;
            total: number;
            category_totals: {
                [key: string]: number;
            };
            items: components["schemas"]["UnifiedAccountItem"][];
            next_cursor: string | null;
            plan_totals: {
                [key: string]: number;
            };
            unobserved_plan_total: number;
        };
        ManagedCredentialPage: {
            config_version: components["schemas"]["Id"];
            revision: string;
            observation_version: string;
            items: components["schemas"]["ManagedCredentialItem"][];
            next_cursor: string | null;
        };
        ManagedEndpointPage: {
            config_version: components["schemas"]["Id"];
            revision: string;
            observation_version: string;
            items: components["schemas"]["Endpoint"][];
            next_cursor: string | null;
        };
        RoutePage: {
            config_version: components["schemas"]["Id"];
            revision: string;
            items: components["schemas"]["RouteListItem"][];
            next_cursor: string | null;
        };
        CandidatePage: {
            config_version: components["schemas"]["Id"];
            revision: string;
            items: components["schemas"]["Candidate"][];
            next_cursor: string | null;
        };
        AliasPage: {
            config_version: components["schemas"]["Id"];
            revision: string;
            items: components["schemas"]["Alias"][];
            next_cursor: string | null;
        };
        Alias: {
            alias: string;
            public_model_id: components["schemas"]["Id"];
        };
        RouteInput: {
            id: components["schemas"]["Id"];
            /** @enum {string} */
            policy: "smooth_weighted_round_robin";
            max_attempts: number;
            bootstrap_timeout_ms: number;
        };
        Route: {
            id: components["schemas"]["Id"];
            public_model_id: components["schemas"]["Id"];
            /** @enum {string} */
            policy: "smooth_weighted_round_robin";
            max_attempts: number;
            bootstrap_timeout_ms: number;
        };
        CandidateInput: {
            id: components["schemas"]["Id"];
            endpoint_id: components["schemas"]["Id"];
            upstream_model: string;
            /** @enum {string} */
            credential_scope: "all_active";
            /** @enum {string} */
            transform_mode: "passthrough" | "canonical" | "lossless_bridge" | "canonical_bridge";
            enabled: boolean;
            priority: number;
            weight: number;
            capability_override: {
                [key: string]: boolean;
            };
        };
        Candidate: {
            id: components["schemas"]["Id"];
            route_id: components["schemas"]["Id"];
            endpoint_id: components["schemas"]["Id"];
            upstream_model: string;
            /** @enum {string} */
            credential_scope: "all_active";
            /** @enum {string} */
            transform_mode: "passthrough" | "canonical" | "lossless_bridge" | "canonical_bridge";
            enabled: boolean;
            priority: number;
            weight: number;
            capability_override: {
                [key: string]: boolean;
            };
        };
        AccessGroupInput: {
            id: components["schemas"]["Id"];
            name: string;
            /** @enum {string} */
            status: "active" | "disabled";
            limits: {
                [key: string]: number;
            };
        };
        AccessGroup: components["schemas"]["AccessGroupInput"];
        AccessGroupRouteInput: {
            route_id: components["schemas"]["Id"];
            enabled: boolean;
        };
        AccessGroupRoute: {
            access_group_id: components["schemas"]["Id"];
            route_id: components["schemas"]["Id"];
            enabled: boolean;
        };
        ClientKeyInput: {
            id: components["schemas"]["Id"];
            access_group_id: components["schemas"]["Id"];
            /** @enum {string} */
            status: "active" | "disabled" | "revoked";
            expires_at_ms?: number | null;
        };
        ClientKey: {
            id: components["schemas"]["Id"];
            access_group_id: components["schemas"]["Id"];
            prefix: string;
            /** @enum {string} */
            status: "active" | "disabled" | "revoked";
            expires_at_ms?: number | null;
            /** @description Latest observed start timestamp among persisted terminal requests for this logical key ID across configuration forks; null means unobserved. In-flight and historical records without terminal timestamps do not supply a value. */
            last_request_at_ms?: number | null;
        };
        IssuedClientKey: {
            id: components["schemas"]["Id"];
            access_group_id: components["schemas"]["Id"];
            prefix: string;
            /** @enum {string} */
            status: "active" | "disabled" | "revoked";
            expires_at_ms?: number | null;
            key: string;
        };
        OAuthOperation: {
            credential_id: components["schemas"]["Id"];
            /** @enum {string} */
            state: "pending" | "complete" | "cancelled" | "failed" | "expired";
            expires_at_ms?: number | null;
            authorization_url?: string | null;
            failure_class?: ("session_start_failed" | "session_missing" | "state_mismatch" | "token_exchange_failed" | "provider_rejected" | "persistence_failed") | null;
            /** @description Enrollment-only opaque receipt handle; legacy credential OAuth clients may omit it. */
            session_id?: string;
        };
        OAuthRefreshOperation: {
            credential_id: components["schemas"]["Id"];
            /** @enum {string} */
            state: "complete";
            revision: number;
        };
        CatalogDiff: {
            added: number;
            removed: number;
            unchanged: number;
        };
        CatalogStatus: {
            endpoint_id: components["schemas"]["Id"];
            credential_id: components["schemas"]["Id"];
            /** @enum {string} */
            freshness: "fresh" | "stale" | "expired" | "missing";
            observed_at_ms: number;
            snapshot_version?: number;
            refresh_due?: boolean;
            model_count?: number;
            last_failure_at_ms?: number;
            /** @enum {string} */
            last_failure_class?: "authentication" | "authorization" | "rate_limit" | "transport" | "upstream" | "internal";
            /** @description Last successful discovery time; null when no successful snapshot exists. */
            last_success_at_ms?: number | null;
            /**
             * @description Latest discovery outcome, independent of retained snapshot freshness. Failed refresh retains the prior successful snapshot.
             * @enum {string}
             */
            observation_state?: "succeeded" | "empty" | "failed" | "unsupported" | "missing";
            /** @enum {string} */
            source?: "upstream_catalog";
        }[];
        RuntimeAvailability: {
            endpoint_id: components["schemas"]["Id"];
            credential_id: components["schemas"]["Id"];
            /** @enum {string} */
            availability: "available" | "cooldown" | "circuit_open" | "quota_blocked" | "credential_forbidden" | "recovery_required" | "credential_unauthorized" | "expired";
        }[];
        RuntimeTarget: {
            endpoint_id: components["schemas"]["Id"];
            credential_id: components["schemas"]["Id"];
            upstream_model?: string | null;
        };
        RuntimeAction: {
            /** @enum {string} */
            state: "released" | "recovery_required" | "probe_scheduled" | "rejected";
        };
        ProviderAccountOperatorAction: {
            provider_id: components["schemas"]["Id"];
            channel_id: components["schemas"]["Id"];
            account_id: components["schemas"]["Id"];
            /** @enum {string} */
            action: "cool_down" | "request_recovery";
            upstream_model?: string | null;
            cooldown_ms?: number | null;
        };
        ProviderAccountOperatorActionReceipt: {
            /** @enum {string} */
            state: "released" | "cooling" | "probe_scheduled" | "recovery_required" | "rejected";
            observed_at_ms: number;
            /** @description Whether the exact completed or observed result audit was durably recorded; false does not undo a confirmed local transition. */
            audit_recorded?: boolean;
            cooldown_until_ms: number | null;
        };
        ProviderAccountFailurePage: {
            observed_through_ordinal: number | null;
            items: components["schemas"]["ProviderAccountFailureItem"][];
            next_cursor: string | null;
        };
        ProviderAccountFailureItem: {
            provider_id: components["schemas"]["Id"];
            channel_id: components["schemas"]["Id"];
            account_id: components["schemas"]["Id"];
            request_id: components["schemas"]["Id512"];
            attempt_id: components["schemas"]["Id512"];
            ended_at_ms: number;
            /** @enum {string} */
            error_code: "ClientRequestError" | "ClientUnauthorized" | "RouteNotFound" | "CredentialUnavailable" | "CredentialUnauthorized" | "CredentialForbidden" | "CredentialQuotaExceeded" | "EgressRejected" | "EgressUnavailable" | "ProviderRateLimited" | "ProviderTransient" | "ProviderPermanent" | "TokenCountUnsupported" | "UpstreamProtocolError" | "StreamTruncated" | "RecordingUnavailable" | "InternalError" | "Cancelled";
            /** @enum {string} */
            error_scope: "request" | "credential" | "account" | "model" | "quota_window" | "egress_session" | "egress" | "provider" | "stream" | "internal";
            /** @enum {string} */
            retry_decision: "completed" | "retry_eligible" | "non_retryable" | "retry_closed" | "cancelled" | "infrastructure_failure";
        };
        RouteExplain: {
            route_id: components["schemas"]["Id"];
            price_policy: components["schemas"]["RouteExplainPricePolicy"] | null;
            candidates: {
                candidate_id: components["schemas"]["Id"];
                /** @enum {string} */
                decision: "selected" | "excluded";
                reason?: string;
                /** @enum {string} */
                price_evidence: "dominant" | "equal" | "dominated" | "incomparable" | "unpriced" | "not_evaluated" | "disabled";
            }[];
        };
        RouteExplainPricePolicy: {
            catalog_version_id: components["schemas"]["Id"];
            /** @enum {string} */
            comparison: "rate_dominance_v1";
        };
        ProviderAccountPoolPage: {
            snapshot_id: components["schemas"]["Id"];
            observed_at_ms: number;
            items: components["schemas"]["ProviderAccountPoolItem"][];
            next_cursor: string | null;
        };
        ProviderEgressStatusPage: {
            config_version_id: components["schemas"]["Id"];
            config_revision: number;
            runtime_revision: number;
            snapshot_id: components["schemas"]["Id"];
            sampled_at_ms: number;
            items: components["schemas"]["ProviderEgressStatusItem"][];
            next_cursor: string | null;
        };
        ProviderEgressStatusItem: components["schemas"]["ProviderEgressStatusEgressItem"] | components["schemas"]["ProviderEgressStatusSessionItem"] | components["schemas"]["ProviderEgressStatusClearanceItem"];
        /** @enum {string} */
        ProviderEgressStatusChannelKind: "generic_compatible" | "grok_build" | "grok_console" | "grok_web" | "official_api" | "codex_chatgpt" | "kiro" | "claude_compatible" | "other_compatible";
        /** @enum {string} */
        ProviderEgressStatusTargetKind: "direct" | "named";
        /** @enum {string} */
        ProviderEgressStatusEgressState: "available" | "cooling_down" | "circuit_open" | "probe_due" | "probe_in_flight" | "disabled";
        /** @enum {string} */
        ProviderEgressStatusSessionState: "absent" | "active" | "expired" | "challenge_required" | "invalid";
        /** @enum {string} */
        ProviderEgressStatusClearanceState: "absent" | "fresh" | "expired" | "refresh_required" | "refresh_in_flight" | "invalid";
        ProviderEgressStatusTarget: {
            target_kind: components["schemas"]["ProviderEgressStatusTargetKind"];
            target_id: components["schemas"]["Id"] | null;
        };
        ProviderEgressStatusEgressItem: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            domain: "egress";
            provider_id: components["schemas"]["Id"];
            upstream_id: components["schemas"]["Id"];
            channel_id: components["schemas"]["Id"];
            channel_kind: components["schemas"]["ProviderEgressStatusChannelKind"];
            target_kind: components["schemas"]["ProviderEgressStatusTargetKind"];
            target_id: components["schemas"]["Id"] | null;
            state: components["schemas"]["ProviderEgressStatusEgressState"];
            deadline_ms: number | null;
        };
        ProviderEgressStatusSessionItem: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            domain: "session";
            provider_id: components["schemas"]["Id"];
            upstream_id: components["schemas"]["Id"];
            channel_id: components["schemas"]["Id"];
            channel_kind: components["schemas"]["ProviderEgressStatusChannelKind"];
            credential_id: components["schemas"]["Id"];
            credential_revision: number;
            session_revision: number;
            state: components["schemas"]["ProviderEgressStatusSessionState"];
            expires_at_ms: number | null;
        };
        ProviderEgressStatusClearanceItem: {
            /**
             * @description discriminator enum property added by openapi-typescript
             * @enum {string}
             */
            domain: "clearance";
            provider_id: components["schemas"]["Id"];
            upstream_id: components["schemas"]["Id"];
            channel_id: components["schemas"]["Id"];
            channel_kind: components["schemas"]["ProviderEgressStatusChannelKind"];
            credential_id: components["schemas"]["Id"];
            credential_revision: number;
            session_revision: number;
            target_kind: components["schemas"]["ProviderEgressStatusTargetKind"];
            target_id: components["schemas"]["Id"] | null;
            clearance_revision: number;
            state: components["schemas"]["ProviderEgressStatusClearanceState"];
            expires_at_ms: number | null;
        };
        ProviderAccountPoolItem: {
            /** @description Exact non-secret credential revision loaded by the runtime source; null or absent means lineage unobserved. */
            credential_revision?: number | null;
            /** @description Ordinary runtime credential configuration scope. Native credential revisions use their global account namespace. Missing scope cannot establish current ordinary authorization. */
            config_version_id?: components["schemas"]["Id"] | null;
            provider_id: components["schemas"]["Id"];
            channel_id: components["schemas"]["Id"];
            account_id: components["schemas"]["Id"];
            account_kind: string;
            /** @enum {string} */
            auth_status: "active" | "reauth_required" | "disabled" | "expired";
            /** @enum {string} */
            runtime_status: "available" | "cooling" | "circuit_open" | "quota_blocked" | "unauthorized" | "recovery_in_flight" | "expired";
            enabled: boolean;
            priority: number;
            weight: number;
            max_concurrency: number;
            active_leases: number;
            expires_at_ms: number | null;
            refresh_due_at_ms: number | null;
            quota_sync_due_at_ms: number | null;
            entitlement: components["schemas"]["ProviderAccountEntitlement"] | null;
            /** @description Identity and channel labels; operational IDs remain exact. Native identity observations may be newer than the immutable scheduling snapshot. */
            presentation: components["schemas"]["AccountRuntimePresentation"] | null;
        };
        ProviderAccountEntitlement: {
            /** @enum {string} */
            domain: "grok_build" | "grok_web" | "chatgpt" | "claude";
            /** @enum {string} */
            tier: "unknown" | "free" | "supergrok" | "heavy" | "basic" | "super" | "go" | "plus" | "pro5x" | "pro20x" | "pro" | "max5x" | "max20x";
            /** @enum {string} */
            source: "provider_subscription" | "signed_token" | "imported_metadata";
            /** @enum {string} */
            confidence: "authoritative" | "derived" | "declared";
            observed_at_ms: number;
        } & (({
            /** @enum {unknown} */
            domain?: "grok_build";
            /** @enum {unknown} */
            tier?: "unknown" | "free" | "supergrok" | "heavy";
        } | {
            /** @enum {unknown} */
            domain?: "grok_web";
            /** @enum {unknown} */
            tier?: "unknown" | "basic" | "super" | "heavy";
        } | {
            /** @enum {unknown} */
            domain?: "chatgpt";
            /** @enum {unknown} */
            tier?: "unknown" | "free" | "go" | "plus" | "pro5x" | "pro20x";
        } | {
            /** @enum {unknown} */
            domain?: "claude";
            /** @enum {unknown} */
            tier?: "unknown" | "free" | "pro" | "max5x" | "max20x";
        }) & ({
            /** @enum {unknown} */
            source?: "provider_subscription";
            /** @enum {unknown} */
            confidence?: "authoritative";
        } | {
            /** @enum {unknown} */
            source?: "signed_token";
            /** @enum {unknown} */
            confidence?: "derived";
        } | {
            /** @enum {unknown} */
            source?: "imported_metadata";
            /** @enum {unknown} */
            confidence?: "declared";
        }));
        OperationalAccountPoolPage: {
            config_version_id: components["schemas"]["Id"];
            revision: number;
            items: components["schemas"]["OperationalAccountPoolItem"][];
            next_cursor: string | null;
        };
        OperationalAccountPoolItem: {
            provider_id: components["schemas"]["Id"];
            provider_name: string;
            provider_kind: string;
            provider_enabled: boolean;
            egress_policy_id: components["schemas"]["Id"] | null;
            channel_id: components["schemas"]["Id"];
            adapter_id: string;
            api_format: string;
            /** @enum {string} */
            transport: "http" | "sse" | "websocket";
            channel_enabled: boolean;
            account_id: components["schemas"]["Id"];
            account_kind: string;
            /** @enum {string} */
            account_status: "active" | "cooling" | "unauthorized" | "disabled";
            account_revision: number;
            binding_enabled: boolean;
            configured_enabled: boolean;
            priority: number;
            weight: number;
            concurrency: number;
            route_ids: components["schemas"]["Id"][];
        };
        OperationalTokenMetric: {
            total: number | null;
            /** @enum {string} */
            confidence: "exact" | "partial" | "unknown";
            /** @enum {string} */
            provenance?: "measured" | "estimated" | "unknown";
        };
        OperationalUsagePage: {
            observed_through_ms: number | null;
            items: components["schemas"]["OperationalUsageItem"][];
            next_cursor: string | null;
            /** @description Number of Usage events excluded due to conflicting payloads in the full filtered snapshot, before pagination. Original events are retained. Nonzero means totals are incomplete. */
            excluded_usage_events: number;
            /** @description Number of ambiguous request ID groups excluded in the full filtered snapshot. These are not a count of real requests. */
            excluded_request_groups: number;
        };
        OperationalUsageItem: {
            provider_id: components["schemas"]["Id"];
            channel_id: components["schemas"]["Id"];
            account_id: components["schemas"]["Id"];
            public_model: string;
            /** @enum {string} */
            protocol: "openai_chat_completions" | "openai_responses" | "anthropic_messages";
            client_key_id: components["schemas"]["Id"];
            access_group_id: components["schemas"]["Id"] | null;
            request_count: number;
            usage_observations: number;
            input_tokens: components["schemas"]["OperationalTokenMetric"];
            output_tokens: components["schemas"]["OperationalTokenMetric"];
            reasoning_tokens: components["schemas"]["OperationalTokenMetric"];
            cache_read_tokens: components["schemas"]["OperationalTokenMetric"];
            cache_creation_tokens: components["schemas"]["OperationalTokenMetric"];
            cached_tokens: components["schemas"]["OperationalTokenMetric"];
            observed_at_ms: number;
            cost_microunits: number | null;
            /** @enum {string} */
            cost_confidence: "unpriced";
        };
        OperationalBillingPage: {
            snapshot_ledger_id: number | null;
            items: components["schemas"]["OperationalBillingItem"][];
            summary: components["schemas"]["OperationalBillingSummary"];
            next_cursor: string | null;
        };
        OperationalBillingItem: {
            ledger_id: number;
            request_id: components["schemas"]["Id512"];
            response_id: components["schemas"]["Id512"];
            provider_id: components["schemas"]["Id"];
            channel_id: components["schemas"]["Id"];
            account_id: components["schemas"]["Id"];
            model: string;
            input_tokens: number | null;
            output_tokens: number | null;
            reasoning_tokens: number | null;
            cache_read_tokens: number | null;
            cache_creation_tokens: number | null;
            cached_tokens: number | null;
            /** @enum {string} */
            usage_provenance?: "measured" | "estimated" | "unknown";
            /** @enum {string} */
            input_accounting?: "inclusive" | "exclusive" | "unknown";
            occurred_at_ms: number;
            catalog_version_id: components["schemas"]["Id"] | null;
            cost_microunits: number | null;
            /** @enum {string} */
            cost_confidence: "exact" | "partial" | "unknown" | "unpriced";
        };
        OperationalBillingSummary: {
            records: number;
            exact_records: number;
            partial_records: number;
            unknown_records: number;
            unpriced_records: number;
            known_cost_microunits: number | null;
        };
        BillingCatalogImportInput: {
            catalog_version_id: components["schemas"]["Id"];
            effective_at_ms: number;
            /** @enum {string} */
            source: "operator" | "imported";
            entries: components["schemas"]["BillingCatalogEntryInput"][];
        };
        BillingCatalogEntryInput: {
            provider_id: components["schemas"]["Id"];
            channel_id: components["schemas"]["Id"];
            model: string;
            input_microunits_per_million: number;
            output_microunits_per_million: number;
            reasoning_microunits_per_million: number;
            cache_read_microunits_per_million: number;
            cache_creation_microunits_per_million: number;
            cached_microunits_per_million: number;
        };
        BillingCatalogRollbackInput: {
            new_catalog_version_id: components["schemas"]["Id"];
            effective_at_ms: number;
        };
        RoutingPricePolicyInput: {
            catalog_version_id: components["schemas"]["Id"];
            /** @enum {string} */
            comparison: "rate_dominance_v1";
        };
        RoutingPricePolicy: {
            catalog_version_id: components["schemas"]["Id"];
            /** @enum {string} */
            comparison: "rate_dominance_v1";
        };
        BillingCatalog: {
            catalog_version_id: components["schemas"]["Id"];
            effective_at_ms: number;
            /** @enum {string} */
            source: "operator" | "imported" | "test";
            created_at_ms: number;
            entries: components["schemas"]["BillingCatalogEntry"][];
        };
        BillingCatalogEntry: {
            provider_id: components["schemas"]["Id"];
            channel_id: components["schemas"]["Id"];
            model: string;
            input_microunits_per_million: number;
            output_microunits_per_million: number;
            reasoning_microunits_per_million: number;
            cache_read_microunits_per_million: number;
            cache_creation_microunits_per_million: number;
            cached_microunits_per_million: number;
        };
        BillingCatalogMutation: {
            catalog_version_id: components["schemas"]["Id"];
            effective_at_ms: number;
            /** @enum {string} */
            source: "operator" | "imported";
            entry_count: number;
            /** @enum {string} */
            operation: "imported" | "rolled_back";
            rolled_back_from: components["schemas"]["Id"] | null;
        };
        Id512: string;
        /** @description Durable per-attempt observation; never an external request success claim. No upstream payload or diagnostic text. */
        AttemptObservation: {
            attempt_number: number;
            upstream_id: components["schemas"]["Id"];
            started_at_ms: number;
            ended_at_ms: number;
            duration_ms: number;
            error_code: ("ClientRequestError" | "ClientUnauthorized" | "RouteNotFound" | "CredentialUnavailable" | "CredentialUnauthorized" | "CredentialForbidden" | "CredentialQuotaExceeded" | "EgressRejected" | "EgressUnavailable" | "ProviderRateLimited" | "ProviderTransient" | "ProviderPermanent" | "TokenCountUnsupported" | "UpstreamProtocolError" | "StreamTruncated" | "RecordingUnavailable" | "InternalError" | "Cancelled") | null;
            error_scope: ("request" | "credential" | "account" | "model" | "quota_window" | "egress_session" | "egress" | "provider" | "stream" | "internal") | null;
            /** @enum {string} */
            retry_decision: "completed" | "retry_eligible" | "non_retryable" | "retry_closed" | "cancelled" | "infrastructure_failure";
        };
        Attempt: {
            attempt_id: components["schemas"]["Id"];
            outcome: string;
            /** @enum {string} */
            stage?: "request_conversion" | "egress_admission" | "http_transport" | "http_status" | "content_type" | "body_read" | "decoder" | "sse_bootstrap";
            endpoint_id?: components["schemas"]["Id"];
            credential_id?: components["schemas"]["Id"];
            observation?: components["schemas"]["AttemptObservation"];
        };
        AuditEvent: {
            id: number;
            action: string;
            actor: string;
            occurred_at_ms: number;
            config_version_id: components["schemas"]["Id"];
            replaced_config_version_id?: components["schemas"]["Id"] | null;
        };
        BackupPreflight: {
            schema_version: number;
            secret_key_required: boolean;
        };
        RestorePreflight: {
            schema_version: number;
            quick_check_required: boolean;
            compatible: boolean;
        };
        RestoreOperation: {
            /** @enum {string} */
            state: "accepted" | "rejected" | "complete";
        };
        Validation: {
            valid: boolean;
            error_codes?: string[];
        };
        Publication: {
            active_config_version_id: components["schemas"]["Id"];
            replaced_config_version_id?: components["schemas"]["Id"] | null;
        };
        Error: {
            error: {
                code: string;
                message: string;
            };
        };
        AccountRuntimePresentation: {
            identity: components["schemas"]["AccountDisplayIdentity"];
            /** @enum {string} */
            category: "api" | "codex" | "claude" | "kimi" | "kiro" | "grok";
            provider: string;
            api_format: string;
            host: string | null;
            source: string | null;
        };
        NativeAccountMutationReceipt: {
            account_id: string;
            /** Format: int64 */
            revision: number;
            removed: boolean;
            runtime_applied: boolean;
            /** @enum {string} */
            identity_state?: "observed" | "unavailable";
        };
        SystemInformation: {
            build: {
                version: string;
                build_revision: string;
                build_target: string;
                rust_version: string;
                schema_version: number;
            };
            uptime_seconds: number;
            /** @enum {string} */
            configuration_application: "live";
            /** @description Current admission of new requests. Null means not observed; false after an unapplied durable runtime change. */
            accepting_requests: boolean | null;
            /** @description Nonsecret identity of this management process. Old invocation evidence is historical after restart. */
            server_instance?: string;
        };
        CatalogModelPage: {
            config_version: components["schemas"]["Id"];
            revision: string;
            target: {
                endpoint_id: components["schemas"]["Id"];
                credential_id: components["schemas"]["Id"];
                snapshot_version: number;
                observed_at_ms: number;
                stale_at_ms: number;
                expires_at_ms: number;
            };
            items: {
                model: string;
                present_in_last_success: boolean;
            }[];
            next_cursor: string | null;
            /** @description Complete last-success count for this account and endpoint; independent of search and page. */
            current_model_count: number;
            /** @description Matching saved entries, including models absent in the latest success. */
            total_count: number;
        };
        RequestHistoryRow: {
            request_id: string;
            client_key_id: string;
            model: string;
            requested_model: string;
            protocol: string;
            upstream_id: string | null;
            endpoint_id: string | null;
            credential_id: string | null;
            error_code: string | null;
            started_at_ms: number | null;
            finished_at_ms: number | null;
            duration_ms: number | null;
            first_content_ms: number | null;
            streaming: boolean;
            attempt_count: number;
            /** @enum {string} */
            outcome: "succeeded" | "failed" | "cancelled" | "unknown";
            usage: {
                /** @enum {string} */
                provenance?: "measured" | "estimated" | "unknown";
                /** @enum {string} */
                input_accounting?: "inclusive" | "exclusive" | "unknown";
                input_tokens?: number | null;
                output_tokens?: number | null;
                reasoning_tokens?: number | null;
                cache_read_tokens?: number | null;
                cache_creation_tokens?: number | null;
                cached_tokens?: number | null;
            } | null;
            cost_microunits: number | null;
            /** @enum {string|null} */
            cost_confidence: "exact" | "partial" | "unknown" | "unpriced" | null;
            ledger_records: number;
        };
        RequestHistorySummary: {
            requests: number;
            succeeded: number;
            failed: number;
            cancelled: number;
            unknown: number;
            attempts: number;
            success_rate: number | null;
            average_duration_ms: number | null;
            average_first_content_ms: number | null;
            p50_duration_ms: number | null;
            p95_duration_ms: number | null;
        } | null;
        RequestHistoryBucket: {
            at_ms: number;
            requests: number;
            succeeded: number;
            failed: number;
            cancelled: number;
            average_duration_ms: number | null;
            average_first_content_ms: number | null;
        };
        RequestHistoryPage: {
            snapshot: number;
            next_cursor: string | null;
            items: components["schemas"]["RequestHistoryRow"][];
            summary: components["schemas"]["RequestHistorySummary"];
            series: components["schemas"]["RequestHistoryBucket"][];
            ledger_snapshot: number;
        };
        AccountAuthorizationReceipt: {
            session_id: components["schemas"]["Id"];
            channel: string;
            config_version: components["schemas"]["Id"];
            upstream_id: components["schemas"]["Id"];
            credential_id: components["schemas"]["Id"];
            started_revision: string;
            revision: string;
            /** @enum {string} */
            state: "pending" | "in_progress" | "completed" | "unknown" | "failed" | "cancelled" | "denied" | "expired";
            expires_at_ms: number;
        };
        CredentialDeletionImpact: {
            config_version: string;
            revision: string;
            credential_id: string;
            upstream_id: string;
            credential_revision: number;
            removed_egress_profiles: number;
            history_retained: boolean;
            upstream_account_revoked: boolean;
            review_token: string;
            removed_bindings: {
                endpoint_id: string;
                enabled: boolean;
            }[];
            retained_endpoints: {
                id: string;
                enabled: boolean;
                remaining_credentials: string[];
            }[];
            retained_routes: string[];
            retained_client_keys: string[];
        };
    };
    responses: {
        /** @description Version metadata */
        ConfigVersion: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["ConfigVersion"];
            };
        };
        /** @description Version metadata list */
        ConfigVersionList: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["ConfigVersion"][];
            };
        };
        /** @description Secret-free egress policy */
        EgressPolicy: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["EgressPolicy"];
            };
        };
        /** @description Egress policies */
        EgressPolicyList: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["EgressPolicy"][];
            };
        };
        /** @description Upstream */
        Upstream: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["Upstream"];
            };
        };
        /** @description Upstreams */
        UpstreamList: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["Upstream"][];
            };
        };
        /** @description Endpoint */
        Endpoint: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["Endpoint"];
            };
        };
        /** @description Redacted credential metadata */
        Credential: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["Credential"];
            };
        };
        /** @description Endpoint credential binding */
        Binding: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["Binding"];
            };
        };
        /** @description Bindings */
        BindingList: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["Binding"][];
            };
        };
        /** @description Secret-free compatible proxy pool */
        CompatibleProxyPool: {
            headers: {
                "Cache-Control"?: string;
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["CompatibleProxyPool"];
            };
        };
        /** @description Secret-free compatible proxy pools */
        CompatibleProxyPoolList: {
            headers: {
                "Cache-Control"?: string;
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["CompatibleProxyPool"][];
            };
        };
        /** @description Secret-free compatible proxy node metadata */
        CompatibleProxyNode: {
            headers: {
                "Cache-Control"?: string;
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["CompatibleProxyNode"];
            };
        };
        /** @description Secret-free compatible proxy nodes */
        CompatibleProxyNodeList: {
            headers: {
                "Cache-Control"?: string;
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["CompatibleProxyNode"][];
            };
        };
        /** @description Secret-free exact compatible egress binding */
        CompatibleEgressBinding: {
            headers: {
                "Cache-Control"?: string;
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["CompatibleEgressBinding"];
            };
        };
        /** @description Secret-free exact compatible egress bindings */
        CompatibleEgressBindingList: {
            headers: {
                "Cache-Control"?: string;
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["CompatibleEgressBinding"][];
            };
        };
        /** @description Value-free endpoint test result */
        EndpointTest: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["EndpointTest"];
            };
        };
        /** @description Value-free result of one exact management Channel Pin */
        ChannelPin: {
            headers: {
                "Cache-Control"?: string;
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["ChannelPin"];
            };
        };
        /** @description Catalog discovery diff */
        CatalogDiff: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["CatalogDiff"];
            };
        };
        /** @description Public model */
        PublicModel: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["PublicModel"];
            };
        };
        /** @description Public models */
        PublicModelList: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["PublicModel"][];
            };
        };
        /** @description Model alias */
        Alias: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["Alias"];
            };
        };
        /** @description Route */
        Route: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["Route"];
            };
        };
        /** @description Route candidate */
        Candidate: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["Candidate"];
            };
        };
        /** @description Access group */
        AccessGroup: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["AccessGroup"];
            };
        };
        /** @description Access groups */
        AccessGroupList: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["AccessGroup"][];
            };
        };
        /** @description Access-group route grant */
        AccessGroupRoute: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["AccessGroupRoute"];
            };
        };
        /** @description Access-group route grants */
        AccessGroupRouteList: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["AccessGroupRoute"][];
            };
        };
        /** @description Redacted client-key metadata */
        ClientKey: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["ClientKey"];
            };
        };
        /** @description Redacted client-key metadata */
        ClientKeyList: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["ClientKey"][];
            };
        };
        /** @description One-time issued client key */
        IssuedClientKey: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["IssuedClientKey"];
            };
        };
        /** @description OAuth operation status without token values */
        OAuthOperation: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["OAuthOperation"];
            };
        };
        /** @description Secret-free Kimi Coding device authorization status */
        KimiDeviceAuthorization: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["KimiDeviceAuthorization"];
            };
        };
        /** @description Resolved dedicated Kimi Coding target */
        KimiPreparedTarget: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["KimiPreparedTarget"];
            };
        };
        /** @description OAuth refresh result without token values */
        OAuthRefreshOperation: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["OAuthRefreshOperation"];
            };
        };
        /** @description Protected non-secret credential metadata */
        CredentialMetadata: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["CredentialMetadata"];
            };
        };
        /** @description Source-labelled catalog status */
        CatalogStatus: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["CatalogStatus"];
            };
        };
        /** @description Secret-free runtime status */
        RuntimeAvailability: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["RuntimeAvailability"];
            };
        };
        /** @description Controlled runtime recovery action */
        RuntimeAction: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["RuntimeAction"];
            };
        };
        /** @description Read-only route explanation */
        RouteExplain: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["RouteExplain"];
            };
        };
        /** @description Secret-free configured Provider/Channel/Account inventory page */
        OperationalAccountPoolPage: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["OperationalAccountPoolPage"];
            };
        };
        /** @description Read-only, secret-free Provider-specific egress/session/clearance runtime projection */
        ProviderEgressStatusPage: {
            headers: {
                "Cache-Control"?: "no-store";
                ETag?: string;
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["ProviderEgressStatusPage"];
            };
        };
        /** @description Secret-free observed Provider-owned account-pool snapshot */
        ProviderAccountPoolPage: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["ProviderAccountPoolPage"];
            };
        };
        /** @description Secret-free local Provider account operator action receipt */
        ProviderAccountOperatorAction: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["ProviderAccountOperatorActionReceipt"];
            };
        };
        /** @description Secret-free bounded failure feedback from durable Attempt classifications */
        ProviderAccountFailurePage: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["ProviderAccountFailurePage"];
            };
        };
        /** @description Secret-free durable usage and explicitly unpriced cost page */
        OperationalUsagePage: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["OperationalUsagePage"];
            };
        };
        /** @description Secret-free immutable billing rows with status and known-cost summary */
        OperationalBillingPage: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["OperationalBillingPage"];
            };
        };
        /** @description Immutable billing catalogs */
        BillingCatalogList: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["BillingCatalog"][];
            };
        };
        /** @description Secret-free billing catalog mutation receipt */
        BillingCatalogMutation: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["BillingCatalogMutation"];
            };
        };
        /** @description Config-Version-bound routing price policy lineage */
        RoutingPricePolicy: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["RoutingPricePolicy"];
            };
        };
        /** @description Value-free attempts */
        AttemptList: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["Attempt"][];
            };
        };
        /** @description Append-only management audit records */
        AuditEventList: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["AuditEvent"][];
            };
        };
        /** @description Backup preflight */
        BackupPreflight: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["BackupPreflight"];
            };
        };
        /** @description Restore preflight */
        RestorePreflight: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["RestorePreflight"];
            };
        };
        /** @description Restore operation */
        RestoreOperation: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["RestoreOperation"];
            };
        };
        /** @description Validation result */
        Validation: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["Validation"];
            };
        };
        /** @description Publication result */
        Publication: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["Publication"];
            };
        };
        /** @description Safe management error */
        Error: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/json": components["schemas"]["Error"];
            };
        };
    };
    parameters: {
        /** @description Explicit draft or active configuration graph identity. */
        ConfigVersion: components["schemas"]["Id"];
        /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
        IfMatch: string;
        ConfigVersionId: components["schemas"]["Id"];
        EgressPolicyId: components["schemas"]["Id"];
        UpstreamId: components["schemas"]["Id"];
        EndpointId: components["schemas"]["Id"];
        CredentialId: components["schemas"]["Id"];
        BillingCatalogVersionId: components["schemas"]["Id"];
        PublicModelId: components["schemas"]["Id"];
        RouteId: components["schemas"]["Id"];
        AccessGroupId: components["schemas"]["Id"];
        ClientKeyId: components["schemas"]["Id"];
        CompatibleProxyPoolId: components["schemas"]["Id"];
        CompatibleProxyNodeId: components["schemas"]["Id"];
        RequestId: components["schemas"]["Id"];
        RequestedModel: string;
        RequestProtocol: "openai_chat_completions" | "openai_responses" | "anthropic_messages";
        /** @description Exact Provider scope. May be omitted only when the selected Route has one unique Provider. */
        RouteExplainProviderId: components["schemas"]["Id"];
        OperationalProviderId: components["schemas"]["Id"];
        OperationalChannelId: components["schemas"]["Id"];
        OperationalAccountStatus: "active" | "cooling" | "unauthorized" | "disabled";
        OperationalEnabled: boolean;
        OperationalLimit: number;
        OperationalCursor: string;
        ProviderEgressStatusProviderId: components["schemas"]["Id"];
        ProviderEgressStatusUpstreamId: components["schemas"]["Id"];
        /** @description Exact configured Endpoint identity used as the Provider channel scope. */
        ProviderEgressStatusChannelId: components["schemas"]["Id"];
        ProviderEgressStatusDomain: "egress" | "session" | "clearance";
        ProviderEgressStatusState: "available" | "cooling_down" | "circuit_open" | "probe_due" | "probe_in_flight" | "disabled" | "absent" | "active" | "expired" | "challenge_required" | "invalid" | "fresh" | "refresh_required" | "refresh_in_flight";
        ProviderEgressStatusCredentialId: components["schemas"]["Id"];
        ProviderEgressStatusLimit: number;
        ProviderEgressStatusCursor: string;
        ProviderAccountPoolProviderId: components["schemas"]["Id"];
        ProviderAccountPoolChannelId: components["schemas"]["Id"];
        ProviderAccountPoolAuthStatus: "active" | "reauth_required" | "disabled" | "expired";
        ProviderAccountPoolRuntimeStatus: "available" | "cooling" | "circuit_open" | "quota_blocked" | "unauthorized" | "recovery_in_flight" | "expired";
        ProviderAccountPoolEnabled: boolean;
        ProviderAccountPoolLimit: number;
        ProviderAccountPoolCursor: string;
        ProviderAccountFailureAccountId: components["schemas"]["Id"];
        ProviderAccountFailureLimit: number;
        ProviderAccountFailureCursor: string;
        OperationalFromMs: number;
        OperationalToMs: number;
        OperationalAccountId: components["schemas"]["Id"];
        OperationalModel: string;
        OperationalClientKeyId: components["schemas"]["Id"];
        OperationalAccessGroupId: components["schemas"]["Id"];
        OperationalProtocol: "openai_chat_completions" | "openai_responses" | "anthropic_messages";
        OperationalUsageLimit: number;
        OperationalUsageCursor: string;
        BillingFromMs: number;
        BillingToMs: number;
        BillingProviderId: components["schemas"]["Id"];
        BillingChannelId: components["schemas"]["Id"];
        BillingAccountId: components["schemas"]["Id"];
        BillingModel: string;
        BillingStatus: "exact" | "partial" | "unknown" | "unpriced";
        BillingLimit: number;
        BillingCursor: string;
    };
    requestBodies: {
        ConfigVersionInput: {
            content: {
                "application/json": components["schemas"]["ConfigVersionInput"];
            };
        };
        EgressPolicyInput: {
            content: {
                "application/json": components["schemas"]["EgressPolicyInput"];
            };
        };
        UpstreamInput: {
            content: {
                "application/json": components["schemas"]["UpstreamInput"];
            };
        };
        EndpointInput: {
            content: {
                "application/json": components["schemas"]["EndpointInput"];
            };
        };
        CredentialInput: {
            content: {
                "application/json": components["schemas"]["CredentialInput"];
            };
        };
        BindingInput: {
            content: {
                "application/json": components["schemas"]["BindingInput"];
            };
        };
        CompatibleProxyPoolInput: {
            content: {
                "application/json": components["schemas"]["CompatibleProxyPoolInput"];
            };
        };
        CompatibleProxyNodeCreateInput: {
            content: {
                "application/json": components["schemas"]["CompatibleProxyNodeCreateInput"];
            };
        };
        CompatibleProxyNodeUpdateInput: {
            content: {
                "application/json": components["schemas"]["CompatibleProxyNodeUpdateInput"];
            };
        };
        CompatibleEgressBindingInput: {
            content: {
                "application/json": components["schemas"]["CompatibleEgressBindingInput"];
            };
        };
        EndpointTestInput: {
            content: {
                "application/json": components["schemas"]["EndpointTestInput"];
            };
        };
        ChannelPinInput: {
            content: {
                "application/json": components["schemas"]["ChannelPinInput"];
            };
        };
        PublicModelInput: {
            content: {
                "application/json": components["schemas"]["PublicModelInput"];
            };
        };
        AliasInput: {
            content: {
                "application/json": components["schemas"]["AliasInput"];
            };
        };
        RouteInput: {
            content: {
                "application/json": components["schemas"]["RouteInput"];
            };
        };
        CandidateInput: {
            content: {
                "application/json": components["schemas"]["CandidateInput"];
            };
        };
        AccessGroupInput: {
            content: {
                "application/json": components["schemas"]["AccessGroupInput"];
            };
        };
        AccessGroupRouteInput: {
            content: {
                "application/json": components["schemas"]["AccessGroupRouteInput"];
            };
        };
        ClientKeyInput: {
            content: {
                "application/json": components["schemas"]["ClientKeyInput"];
            };
        };
        RuntimeTarget: {
            content: {
                "application/json": components["schemas"]["RuntimeTarget"];
            };
        };
        ProviderAccountOperatorAction: {
            content: {
                "application/json": components["schemas"]["ProviderAccountOperatorAction"];
            };
        };
        BillingCatalogImportInput: {
            content: {
                "application/json": components["schemas"]["BillingCatalogImportInput"];
            };
        };
        BillingCatalogRollbackInput: {
            content: {
                "application/json": components["schemas"]["BillingCatalogRollbackInput"];
            };
        };
        RoutingPricePolicyInput: {
            content: {
                "application/json": components["schemas"]["RoutingPricePolicyInput"];
            };
        };
        CredentialExportInput: {
            content: {
                "application/json": {
                    /** @enum {string} */
                    format: "cpa" | "sub2api";
                };
            };
        };
        OAuthCallbackInput: {
            content: {
                "application/json": {
                    state: string;
                    code?: string;
                    error?: string;
                    /** Format: uri */
                    callback_url?: string;
                };
            };
        };
        BackupMaterial: {
            content: {
                "application/octet-stream": string;
            };
        };
    };
    headers: never;
    pathItems: never;
}
export type $defs = Record<string, never>;
export interface operations {
    refreshPriceSource: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    models: string[];
                };
            };
        };
        responses: {
            /** @description Source quotes; missing rates stay null and tiered rates require manual review */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        /** @constant */
                        source: "models.dev";
                        url: string;
                        /** @constant */
                        currency: "USD";
                        /** @constant */
                        unit: "microunits_per_million_tokens";
                        items: {
                            provider: string;
                            model: string;
                            tiered: boolean;
                            input_microunits_per_million: number | null;
                            output_microunits_per_million: number | null;
                            reasoning_microunits_per_million: number | null;
                            cache_read_microunits_per_million: number | null;
                            cache_creation_microunits_per_million: number | null;
                            cached_microunits_per_million: number | null;
                        }[];
                    };
                };
            };
            /** @description Invalid model list */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Authentication required */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description CSRF rejected */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Read capacity exceeded */
            429: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Source unavailable; existing prices unchanged */
            502: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    startKiroEnrollment: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    id: string;
                    session_id?: string;
                    region?: string;
                    start_url?: string;
                    /**
                     * @description Replace the exact existing credential with CAS; preserve bindings and disabled state.
                     * @default false
                     */
                    replace_existing?: boolean;
                };
            };
        };
        responses: {
            /** @description Pending, denial, expiry, cancellation or completed replacement; ETag advances only when saved */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        /** @enum {unknown} */
                        state: "pending" | "completed" | "cancelled" | "denied" | "expired" | "failed";
                        session_id?: string;
                        user_code?: string;
                        verification_uri?: string;
                        expires_at_ms?: number;
                        interval_ms?: number;
                        credential_id?: string;
                    };
                };
            };
            /** @description Credential persisted with advanced ETag */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        /** @enum {unknown} */
                        state: "pending" | "completed" | "cancelled" | "denied" | "expired" | "failed";
                        session_id?: string;
                        user_code?: string;
                        verification_uri?: string;
                        expires_at_ms?: number;
                        interval_ms?: number;
                        credential_id?: string;
                    };
                };
            };
            /** @description Invalid input */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Authentication required */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description CSRF rejected */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Expired, consumed or changed context */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description OIDC unavailable; do not automatically replay */
            502: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    pollKiroEnrollment: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    id: string;
                    session_id: string;
                    region?: string;
                    start_url?: string;
                    /**
                     * @description Replace the exact existing credential with CAS; preserve bindings and disabled state.
                     * @default false
                     */
                    replace_existing?: boolean;
                };
            };
        };
        responses: {
            /** @description Pending, denial, expiry, cancellation or completed replacement; ETag advances only when saved */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        /** @enum {unknown} */
                        state: "pending" | "completed" | "cancelled" | "denied" | "expired" | "failed";
                        session_id?: string;
                        user_code?: string;
                        verification_uri?: string;
                        expires_at_ms?: number;
                        interval_ms?: number;
                        credential_id?: string;
                    };
                };
            };
            /** @description Credential persisted with advanced ETag */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        /** @enum {unknown} */
                        state: "pending" | "completed" | "cancelled" | "denied" | "expired" | "failed";
                        session_id?: string;
                        user_code?: string;
                        verification_uri?: string;
                        expires_at_ms?: number;
                        interval_ms?: number;
                        credential_id?: string;
                    };
                };
            };
            /** @description Invalid input */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Authentication required */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description CSRF rejected */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Expired, consumed or changed context */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description OIDC unavailable; do not automatically replay */
            502: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    cancelKiroEnrollment: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    id: string;
                    session_id: string;
                    region?: string;
                    start_url?: string;
                    /**
                     * @description Replace the exact existing credential with CAS; preserve bindings and disabled state.
                     * @default false
                     */
                    replace_existing?: boolean;
                };
            };
        };
        responses: {
            /** @description Pending, denial, expiry, cancellation or completed replacement; ETag advances only when saved */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        /** @enum {unknown} */
                        state: "pending" | "completed" | "cancelled" | "denied" | "expired" | "failed";
                        session_id?: string;
                        user_code?: string;
                        verification_uri?: string;
                        expires_at_ms?: number;
                        interval_ms?: number;
                        credential_id?: string;
                    };
                };
            };
            /** @description Credential persisted with advanced ETag */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        /** @enum {unknown} */
                        state: "pending" | "completed" | "cancelled" | "denied" | "expired" | "failed";
                        session_id?: string;
                        user_code?: string;
                        verification_uri?: string;
                        expires_at_ms?: number;
                        interval_ms?: number;
                        credential_id?: string;
                    };
                };
            };
            /** @description Invalid input */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Authentication required */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description CSRF rejected */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Expired, consumed or changed context */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description OIDC unavailable; do not automatically replay */
            502: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    startKimiEnrollment: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    id: string;
                    /** @default false */
                    replace_existing?: boolean;
                };
            };
        };
        responses: {
            200: components["responses"]["KimiDeviceAuthorization"];
            201: components["responses"]["KimiDeviceAuthorization"];
            /** @description Invalid input */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Authentication required */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description CSRF rejected */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Expired, consumed or changed context */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Kimi OAuth unavailable; do not automatically replay */
            502: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    pollKimiEnrollment: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    id: string;
                    session_id: string;
                    /** @default false */
                    replace_existing?: boolean;
                };
            };
        };
        responses: {
            200: components["responses"]["KimiDeviceAuthorization"];
            201: components["responses"]["KimiDeviceAuthorization"];
            /** @description Invalid input */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Authentication required */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description CSRF rejected */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Expired, consumed or changed context */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Kimi OAuth unavailable; do not automatically replay */
            502: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    cancelKimiEnrollment: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    id: string;
                    session_id: string;
                    /** @default false */
                    replace_existing?: boolean;
                };
            };
        };
        responses: {
            200: components["responses"]["KimiDeviceAuthorization"];
            /** @description Invalid input */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Authentication required */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description CSRF rejected */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Expired, consumed or changed context */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    prepareKimiAccountTarget: {
        parameters: {
            query?: {
                /** @description Explicit existing Kimi Coding service; unknown or foreign owner is rejected. */
                upstream_id?: string;
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["KimiPreparedTarget"];
            /** @description Authentication required */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description CSRF rejected */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Draft changed or Kimi target is ambiguous */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    prepareKiroAccountTarget: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    region?: string;
                };
            };
        };
        responses: {
            200: components["responses"]["KimiPreparedTarget"];
            /** @description Invalid region */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Authentication required */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description CSRF rejected */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Draft changed or Kiro target is ambiguous */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    prepareCodexAccountTarget: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["KimiPreparedTarget"];
            /** @description Authentication required */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description CSRF rejected */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Draft changed or Codex target is incompatible */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    prepareClaudeAccountTarget: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["KimiPreparedTarget"];
            /** @description Authentication required */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description CSRF rejected */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Draft changed or Claude target is incompatible */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    getAccountImportReceipt: {
        parameters: {
            query: {
                started_revision: string;
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                upstream_id: components["schemas"]["Id"];
                import_id: components["schemas"]["Id"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Bounded process-local import receipt for the exact actor, configuration, owner, input ID and starting revision. Read only; never reimports or calls Provider. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AccountAuthorizationReceipt"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getNativeAccountImportReceipt: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                batch_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Durable value-free committed import receipt; does not reimport or apply runtime. Old batches without a recorded receipt remain unknown. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        batch_id: string;
                        account_id: string;
                        /** @enum {string} */
                        channel: "grok.build" | "grok.console" | "grok.web";
                        credential_revision: number;
                        /** @enum {string} */
                        outcome: "created" | "unchanged" | "updated";
                        saved_at_ms: number;
                        /** @constant */
                        saved: true;
                        account_exists: boolean;
                    };
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listNativeAccounts: {
        parameters: {
            query?: {
                limit?: number;
                cursor?: string;
                q?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Safe native account result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        items: {
                            id: string;
                            /** @enum {string} */
                            provider: "grok_build" | "grok_console" | "grok_web";
                            auth_status: string;
                            enabled: boolean;
                            /** Format: int64 */
                            revision: number;
                            import_batch_id: string;
                            identity: components["schemas"]["AccountDisplayIdentity"];
                            /** @description Adapter supported IDs only, not upstream observation or account entitlement; null for dynamic-catalog channels. */
                            adapter_models?: string[] | null;
                        }[];
                        next_cursor: string | null;
                    };
                };
            };
            default: components["responses"]["Error"];
        };
    };
    importNativeAccount: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    id: string;
                    /** @enum {string} */
                    channel: "grok.build" | "grok.console" | "grok.web";
                    secret: string;
                };
            };
        };
        responses: {
            /** @description Atomically imported native account */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        /** Format: int64 */
                        created: number;
                        /** Format: int64 */
                        unchanged: number;
                        /**
                         * @description SSO imports attempt identity lookup after saving; failure does not discard a valid imported credential.
                         * @enum {string}
                         */
                        identity_state?: "observed" | "unavailable";
                        runtime_applied?: boolean;
                        /**
                         * Format: int64
                         * @description Reliable same-identity authorization updates; operator settings are retained.
                         */
                        updated?: number;
                    };
                };
            };
            default: components["responses"]["Error"];
        };
    };
    startNativeAccountAuthorization: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    /** @description Optional import trace label. Identity is obtained from the OAuth provider, never inferred from this label. */
                    name?: string;
                    target?: {
                        account_id: string;
                        /** Format: int64 */
                        revision: number;
                    };
                };
            };
        };
        responses: {
            /** @description Safe native account result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        session_id: string;
                        /** @enum {string} */
                        state: "pending" | "complete" | "denied" | "expired" | "cancelled" | "failed" | "persistence_conflict";
                        user_code: string;
                        verification_uri: string;
                        /** Format: int64 */
                        expires_at_ms: number;
                        /** Format: int64 */
                        retry_at_ms: number;
                        identity: components["schemas"]["AccountDisplayIdentity"] | null;
                        /** @enum {string} */
                        identity_state: "pending" | "observed" | "not_provided" | "unavailable";
                        account_id?: string | null;
                        credential_revision?: number | null;
                        runtime_applied?: boolean | null;
                    };
                };
            };
            default: components["responses"]["Error"];
        };
    };
    pollNativeAccountAuthorization: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                session_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Safe native account result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        session_id: string;
                        /** @enum {string} */
                        state: "pending" | "complete" | "denied" | "expired" | "cancelled" | "failed" | "persistence_conflict";
                        user_code: string;
                        verification_uri: string;
                        /** Format: int64 */
                        expires_at_ms: number;
                        /** Format: int64 */
                        retry_at_ms: number;
                        identity: components["schemas"]["AccountDisplayIdentity"] | null;
                        /** @enum {string} */
                        identity_state: "pending" | "observed" | "not_provided" | "unavailable";
                        account_id?: string | null;
                        credential_revision?: number | null;
                        runtime_applied?: boolean | null;
                    };
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getNativeAccountAuthorizationReceipt: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                session_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Value-free original device result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        session_id: string;
                        /** @enum {string} */
                        state: "pending" | "complete" | "denied" | "expired" | "cancelled" | "failed" | "persistence_conflict";
                        /** Format: int64 */
                        expires_at_ms: number;
                        /** Format: int64 */
                        retry_at_ms: number;
                        identity: components["schemas"]["AccountDisplayIdentity"] | null;
                        /** @enum {string} */
                        identity_state: "pending" | "observed" | "not_provided" | "unavailable";
                        runtime_applied?: boolean | null;
                        account_id: string | null;
                        credential_revision: number | null;
                    };
                };
            };
            /** @description Missing, foreign or expired receipt; reconcile the saved account without replay */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Management read unavailable */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    cancelNativeAccountAuthorization: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                session_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Safe native account result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        session_id: string;
                        /** @enum {string} */
                        state: "pending" | "complete" | "denied" | "expired" | "cancelled" | "failed" | "persistence_conflict";
                        user_code: string;
                        verification_uri: string;
                        /** Format: int64 */
                        expires_at_ms: number;
                        /** Format: int64 */
                        retry_at_ms: number;
                        identity: components["schemas"]["AccountDisplayIdentity"] | null;
                        /** @enum {string} */
                        identity_state: "pending" | "observed" | "not_provided" | "unavailable";
                        account_id?: string | null;
                        credential_revision?: number | null;
                        runtime_applied?: boolean | null;
                    };
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listAccountChannels: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Explicit channel enrollment methods and current integration availability. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        id: string;
                        name: string;
                        credential_format: string;
                        import_available: boolean;
                        /** @enum {string} */
                        authorization_flow: "none" | "authorization_code" | "device_code";
                        authorization_available: boolean;
                        upstream_kinds: string[];
                    }[];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    importChannelAccount: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    id: string;
                    /** @enum {string} */
                    channel: "openai-compatible" | "anthropic-compatible" | "codex" | "claude" | "grok.official" | "kiro" | "kimi-api" | "kimi-coding";
                    secret: string;
                };
            };
        };
        responses: {
            /** @description Identical existing credential retained; status and bindings unchanged, graph revision and import audit advanced. */
            200: components["responses"]["Credential"];
            201: components["responses"]["Credential"];
            default: components["responses"]["Error"];
        };
    };
    updateCredentialStatus: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    /** @enum {string} */
                    status: "active" | "disabled";
                    /** Format: int64 */
                    credential_revision: number;
                };
            };
        };
        responses: {
            200: components["responses"]["Credential"];
            default: components["responses"]["Error"];
        };
    };
    loginAdministrator: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["AdministratorLoginInput"];
            };
        };
        responses: {
            /** @description Administrator session; no-store */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AdministratorSession"];
                };
            };
            401: components["responses"]["Error"];
            429: components["responses"]["Error"];
            default: components["responses"]["Error"];
        };
    };
    changeAdministratorPassword: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["AdministratorPasswordInput"];
            };
        };
        responses: {
            /** @description Completed; no-store */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            429: components["responses"]["Error"];
            default: components["responses"]["Error"];
        };
    };
    logoutAdministrator: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Completed; no-store */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            default: components["responses"]["Error"];
        };
    };
    forkConfigVersion: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                config_version_id: components["parameters"]["ConfigVersionId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    id: components["schemas"]["Id"];
                    description: string;
                };
            };
        };
        responses: {
            /** @description Complete editable copy. The response revision belongs to the new draft, not the source. */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ConfigVersion"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    compareConfigVersions: {
        parameters: {
            query: {
                base_id: components["schemas"]["Id"];
                limit?: number;
                cursor?: string;
            };
            header?: never;
            path: {
                config_version_id: components["parameters"]["ConfigVersionId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description One comparison page */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ConfigurationDiffPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listManagementResourceAuditEvents: {
        parameters: {
            query?: {
                limit?: number;
                before_id?: string;
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description One resource audit page */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ResourceAuditPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listConfigVersions: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["ConfigVersionList"];
            default: components["responses"]["Error"];
        };
    };
    createConfigVersion: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: components["requestBodies"]["ConfigVersionInput"];
        responses: {
            201: components["responses"]["ConfigVersion"];
            default: components["responses"]["Error"];
        };
    };
    getConfigVersion: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                config_version_id: components["parameters"]["ConfigVersionId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["ConfigVersion"];
            default: components["responses"]["Error"];
        };
    };
    validateConfigVersion: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                config_version_id: components["parameters"]["ConfigVersionId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["Validation"];
            default: components["responses"]["Error"];
        };
    };
    publishConfigVersion: {
        parameters: {
            query?: never;
            header: {
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
                /** @description JSON string of expected active configuration ID, or null. Mismatch returns 409 before mutation. Optional for legacy callers. */
                "X-Expected-Active-Version"?: string;
                /** @description Latest publication or rollback append ID observed in audit, or 0 if absent. Prevents active-version ABA changes during confirmation; mismatch returns 409. */
                "X-Expected-Lifecycle-Event"?: number;
                /** @description Explicit confirmation of a current nonempty restoration review; stale or missing confirmation fails closed in the activation transaction. */
                "X-Account-Restoration-Review"?: string;
            };
            path: {
                config_version_id: components["parameters"]["ConfigVersionId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["Publication"];
            409: components["responses"]["Error"];
            default: components["responses"]["Error"];
        };
    };
    rollbackConfigVersion: {
        parameters: {
            query?: never;
            header: {
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
                /** @description JSON string of expected active configuration ID, or null. Mismatch returns 409 before mutation. Optional for legacy callers. */
                "X-Expected-Active-Version"?: string;
                /** @description Latest publication or rollback append ID observed in audit, or 0 if absent. Prevents active-version ABA changes during confirmation; mismatch returns 409. */
                "X-Expected-Lifecycle-Event"?: number;
                /** @description Explicit confirmation of a current nonempty restoration review; stale or missing confirmation fails closed in the activation transaction. */
                "X-Account-Restoration-Review"?: string;
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["Publication"];
            409: components["responses"]["Error"];
            default: components["responses"]["Error"];
        };
    };
    listEgressPolicies: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["EgressPolicyList"];
            default: components["responses"]["Error"];
        };
    };
    createEgressPolicy: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: components["requestBodies"]["EgressPolicyInput"];
        responses: {
            201: components["responses"]["EgressPolicy"];
            default: components["responses"]["Error"];
        };
    };
    getEgressPolicy: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                egress_policy_id: components["parameters"]["EgressPolicyId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["EgressPolicy"];
            default: components["responses"]["Error"];
        };
    };
    deleteEgressPolicy: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                egress_policy_id: components["parameters"]["EgressPolicyId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Deleted */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            default: components["responses"]["Error"];
        };
    };
    updateEgressPolicy: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                egress_policy_id: components["parameters"]["EgressPolicyId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["EgressPolicyInput"];
        responses: {
            200: components["responses"]["EgressPolicy"];
            default: components["responses"]["Error"];
        };
    };
    listUpstreams: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["UpstreamList"];
            default: components["responses"]["Error"];
        };
    };
    createUpstream: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: components["requestBodies"]["UpstreamInput"];
        responses: {
            201: components["responses"]["Upstream"];
            default: components["responses"]["Error"];
        };
    };
    getUpstream: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["Upstream"];
            default: components["responses"]["Error"];
        };
    };
    deleteUpstream: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Deleted */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            default: components["responses"]["Error"];
        };
    };
    updateUpstream: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["UpstreamInput"];
        responses: {
            200: components["responses"]["Upstream"];
            default: components["responses"]["Error"];
        };
    };
    createEndpoint: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["EndpointInput"];
        responses: {
            201: components["responses"]["Endpoint"];
            default: components["responses"]["Error"];
        };
    };
    getEndpoint: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                endpoint_id: components["parameters"]["EndpointId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["Endpoint"];
            default: components["responses"]["Error"];
        };
    };
    deleteEndpoint: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                endpoint_id: components["parameters"]["EndpointId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Deleted */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            default: components["responses"]["Error"];
        };
    };
    updateEndpoint: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                endpoint_id: components["parameters"]["EndpointId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["EndpointInput"];
        responses: {
            200: components["responses"]["Endpoint"];
            default: components["responses"]["Error"];
        };
    };
    createCredential: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["CredentialInput"];
        responses: {
            201: components["responses"]["Credential"];
            default: components["responses"]["Error"];
        };
    };
    getCredential: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Exact reviewed impact digest. A stale supplied review is rejected before deletion. */
                "X-Deletion-Impact-Review"?: string;
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["Credential"];
            default: components["responses"]["Error"];
        };
    };
    deleteCredential: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Exact reviewed impact digest. A stale supplied review is rejected before deletion. */
                "X-Deletion-Impact-Review"?: string;
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Deleted */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            default: components["responses"]["Error"];
        };
    };
    updateCredential: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Exact reviewed impact digest. A stale supplied review is rejected before deletion. */
                "X-Deletion-Impact-Review"?: string;
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["CredentialInput"];
        responses: {
            200: components["responses"]["Credential"];
            default: components["responses"]["Error"];
        };
    };
    listEndpointCredentialBindings: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                endpoint_id: components["parameters"]["EndpointId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["BindingList"];
            default: components["responses"]["Error"];
        };
    };
    createEndpointCredentialBinding: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                endpoint_id: components["parameters"]["EndpointId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["BindingInput"];
        responses: {
            201: components["responses"]["Binding"];
            default: components["responses"]["Error"];
        };
    };
    testEndpoint: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                endpoint_id: components["parameters"]["EndpointId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["EndpointTestInput"];
        responses: {
            200: components["responses"]["EndpointTest"];
            default: components["responses"]["Error"];
        };
    };
    previewCatalogDiscovery: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                endpoint_id: components["parameters"]["EndpointId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["CatalogDiff"];
            default: components["responses"]["Error"];
        };
    };
    applyCatalogDiscovery: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                endpoint_id: components["parameters"]["EndpointId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["CatalogDiff"];
            default: components["responses"]["Error"];
        };
    };
    listPublicModels: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["PublicModelList"];
            default: components["responses"]["Error"];
        };
    };
    createPublicModel: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: components["requestBodies"]["PublicModelInput"];
        responses: {
            201: components["responses"]["PublicModel"];
            default: components["responses"]["Error"];
        };
    };
    getPublicModel: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                public_model_id: components["parameters"]["PublicModelId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["PublicModel"];
            default: components["responses"]["Error"];
        };
    };
    deletePublicModel: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                public_model_id: components["parameters"]["PublicModelId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Deleted */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            default: components["responses"]["Error"];
        };
    };
    updatePublicModel: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                public_model_id: components["parameters"]["PublicModelId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["PublicModelInput"];
        responses: {
            200: components["responses"]["PublicModel"];
            default: components["responses"]["Error"];
        };
    };
    createModelAlias: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                public_model_id: components["parameters"]["PublicModelId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["AliasInput"];
        responses: {
            201: components["responses"]["Alias"];
            default: components["responses"]["Error"];
        };
    };
    deleteModelAlias: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                public_model_id: components["parameters"]["PublicModelId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["AliasInput"];
        responses: {
            /** @description Alias deleted */
            204: {
                headers: {
                    ETag?: string;
                    [name: string]: unknown;
                };
                content?: never;
            };
            default: components["responses"]["Error"];
        };
    };
    createRoute: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                public_model_id: components["parameters"]["PublicModelId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["RouteInput"];
        responses: {
            201: components["responses"]["Route"];
            default: components["responses"]["Error"];
        };
    };
    listAccountInventory: {
        parameters: {
            query?: {
                /** @description Classify actual stored authentication material: api selects configured API keys; account selects OAuth and provider-native accounts. Counts and cursor use the same filter. Omission preserves the unified inventory. */
                entry_type?: "api" | "account";
                q?: string;
                category?: "api" | "codex" | "claude" | "kimi" | "kiro" | "grok";
                status?: "enabled" | "disabled" | "reauth_required";
                upstream_id?: components["schemas"]["Id"];
                sort?: "name" | "name_desc" | "provider";
                limit?: number;
                cursor?: string;
                plan?: string;
                /** @description Match only unobserved plans; cannot combine with plan. */
                without_plan?: boolean;
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Complete-filter counts and one page */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["UnifiedAccountPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listManagedCredentials: {
        parameters: {
            query?: {
                limit?: number;
                cursor?: string;
                upstream_id?: string;
                q?: string;
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description One revision-consistent resource page */
            200: {
                headers: {
                    ETag?: string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ManagedCredentialPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listManagedEndpoints: {
        parameters: {
            query?: {
                limit?: number;
                cursor?: string;
                upstream_id?: string;
                q?: string;
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description One revision-consistent resource page */
            200: {
                headers: {
                    ETag?: string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ManagedEndpointPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listRoutes: {
        parameters: {
            query?: {
                limit?: number;
                cursor?: string;
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description One revision-consistent resource page */
            200: {
                headers: {
                    ETag?: string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RoutePage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listRouteCandidates: {
        parameters: {
            query?: {
                limit?: number;
                cursor?: string;
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description One revision-consistent resource page */
            200: {
                headers: {
                    ETag?: string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CandidatePage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listModelAliases: {
        parameters: {
            query?: {
                limit?: number;
                cursor?: string;
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description One revision-consistent resource page */
            200: {
                headers: {
                    ETag?: string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AliasPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getRoute: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                route_id: components["parameters"]["RouteId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["Route"];
            default: components["responses"]["Error"];
        };
    };
    deleteRoute: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                route_id: components["parameters"]["RouteId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Deleted */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            default: components["responses"]["Error"];
        };
    };
    updateRoute: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                route_id: components["parameters"]["RouteId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["RouteInput"];
        responses: {
            200: components["responses"]["Route"];
            default: components["responses"]["Error"];
        };
    };
    createRouteCandidate: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                route_id: components["parameters"]["RouteId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["CandidateInput"];
        responses: {
            201: components["responses"]["Candidate"];
            default: components["responses"]["Error"];
        };
    };
    deleteRouteCandidate: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                route_id: components["parameters"]["RouteId"];
                candidate_id: components["schemas"]["Id"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Deleted; ETag contains the new draft revision */
            204: {
                headers: {
                    ETag?: string;
                    [name: string]: unknown;
                };
                content?: never;
            };
            default: components["responses"]["Error"];
        };
    };
    updateRouteCandidate: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                route_id: components["parameters"]["RouteId"];
                candidate_id: components["schemas"]["Id"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["CandidateInput"];
        responses: {
            200: components["responses"]["Candidate"];
            default: components["responses"]["Error"];
        };
    };
    validateRoute: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                route_id: components["parameters"]["RouteId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["Validation"];
            default: components["responses"]["Error"];
        };
    };
    explainRoute: {
        parameters: {
            query: {
                requested_model: components["parameters"]["RequestedModel"];
                protocol: components["parameters"]["RequestProtocol"];
                /** @description Exact Provider scope. May be omitted only when the selected Route has one unique Provider. */
                provider_id?: components["parameters"]["RouteExplainProviderId"];
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                route_id: components["parameters"]["RouteId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["RouteExplain"];
            default: components["responses"]["Error"];
        };
    };
    listAccessGroups: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["AccessGroupList"];
            default: components["responses"]["Error"];
        };
    };
    createAccessGroup: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: components["requestBodies"]["AccessGroupInput"];
        responses: {
            201: components["responses"]["AccessGroup"];
            default: components["responses"]["Error"];
        };
    };
    getAccessGroup: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                access_group_id: components["parameters"]["AccessGroupId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["AccessGroup"];
            default: components["responses"]["Error"];
        };
    };
    deleteAccessGroup: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                access_group_id: components["parameters"]["AccessGroupId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Deleted */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            default: components["responses"]["Error"];
        };
    };
    updateAccessGroup: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                access_group_id: components["parameters"]["AccessGroupId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["AccessGroupInput"];
        responses: {
            200: components["responses"]["AccessGroup"];
            default: components["responses"]["Error"];
        };
    };
    listAccessGroupRoutes: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                access_group_id: components["parameters"]["AccessGroupId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["AccessGroupRouteList"];
            default: components["responses"]["Error"];
        };
    };
    grantAccessGroupRoute: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                access_group_id: components["parameters"]["AccessGroupId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["AccessGroupRouteInput"];
        responses: {
            201: components["responses"]["AccessGroupRoute"];
            default: components["responses"]["Error"];
        };
    };
    listClientKeys: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["ClientKeyList"];
            default: components["responses"]["Error"];
        };
    };
    issueClientKey: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: components["requestBodies"]["ClientKeyInput"];
        responses: {
            201: components["responses"]["IssuedClientKey"];
            default: components["responses"]["Error"];
        };
    };
    getClientKey: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                client_key_id: components["parameters"]["ClientKeyId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["ClientKey"];
            default: components["responses"]["Error"];
        };
    };
    revokeClientKey: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                client_key_id: components["parameters"]["ClientKeyId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Revoked */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            default: components["responses"]["Error"];
        };
    };
    updateClientKey: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                client_key_id: components["parameters"]["ClientKeyId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["ClientKeyInput"];
        responses: {
            200: components["responses"]["ClientKey"];
            default: components["responses"]["Error"];
        };
    };
    startCodexEnrollment: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    id: string;
                    /**
                     * @description Explicitly reauthorize the same owned credential instead of creating one.
                     * @default false
                     */
                    replace_existing?: boolean;
                };
            };
        };
        responses: {
            202: components["responses"]["OAuthOperation"];
            default: components["responses"]["Error"];
        };
    };
    cancelCodexEnrollment: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    id: string;
                    /**
                     * @description Explicitly reauthorize the same owned credential instead of creating one.
                     * @default false
                     */
                    replace_existing?: boolean;
                };
            };
        };
        responses: {
            /** @description Transient authorization cancelled; no account created */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            default: components["responses"]["Error"];
        };
    };
    completeCodexEnrollment: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    id: string;
                    callback: {
                        state?: string;
                        code?: string;
                        error?: string;
                        /** Format: uri */
                        callback_url?: string;
                    };
                    /**
                     * @description Explicitly reauthorize the same owned credential instead of creating one.
                     * @default false
                     */
                    replace_existing?: boolean;
                };
            };
        };
        responses: {
            /** @description Authorized credential saved; identical credential material may reuse an existing credential */
            200: {
                headers: {
                    ETag?: string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Credential"];
                };
            };
            /** @description Authorized credential saved; identical credential material may reuse an existing credential */
            201: {
                headers: {
                    ETag?: string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Credential"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    startClaudeEnrollment: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    id: string;
                    /**
                     * @description Explicitly reauthorize the same owned credential instead of creating one.
                     * @default false
                     */
                    replace_existing?: boolean;
                };
            };
        };
        responses: {
            202: components["responses"]["OAuthOperation"];
            default: components["responses"]["Error"];
        };
    };
    cancelClaudeEnrollment: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    id: string;
                    /**
                     * @description Explicitly reauthorize the same owned credential instead of creating one.
                     * @default false
                     */
                    replace_existing?: boolean;
                };
            };
        };
        responses: {
            /** @description Transient authorization cancelled; no account created */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            default: components["responses"]["Error"];
        };
    };
    completeClaudeEnrollment: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                upstream_id: components["parameters"]["UpstreamId"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    id: string;
                    callback: {
                        state?: string;
                        code?: string;
                        error?: string;
                        /** Format: uri */
                        callback_url?: string;
                    };
                    /**
                     * @description Explicitly reauthorize the same owned credential instead of creating one.
                     * @default false
                     */
                    replace_existing?: boolean;
                };
            };
        };
        responses: {
            /** @description Authorized credential saved; identical credential material may reuse an existing credential */
            200: {
                headers: {
                    ETag?: string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Credential"];
                };
            };
            /** @description Authorized credential saved; identical credential material may reuse an existing credential */
            201: {
                headers: {
                    ETag?: string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Credential"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    startCredentialOAuth: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            202: components["responses"]["OAuthOperation"];
            default: components["responses"]["Error"];
        };
    };
    getCredentialOAuthStatus: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["OAuthOperation"];
            default: components["responses"]["Error"];
        };
    };
    cancelCredentialOAuth: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Cancelled */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            default: components["responses"]["Error"];
        };
    };
    completeCredentialOAuth: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["OAuthCallbackInput"];
        responses: {
            202: components["responses"]["OAuthOperation"];
            409: components["responses"]["Error"];
            default: components["responses"]["Error"];
        };
    };
    refreshCredentialOAuth: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            202: components["responses"]["OAuthRefreshOperation"];
            409: components["responses"]["Error"];
            default: components["responses"]["Error"];
        };
    };
    exportCredential: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["CredentialExportInput"];
        responses: {
            /** @description One-time CPA or sub2api JSON export */
            200: {
                headers: {
                    "Cache-Control"?: string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: unknown;
                    };
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getCredentialMetadata: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["CredentialMetadata"];
            default: components["responses"]["Error"];
        };
    };
    getBillingProcessingStatus: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Safe processing status */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["BillingProcessingStatus"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listEffectiveModels: {
        parameters: {
            query?: {
                access_group_id?: components["schemas"]["Id"];
                client_key_id?: components["schemas"]["Id"];
                limit?: number;
                cursor?: string;
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Serving model page */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EffectiveModelPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getCatalogStatus: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["CatalogStatus"];
            default: components["responses"]["Error"];
        };
    };
    getRuntimeAvailability: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["RuntimeAvailability"];
            default: components["responses"]["Error"];
        };
    };
    requestQuotaRecovery: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: components["requestBodies"]["RuntimeTarget"];
        responses: {
            202: components["responses"]["RuntimeAction"];
            default: components["responses"]["Error"];
        };
    };
    listOperationalAccountPools: {
        parameters: {
            query?: {
                provider_id?: components["parameters"]["OperationalProviderId"];
                channel_id?: components["parameters"]["OperationalChannelId"];
                account_status?: components["parameters"]["OperationalAccountStatus"];
                enabled?: components["parameters"]["OperationalEnabled"];
                limit?: components["parameters"]["OperationalLimit"];
                cursor?: components["parameters"]["OperationalCursor"];
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["OperationalAccountPoolPage"];
            409: components["responses"]["Error"];
            default: components["responses"]["Error"];
        };
    };
    listProviderEgressStatus: {
        parameters: {
            query?: {
                provider_id?: components["parameters"]["ProviderEgressStatusProviderId"];
                upstream_id?: components["parameters"]["ProviderEgressStatusUpstreamId"];
                /** @description Exact configured Endpoint identity used as the Provider channel scope. */
                channel_id?: components["parameters"]["ProviderEgressStatusChannelId"];
                domain?: components["parameters"]["ProviderEgressStatusDomain"];
                state?: components["parameters"]["ProviderEgressStatusState"];
                credential_id?: components["parameters"]["ProviderEgressStatusCredentialId"];
                limit?: components["parameters"]["ProviderEgressStatusLimit"];
                cursor?: components["parameters"]["ProviderEgressStatusCursor"];
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["ProviderEgressStatusPage"];
            400: components["responses"]["Error"];
            409: components["responses"]["Error"];
            503: components["responses"]["Error"];
            default: components["responses"]["Error"];
        };
    };
    listCompatibleProxyPools: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["CompatibleProxyPoolList"];
            default: components["responses"]["Error"];
        };
    };
    createCompatibleProxyPool: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: components["requestBodies"]["CompatibleProxyPoolInput"];
        responses: {
            201: components["responses"]["CompatibleProxyPool"];
            default: components["responses"]["Error"];
        };
    };
    getCompatibleProxyPool: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                pool_id: components["parameters"]["CompatibleProxyPoolId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["CompatibleProxyPool"];
            default: components["responses"]["Error"];
        };
    };
    deleteCompatibleProxyPool: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                pool_id: components["parameters"]["CompatibleProxyPoolId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Deleted */
            204: {
                headers: {
                    "Cache-Control"?: string;
                    [name: string]: unknown;
                };
                content?: never;
            };
            default: components["responses"]["Error"];
        };
    };
    updateCompatibleProxyPool: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                pool_id: components["parameters"]["CompatibleProxyPoolId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["CompatibleProxyPoolInput"];
        responses: {
            200: components["responses"]["CompatibleProxyPool"];
            default: components["responses"]["Error"];
        };
    };
    listCompatibleProxyNodes: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["CompatibleProxyNodeList"];
            default: components["responses"]["Error"];
        };
    };
    createCompatibleProxyNode: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: components["requestBodies"]["CompatibleProxyNodeCreateInput"];
        responses: {
            201: components["responses"]["CompatibleProxyNode"];
            default: components["responses"]["Error"];
        };
    };
    getCompatibleProxyNode: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                node_id: components["parameters"]["CompatibleProxyNodeId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["CompatibleProxyNode"];
            default: components["responses"]["Error"];
        };
    };
    deleteCompatibleProxyNode: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                node_id: components["parameters"]["CompatibleProxyNodeId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Deleted */
            204: {
                headers: {
                    "Cache-Control"?: string;
                    [name: string]: unknown;
                };
                content?: never;
            };
            default: components["responses"]["Error"];
        };
    };
    updateCompatibleProxyNode: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                node_id: components["parameters"]["CompatibleProxyNodeId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["CompatibleProxyNodeUpdateInput"];
        responses: {
            200: components["responses"]["CompatibleProxyNode"];
            default: components["responses"]["Error"];
        };
    };
    listCompatibleEgressBindings: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["CompatibleEgressBindingList"];
            default: components["responses"]["Error"];
        };
    };
    createCompatibleEgressBinding: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: components["requestBodies"]["CompatibleEgressBindingInput"];
        responses: {
            201: components["responses"]["CompatibleEgressBinding"];
            default: components["responses"]["Error"];
        };
    };
    getCompatibleEgressBinding: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                endpoint_id: components["parameters"]["EndpointId"];
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["CompatibleEgressBinding"];
            default: components["responses"]["Error"];
        };
    };
    deleteCompatibleEgressBinding: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                endpoint_id: components["parameters"]["EndpointId"];
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Deleted */
            204: {
                headers: {
                    "Cache-Control"?: string;
                    [name: string]: unknown;
                };
                content?: never;
            };
            default: components["responses"]["Error"];
        };
    };
    updateCompatibleEgressBinding: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                endpoint_id: components["parameters"]["EndpointId"];
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["CompatibleEgressBindingInput"];
        responses: {
            200: components["responses"]["CompatibleEgressBinding"];
            default: components["responses"]["Error"];
        };
    };
    executeChannelPin: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: components["requestBodies"]["ChannelPinInput"];
        responses: {
            200: components["responses"]["ChannelPin"];
            409: components["responses"]["Error"];
            default: components["responses"]["Error"];
        };
    };
    listProviderAccountPools: {
        parameters: {
            query?: {
                provider_id?: components["parameters"]["ProviderAccountPoolProviderId"];
                channel_id?: components["parameters"]["ProviderAccountPoolChannelId"];
                auth_status?: components["parameters"]["ProviderAccountPoolAuthStatus"];
                runtime_status?: components["parameters"]["ProviderAccountPoolRuntimeStatus"];
                enabled?: components["parameters"]["ProviderAccountPoolEnabled"];
                limit?: components["parameters"]["ProviderAccountPoolLimit"];
                cursor?: components["parameters"]["ProviderAccountPoolCursor"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["ProviderAccountPoolPage"];
            409: components["responses"]["Error"];
            default: components["responses"]["Error"];
        };
    };
    applyProviderAccountPoolAction: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: components["requestBodies"]["ProviderAccountOperatorAction"];
        responses: {
            202: components["responses"]["ProviderAccountOperatorAction"];
            409: components["responses"]["Error"];
            default: components["responses"]["Error"];
        };
    };
    listProviderAccountFailures: {
        parameters: {
            query?: {
                provider_id?: components["parameters"]["ProviderAccountPoolProviderId"];
                channel_id?: components["parameters"]["ProviderAccountPoolChannelId"];
                account_id?: components["parameters"]["ProviderAccountFailureAccountId"];
                limit?: components["parameters"]["ProviderAccountFailureLimit"];
                cursor?: components["parameters"]["ProviderAccountFailureCursor"];
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["ProviderAccountFailurePage"];
            default: components["responses"]["Error"];
        };
    };
    listOperationalUsage: {
        parameters: {
            query?: {
                from_ms?: components["parameters"]["OperationalFromMs"];
                to_ms?: components["parameters"]["OperationalToMs"];
                provider_id?: components["parameters"]["OperationalProviderId"];
                channel_id?: components["parameters"]["OperationalChannelId"];
                account_id?: components["parameters"]["OperationalAccountId"];
                model?: components["parameters"]["OperationalModel"];
                client_key_id?: components["parameters"]["OperationalClientKeyId"];
                access_group_id?: components["parameters"]["OperationalAccessGroupId"];
                protocol?: components["parameters"]["OperationalProtocol"];
                limit?: components["parameters"]["OperationalUsageLimit"];
                cursor?: components["parameters"]["OperationalUsageCursor"];
                /** @description Explicit opt-in to verifiable partial usage with exclusion counts. Defaults to strict validation. */
                allow_partial?: boolean;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["OperationalUsagePage"];
            default: components["responses"]["Error"];
        };
    };
    listOperationalBilling: {
        parameters: {
            query?: {
                from_ms?: components["parameters"]["BillingFromMs"];
                to_ms?: components["parameters"]["BillingToMs"];
                provider_id?: components["parameters"]["BillingProviderId"];
                channel_id?: components["parameters"]["BillingChannelId"];
                account_id?: components["parameters"]["BillingAccountId"];
                model?: components["parameters"]["BillingModel"];
                status?: components["parameters"]["BillingStatus"];
                limit?: components["parameters"]["BillingLimit"];
                cursor?: components["parameters"]["BillingCursor"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["OperationalBillingPage"];
            default: components["responses"]["Error"];
        };
    };
    listBillingCatalogs: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["BillingCatalogList"];
            default: components["responses"]["Error"];
        };
    };
    importBillingCatalog: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: components["requestBodies"]["BillingCatalogImportInput"];
        responses: {
            201: components["responses"]["BillingCatalogMutation"];
            400: components["responses"]["Error"];
            409: components["responses"]["Error"];
            default: components["responses"]["Error"];
        };
    };
    rollbackBillingCatalog: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path: {
                catalog_version_id: components["parameters"]["BillingCatalogVersionId"];
            };
            cookie?: never;
        };
        requestBody: components["requestBodies"]["BillingCatalogRollbackInput"];
        responses: {
            201: components["responses"]["BillingCatalogMutation"];
            400: components["responses"]["Error"];
            404: components["responses"]["Error"];
            409: components["responses"]["Error"];
            default: components["responses"]["Error"];
        };
    };
    getRoutingPricePolicy: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["RoutingPricePolicy"];
            404: components["responses"]["Error"];
            default: components["responses"]["Error"];
        };
    };
    setRoutingPricePolicy: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: components["requestBodies"]["RoutingPricePolicyInput"];
        responses: {
            200: components["responses"]["RoutingPricePolicy"];
            409: components["responses"]["Error"];
            default: components["responses"]["Error"];
        };
    };
    clearRoutingPricePolicy: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Opaque configuration revision token; a mismatch returns 409 without partial mutation. */
                "If-Match": components["parameters"]["IfMatch"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Routing price policy cleared */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            404: components["responses"]["Error"];
            409: components["responses"]["Error"];
            default: components["responses"]["Error"];
        };
    };
    listRequestAttempts: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                request_id: components["parameters"]["RequestId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["AttemptList"];
            default: components["responses"]["Error"];
        };
    };
    getObservabilityMetrics: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Prometheus text exposition */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "text/plain; version=0.0.4": string;
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listManagementAuditEvents: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["AuditEventList"];
            default: components["responses"]["Error"];
        };
    };
    previewBackup: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: components["responses"]["BackupPreflight"];
            default: components["responses"]["Error"];
        };
    };
    previewRestore: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: components["requestBodies"]["BackupMaterial"];
        responses: {
            200: components["responses"]["RestorePreflight"];
            default: components["responses"]["Error"];
        };
    };
    restoreBackup: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: components["requestBodies"]["BackupMaterial"];
        responses: {
            202: components["responses"]["RestoreOperation"];
            default: components["responses"]["Error"];
        };
    };
    refreshNativeAccountIdentity: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                account_id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    /** Format: int64 */
                    revision: number;
                };
            };
        };
        responses: {
            /** @description Provider identity observed and encrypted for the unchanged credential */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        identity: components["schemas"]["AccountDisplayIdentity"];
                        /** @enum {string} */
                        identity_state: "observed";
                    };
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getNativeAccountDeletionImpact: {
        parameters: {
            query: {
                revision: number;
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                account_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Exact value-free impact */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        config_version: string;
                        /** Format: int64 */
                        config_revision: number;
                        /** Format: int64 */
                        inventory_generation: number;
                        account_id: string;
                        /** Format: int64 */
                        account_revision: number;
                        provider: string;
                        connections: {
                            upstream_id: string;
                            service_name: string;
                            upstream_enabled: boolean;
                            endpoint_id: string;
                            endpoint_enabled: boolean;
                            models: {
                                route_id: string;
                                model: string;
                                upstream_model: string;
                                candidate_enabled: boolean;
                            }[];
                        }[];
                        remaining_accounts: {
                            id: string;
                            /** Format: int64 */
                            revision: number;
                            enabled: boolean;
                            auth_status: string;
                        }[];
                        replacement_guaranteed: boolean;
                        history_retained: boolean;
                        shared_resources_retained: boolean;
                        upstream_account_revoked: boolean;
                        review_token: string;
                    };
                };
            };
            400: components["responses"]["Error"];
            404: components["responses"]["Error"];
            409: components["responses"]["Error"];
            503: components["responses"]["Error"];
        };
    };
    deleteNativeAccount: {
        parameters: {
            query: {
                revision: number;
            };
            header?: {
                /** @description Exact reviewed impact token. When supplied X-Config-Version is required; changed graph or native generation rejects removal. */
                "X-Deletion-Impact-Review"?: string;
                "X-Config-Version"?: string;
            };
            path: {
                account_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Management operation result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NativeAccountMutationReceipt"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    updateNativeAccount: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                account_id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    /** Format: int64 */
                    revision: number;
                    enabled: boolean;
                };
            };
        };
        responses: {
            /** @description Management operation result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NativeAccountMutationReceipt"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    replaceNativeAccountCredential: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                account_id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    /** Format: int64 */
                    revision: number;
                    secret: string;
                };
            };
        };
        responses: {
            /** @description Management operation result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NativeAccountMutationReceipt"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getNativeAccountUsage: {
        parameters: {
            query: {
                revision: number;
            };
            header?: never;
            path: {
                account_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Observation or explicit upstream failure */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        observation: components["schemas"]["AccountQuotaObservation"] | null;
                        /** @enum {string|null} */
                        error: null | "not_connected" | "busy" | "unauthorized" | "forbidden" | "invalid_response" | "unavailable";
                    };
                };
            };
            /** @description Account revision changed */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    listNativeAccountAudit: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                account_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Management operation result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        /** Format: int64 */
                        id: number;
                        /** @enum {string} */
                        provider: "web" | "console" | "build";
                        /** @enum {string} */
                        action: "enabled" | "disabled" | "credential_updated" | "removed";
                        /** Format: int64 */
                        revision: number;
                        actor: string;
                        /** Format: int64 */
                        occurred_at_ms: number;
                    }[];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    applyRuntimeConfiguration: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Management operation result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        runtime_applied: boolean;
                    };
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getSystemInformation: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Running service information */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SystemInformation"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listCatalogModels: {
        parameters: {
            query: {
                endpoint_id: components["schemas"]["Id"];
                credential_id: components["schemas"]["Id"];
                limit?: number;
                cursor?: string;
                q?: string;
            };
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description One bounded, consistent page of saved source models */
            200: {
                headers: {
                    ETag?: string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CatalogModelPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    refreshCatalogModels: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": {
                    endpoint_id: components["schemas"]["Id"];
                    credential_id: components["schemas"]["Id"];
                };
            };
        };
        responses: {
            /** @description Completed complete upstream model catalog */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        config_version: components["schemas"]["Id"];
                        endpoint_id: components["schemas"]["Id"];
                        credential_id: components["schemas"]["Id"];
                        observed_at_ms: number;
                        model_count: number;
                    };
                };
            };
            default: components["responses"]["Error"];
        };
    };
    listRequests: {
        parameters: {
            query?: {
                from_ms?: number;
                to_ms?: number;
                bucket_ms?: number;
                limit?: number;
                model?: string;
                upstream_id?: string;
                credential_id?: string;
                client_key_id?: string;
                outcome?: string;
                request_id?: string;
                cursor?: string;
                include_unknown?: boolean;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Bounded request page; summary and series are populated only by the summary operation. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RequestHistoryPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    summarizeRequests: {
        parameters: {
            query?: {
                from_ms?: number;
                to_ms?: number;
                bucket_ms?: number;
                limit?: number;
                model?: string;
                upstream_id?: string;
                credential_id?: string;
                client_key_id?: string;
                outcome?: string;
                request_id?: string;
                cursor?: string;
                include_unknown?: boolean;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Bounded request page; summary and series are populated only by the summary operation. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RequestHistoryPage"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getAccountAuthorizationReceipt: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
            };
            path: {
                session_id: components["schemas"]["Id"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Value-free retained authorization receipt */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AccountAuthorizationReceipt"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getAccountRestorationReview: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                config_version_id: components["schemas"]["Id"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Bounded restoration review */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        target_id: string;
                        target_revision: number;
                        active_id: string | null;
                        active_revision: number | null;
                        deletion_event_id: number;
                        review_token: string;
                        accounts: {
                            credential_id: string;
                            upstream_id: string;
                            credential_revision: number;
                        }[];
                    };
                };
            };
            default: components["responses"]["Error"];
        };
    };
    getCredentialDeletionImpact: {
        parameters: {
            query?: never;
            header: {
                /** @description Explicit draft or active configuration graph identity. */
                "X-Config-Version": components["parameters"]["ConfigVersion"];
                /** @description Exact reviewed impact digest. A stale supplied review is rejected before deletion. */
                "X-Deletion-Impact-Review"?: string;
            };
            path: {
                credential_id: components["parameters"]["CredentialId"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Value-free local deletion impact */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CredentialDeletionImpact"];
                };
            };
            default: components["responses"]["Error"];
        };
    };
}
