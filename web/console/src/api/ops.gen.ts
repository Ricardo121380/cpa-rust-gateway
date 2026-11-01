// Generated from docs/openapi/management-v1.json by scripts/gen-types.mjs. Do not edit.
// cv: 0 = no X-Config-Version, 1 = optional, 2 = required. ifMatch: 1 = If-Match required.
export const OPS: Record<string, { op: string; cv: 0 | 1 | 2; ifMatch: 0 | 1 }> = {
 "POST /admin/billing/price-source/refresh": {
  "op": "refreshPriceSource",
  "cv": 0,
  "ifMatch": 0
 },
 "POST /admin/upstreams/{upstream_id}/kiro-authorization/start": {
  "op": "startKiroEnrollment",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/upstreams/{upstream_id}/kiro-authorization/poll": {
  "op": "pollKiroEnrollment",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/upstreams/{upstream_id}/kiro-authorization/cancel": {
  "op": "cancelKiroEnrollment",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/upstreams/{upstream_id}/kimi-authorization/start": {
  "op": "startKimiEnrollment",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/upstreams/{upstream_id}/kimi-authorization/poll": {
  "op": "pollKimiEnrollment",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/upstreams/{upstream_id}/kimi-authorization/cancel": {
  "op": "cancelKimiEnrollment",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/account-channels/kimi/prepare-target": {
  "op": "prepareKimiAccountTarget",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/account-channels/kiro/prepare-target": {
  "op": "prepareKiroAccountTarget",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/account-channels/codex/prepare-target": {
  "op": "prepareCodexAccountTarget",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/account-channels/claude/prepare-target": {
  "op": "prepareClaudeAccountTarget",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/upstreams/{upstream_id}/account-imports/{import_id}": {
  "op": "getAccountImportReceipt",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/native-account-imports/{batch_id}": {
  "op": "getNativeAccountImportReceipt",
  "cv": 0,
  "ifMatch": 0
 },
 "GET /admin/native-accounts": {
  "op": "listNativeAccounts",
  "cv": 0,
  "ifMatch": 0
 },
 "POST /admin/native-accounts/import": {
  "op": "importNativeAccount",
  "cv": 0,
  "ifMatch": 0
 },
 "POST /admin/native-account-authorizations": {
  "op": "startNativeAccountAuthorization",
  "cv": 0,
  "ifMatch": 0
 },
 "POST /admin/native-account-authorizations/{session_id}/poll": {
  "op": "pollNativeAccountAuthorization",
  "cv": 0,
  "ifMatch": 0
 },
 "GET /admin/native-account-authorizations/{session_id}": {
  "op": "getNativeAccountAuthorizationReceipt",
  "cv": 0,
  "ifMatch": 0
 },
 "DELETE /admin/native-account-authorizations/{session_id}": {
  "op": "cancelNativeAccountAuthorization",
  "cv": 0,
  "ifMatch": 0
 },
 "GET /admin/account-channels": {
  "op": "listAccountChannels",
  "cv": 0,
  "ifMatch": 0
 },
 "POST /admin/upstreams/{upstream_id}/account-import": {
  "op": "importChannelAccount",
  "cv": 2,
  "ifMatch": 1
 },
 "PATCH /admin/credentials/{credential_id}/status": {
  "op": "updateCredentialStatus",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/auth/login": {
  "op": "loginAdministrator",
  "cv": 0,
  "ifMatch": 0
 },
 "POST /admin/auth/password": {
  "op": "changeAdministratorPassword",
  "cv": 0,
  "ifMatch": 0
 },
 "POST /admin/auth/logout": {
  "op": "logoutAdministrator",
  "cv": 0,
  "ifMatch": 0
 },
 "POST /admin/config-versions/{config_version_id}/fork": {
  "op": "forkConfigVersion",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/config-versions/{config_version_id}/diff": {
  "op": "compareConfigVersions",
  "cv": 0,
  "ifMatch": 0
 },
 "GET /admin/resource-audit-events": {
  "op": "listManagementResourceAuditEvents",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/config-versions": {
  "op": "listConfigVersions",
  "cv": 0,
  "ifMatch": 0
 },
 "POST /admin/config-versions": {
  "op": "createConfigVersion",
  "cv": 0,
  "ifMatch": 0
 },
 "GET /admin/config-versions/{config_version_id}": {
  "op": "getConfigVersion",
  "cv": 0,
  "ifMatch": 0
 },
 "POST /admin/config-versions/{config_version_id}/validate": {
  "op": "validateConfigVersion",
  "cv": 0,
  "ifMatch": 0
 },
 "POST /admin/config-versions/{config_version_id}/publish": {
  "op": "publishConfigVersion",
  "cv": 0,
  "ifMatch": 1
 },
 "POST /admin/config-versions/rollback": {
  "op": "rollbackConfigVersion",
  "cv": 0,
  "ifMatch": 1
 },
 "GET /admin/egress-policies": {
  "op": "listEgressPolicies",
  "cv": 2,
  "ifMatch": 0
 },
 "POST /admin/egress-policies": {
  "op": "createEgressPolicy",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/egress-policies/{egress_policy_id}": {
  "op": "getEgressPolicy",
  "cv": 2,
  "ifMatch": 0
 },
 "PATCH /admin/egress-policies/{egress_policy_id}": {
  "op": "updateEgressPolicy",
  "cv": 2,
  "ifMatch": 1
 },
 "DELETE /admin/egress-policies/{egress_policy_id}": {
  "op": "deleteEgressPolicy",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/upstreams": {
  "op": "listUpstreams",
  "cv": 2,
  "ifMatch": 0
 },
 "POST /admin/upstreams": {
  "op": "createUpstream",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/upstreams/{upstream_id}": {
  "op": "getUpstream",
  "cv": 2,
  "ifMatch": 0
 },
 "PATCH /admin/upstreams/{upstream_id}": {
  "op": "updateUpstream",
  "cv": 2,
  "ifMatch": 1
 },
 "DELETE /admin/upstreams/{upstream_id}": {
  "op": "deleteUpstream",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/upstreams/{upstream_id}/endpoints": {
  "op": "createEndpoint",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/endpoints/{endpoint_id}": {
  "op": "getEndpoint",
  "cv": 2,
  "ifMatch": 0
 },
 "PATCH /admin/endpoints/{endpoint_id}": {
  "op": "updateEndpoint",
  "cv": 2,
  "ifMatch": 1
 },
 "DELETE /admin/endpoints/{endpoint_id}": {
  "op": "deleteEndpoint",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/upstreams/{upstream_id}/credentials": {
  "op": "createCredential",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/credentials/{credential_id}": {
  "op": "getCredential",
  "cv": 2,
  "ifMatch": 0
 },
 "PATCH /admin/credentials/{credential_id}": {
  "op": "updateCredential",
  "cv": 2,
  "ifMatch": 1
 },
 "DELETE /admin/credentials/{credential_id}": {
  "op": "deleteCredential",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/endpoints/{endpoint_id}/credential-bindings": {
  "op": "listEndpointCredentialBindings",
  "cv": 2,
  "ifMatch": 0
 },
 "POST /admin/endpoints/{endpoint_id}/credential-bindings": {
  "op": "createEndpointCredentialBinding",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/endpoints/{endpoint_id}/test": {
  "op": "testEndpoint",
  "cv": 2,
  "ifMatch": 0
 },
 "POST /admin/endpoints/{endpoint_id}/models/discover-preview": {
  "op": "previewCatalogDiscovery",
  "cv": 2,
  "ifMatch": 0
 },
 "POST /admin/endpoints/{endpoint_id}/models/discover-apply": {
  "op": "applyCatalogDiscovery",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/public-models": {
  "op": "listPublicModels",
  "cv": 2,
  "ifMatch": 0
 },
 "POST /admin/public-models": {
  "op": "createPublicModel",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/public-models/{public_model_id}": {
  "op": "getPublicModel",
  "cv": 2,
  "ifMatch": 0
 },
 "PATCH /admin/public-models/{public_model_id}": {
  "op": "updatePublicModel",
  "cv": 2,
  "ifMatch": 1
 },
 "DELETE /admin/public-models/{public_model_id}": {
  "op": "deletePublicModel",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/public-models/{public_model_id}/aliases": {
  "op": "createModelAlias",
  "cv": 2,
  "ifMatch": 1
 },
 "DELETE /admin/public-models/{public_model_id}/aliases": {
  "op": "deleteModelAlias",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/public-models/{public_model_id}/routes": {
  "op": "createRoute",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/accounts/inventory": {
  "op": "listAccountInventory",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/credentials": {
  "op": "listManagedCredentials",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/endpoints": {
  "op": "listManagedEndpoints",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/routes": {
  "op": "listRoutes",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/route-candidates": {
  "op": "listRouteCandidates",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/model-aliases": {
  "op": "listModelAliases",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/routes/{route_id}": {
  "op": "getRoute",
  "cv": 2,
  "ifMatch": 0
 },
 "PATCH /admin/routes/{route_id}": {
  "op": "updateRoute",
  "cv": 2,
  "ifMatch": 1
 },
 "DELETE /admin/routes/{route_id}": {
  "op": "deleteRoute",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/routes/{route_id}/candidates": {
  "op": "createRouteCandidate",
  "cv": 2,
  "ifMatch": 1
 },
 "PATCH /admin/routes/{route_id}/candidates/{candidate_id}": {
  "op": "updateRouteCandidate",
  "cv": 2,
  "ifMatch": 1
 },
 "DELETE /admin/routes/{route_id}/candidates/{candidate_id}": {
  "op": "deleteRouteCandidate",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/routes/{route_id}/validate": {
  "op": "validateRoute",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/routes/{route_id}/explain": {
  "op": "explainRoute",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/access-groups": {
  "op": "listAccessGroups",
  "cv": 2,
  "ifMatch": 0
 },
 "POST /admin/access-groups": {
  "op": "createAccessGroup",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/access-groups/{access_group_id}": {
  "op": "getAccessGroup",
  "cv": 2,
  "ifMatch": 0
 },
 "PATCH /admin/access-groups/{access_group_id}": {
  "op": "updateAccessGroup",
  "cv": 2,
  "ifMatch": 1
 },
 "DELETE /admin/access-groups/{access_group_id}": {
  "op": "deleteAccessGroup",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/access-groups/{access_group_id}/routes": {
  "op": "listAccessGroupRoutes",
  "cv": 2,
  "ifMatch": 0
 },
 "POST /admin/access-groups/{access_group_id}/routes": {
  "op": "grantAccessGroupRoute",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/client-keys": {
  "op": "listClientKeys",
  "cv": 2,
  "ifMatch": 0
 },
 "POST /admin/client-keys": {
  "op": "issueClientKey",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/client-keys/{client_key_id}": {
  "op": "getClientKey",
  "cv": 2,
  "ifMatch": 0
 },
 "PATCH /admin/client-keys/{client_key_id}": {
  "op": "updateClientKey",
  "cv": 2,
  "ifMatch": 1
 },
 "DELETE /admin/client-keys/{client_key_id}": {
  "op": "revokeClientKey",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/upstreams/{upstream_id}/codex-authorization/start": {
  "op": "startCodexEnrollment",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/upstreams/{upstream_id}/codex-authorization/cancel": {
  "op": "cancelCodexEnrollment",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/upstreams/{upstream_id}/codex-authorization/callback": {
  "op": "completeCodexEnrollment",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/upstreams/{upstream_id}/claude-authorization/start": {
  "op": "startClaudeEnrollment",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/upstreams/{upstream_id}/claude-authorization/cancel": {
  "op": "cancelClaudeEnrollment",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/upstreams/{upstream_id}/claude-authorization/callback": {
  "op": "completeClaudeEnrollment",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/credentials/{credential_id}/oauth/start": {
  "op": "startCredentialOAuth",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/credentials/{credential_id}/oauth/status": {
  "op": "getCredentialOAuthStatus",
  "cv": 2,
  "ifMatch": 0
 },
 "POST /admin/credentials/{credential_id}/oauth/cancel": {
  "op": "cancelCredentialOAuth",
  "cv": 2,
  "ifMatch": 0
 },
 "POST /admin/credentials/{credential_id}/oauth/callback": {
  "op": "completeCredentialOAuth",
  "cv": 2,
  "ifMatch": 0
 },
 "POST /admin/credentials/{credential_id}/oauth/refresh": {
  "op": "refreshCredentialOAuth",
  "cv": 2,
  "ifMatch": 0
 },
 "POST /admin/credentials/{credential_id}/export": {
  "op": "exportCredential",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/credentials/{credential_id}/metadata": {
  "op": "getCredentialMetadata",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/operations/billing-processing": {
  "op": "getBillingProcessingStatus",
  "cv": 0,
  "ifMatch": 0
 },
 "GET /admin/models/effective": {
  "op": "listEffectiveModels",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/catalog/status": {
  "op": "getCatalogStatus",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/runtime/availability": {
  "op": "getRuntimeAvailability",
  "cv": 2,
  "ifMatch": 0
 },
 "POST /admin/runtime/quota/reset": {
  "op": "requestQuotaRecovery",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/operations/account-pools": {
  "op": "listOperationalAccountPools",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/operations/provider-egress-status": {
  "op": "listProviderEgressStatus",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/compatible-proxy-pools": {
  "op": "listCompatibleProxyPools",
  "cv": 2,
  "ifMatch": 0
 },
 "POST /admin/compatible-proxy-pools": {
  "op": "createCompatibleProxyPool",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/compatible-proxy-pools/{pool_id}": {
  "op": "getCompatibleProxyPool",
  "cv": 2,
  "ifMatch": 0
 },
 "PATCH /admin/compatible-proxy-pools/{pool_id}": {
  "op": "updateCompatibleProxyPool",
  "cv": 2,
  "ifMatch": 1
 },
 "DELETE /admin/compatible-proxy-pools/{pool_id}": {
  "op": "deleteCompatibleProxyPool",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/compatible-proxy-nodes": {
  "op": "listCompatibleProxyNodes",
  "cv": 2,
  "ifMatch": 0
 },
 "POST /admin/compatible-proxy-nodes": {
  "op": "createCompatibleProxyNode",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/compatible-proxy-nodes/{node_id}": {
  "op": "getCompatibleProxyNode",
  "cv": 2,
  "ifMatch": 0
 },
 "PATCH /admin/compatible-proxy-nodes/{node_id}": {
  "op": "updateCompatibleProxyNode",
  "cv": 2,
  "ifMatch": 1
 },
 "DELETE /admin/compatible-proxy-nodes/{node_id}": {
  "op": "deleteCompatibleProxyNode",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/compatible-egress-bindings": {
  "op": "listCompatibleEgressBindings",
  "cv": 2,
  "ifMatch": 0
 },
 "POST /admin/compatible-egress-bindings": {
  "op": "createCompatibleEgressBinding",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/compatible-egress-bindings/{endpoint_id}/{credential_id}": {
  "op": "getCompatibleEgressBinding",
  "cv": 2,
  "ifMatch": 0
 },
 "PATCH /admin/compatible-egress-bindings/{endpoint_id}/{credential_id}": {
  "op": "updateCompatibleEgressBinding",
  "cv": 2,
  "ifMatch": 1
 },
 "DELETE /admin/compatible-egress-bindings/{endpoint_id}/{credential_id}": {
  "op": "deleteCompatibleEgressBinding",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/operations/channel-pin": {
  "op": "executeChannelPin",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/operations/provider-account-pools": {
  "op": "listProviderAccountPools",
  "cv": 0,
  "ifMatch": 0
 },
 "POST /admin/operations/provider-account-pools/actions": {
  "op": "applyProviderAccountPoolAction",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/operations/provider-account-pools/failures": {
  "op": "listProviderAccountFailures",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/operations/usage": {
  "op": "listOperationalUsage",
  "cv": 0,
  "ifMatch": 0
 },
 "GET /admin/operations/billing": {
  "op": "listOperationalBilling",
  "cv": 0,
  "ifMatch": 0
 },
 "GET /admin/billing/catalogs": {
  "op": "listBillingCatalogs",
  "cv": 2,
  "ifMatch": 0
 },
 "POST /admin/billing/catalogs": {
  "op": "importBillingCatalog",
  "cv": 2,
  "ifMatch": 1
 },
 "POST /admin/billing/catalogs/{catalog_version_id}/rollback": {
  "op": "rollbackBillingCatalog",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/billing/routing-price-policy": {
  "op": "getRoutingPricePolicy",
  "cv": 2,
  "ifMatch": 0
 },
 "PUT /admin/billing/routing-price-policy": {
  "op": "setRoutingPricePolicy",
  "cv": 2,
  "ifMatch": 1
 },
 "DELETE /admin/billing/routing-price-policy": {
  "op": "clearRoutingPricePolicy",
  "cv": 2,
  "ifMatch": 1
 },
 "GET /admin/requests/{request_id}/attempts": {
  "op": "listRequestAttempts",
  "cv": 0,
  "ifMatch": 0
 },
 "GET /admin/observability/metrics": {
  "op": "getObservabilityMetrics",
  "cv": 0,
  "ifMatch": 0
 },
 "GET /admin/audit-events": {
  "op": "listManagementAuditEvents",
  "cv": 0,
  "ifMatch": 0
 },
 "POST /admin/backups/preflight": {
  "op": "previewBackup",
  "cv": 0,
  "ifMatch": 0
 },
 "POST /admin/restores/preflight": {
  "op": "previewRestore",
  "cv": 0,
  "ifMatch": 0
 },
 "POST /admin/restores": {
  "op": "restoreBackup",
  "cv": 0,
  "ifMatch": 0
 },
 "POST /admin/native-accounts/{account_id}/identity": {
  "op": "refreshNativeAccountIdentity",
  "cv": 0,
  "ifMatch": 0
 },
 "GET /admin/native-accounts/{account_id}/deletion-impact": {
  "op": "getNativeAccountDeletionImpact",
  "cv": 2,
  "ifMatch": 0
 },
 "PATCH /admin/native-accounts/{account_id}": {
  "op": "updateNativeAccount",
  "cv": 0,
  "ifMatch": 0
 },
 "DELETE /admin/native-accounts/{account_id}": {
  "op": "deleteNativeAccount",
  "cv": 1,
  "ifMatch": 0
 },
 "PUT /admin/native-accounts/{account_id}/credential": {
  "op": "replaceNativeAccountCredential",
  "cv": 0,
  "ifMatch": 0
 },
 "GET /admin/native-accounts/{account_id}/usage": {
  "op": "getNativeAccountUsage",
  "cv": 0,
  "ifMatch": 0
 },
 "GET /admin/native-accounts/{account_id}/audit": {
  "op": "listNativeAccountAudit",
  "cv": 0,
  "ifMatch": 0
 },
 "POST /admin/operations/runtime/apply": {
  "op": "applyRuntimeConfiguration",
  "cv": 0,
  "ifMatch": 0
 },
 "GET /admin/system": {
  "op": "getSystemInformation",
  "cv": 0,
  "ifMatch": 0
 },
 "GET /admin/catalog/models": {
  "op": "listCatalogModels",
  "cv": 2,
  "ifMatch": 0
 },
 "POST /admin/catalog/refresh": {
  "op": "refreshCatalogModels",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/requests": {
  "op": "listRequests",
  "cv": 0,
  "ifMatch": 0
 },
 "GET /admin/requests/summary": {
  "op": "summarizeRequests",
  "cv": 0,
  "ifMatch": 0
 },
 "GET /admin/account-authorization/{session_id}": {
  "op": "getAccountAuthorizationReceipt",
  "cv": 2,
  "ifMatch": 0
 },
 "GET /admin/config-versions/{config_version_id}/account-restoration": {
  "op": "getAccountRestorationReview",
  "cv": 0,
  "ifMatch": 0
 },
 "GET /admin/credentials/{credential_id}/deletion-impact": {
  "op": "getCredentialDeletionImpact",
  "cv": 2,
  "ifMatch": 0
 }
};
