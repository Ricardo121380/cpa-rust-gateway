//! P10-01 management `OpenAPI` contract regression tests.

#![deny(unsafe_code)]

use std::error::Error;

use serde_json::{Map, Value};

type TestResult = Result<(), Box<dyn Error>>;

const CONTRACT: &str = include_str!("../../../docs/openapi/management-v1.json");

#[test]
fn management_contract_has_the_versioned_complete_resource_surface() -> TestResult {
    let document = document()?;
    assert_eq!(document["openapi"], "3.1.0");
    assert_eq!(document["x-contract-status"], "contract_only");
    assert!(document.get("servers").is_none());

    let paths = object(&document, "paths")?;
    for (path, methods) in required_operations() {
        let operation = paths
            .get(path)
            .and_then(Value::as_object)
            .and_then(|path_item| path_item.get(methods))
            .and_then(Value::as_object)
            .ok_or_else(|| format!("missing {methods} {path}"))?;
        assert!(operation.get("operationId").is_some(), "{methods} {path}");
        assert!(operation.get("responses").is_some(), "{methods} {path}");
    }
    assert!(paths.keys().all(|path| path.starts_with("/admin/")));
    assert!(!paths.contains_key("/admin/proxy"));
    assert!(!paths.contains_key("/admin/http"));
    assert!(!paths.contains_key("/admin/requests"));
    Ok(())
}

#[test]
fn management_contract_has_no_dangling_local_references() -> TestResult {
    let document = document()?;
    validate_references(&document, &document)?;
    Ok(())
}

#[test]
fn provider_account_pool_bounds_match_existing_runtime_scheduler_domains() -> TestResult {
    let document = document()?;
    let item = &document["components"]["schemas"]["ProviderAccountPoolItem"]["properties"];
    assert_eq!(item["priority"]["minimum"], 0);
    assert!(item["priority"].get("maximum").is_none());
    assert_eq!(item["max_concurrency"]["maximum"], 100_000);
    assert_eq!(item["active_leases"]["maximum"], 100_000);
    Ok(())
}

#[test]
fn provider_account_operator_contract_is_bounded_audited_and_value_free() -> TestResult {
    let document = document()?;
    let paths = object(&document, "paths")?;
    let action = &paths["/admin/operations/provider-account-pools/actions"]["post"];
    assert_eq!(action["operationId"], "applyProviderAccountPoolAction");
    assert_eq!(action["x-delivery-phase"], "P13-06C");
    assert_eq!(action["x-audit-action"], "provider_account_operator_action");
    assert!(
        operation_parameters(
            &document,
            "/admin/operations/provider-account-pools/actions",
            "post",
        )?
        .iter()
        .any(|parameter| parameter["$ref"] == "#/components/parameters/ConfigVersion")
    );

    let schemas = object(&document["components"], "schemas")?;
    let input = schemas
        .get("ProviderAccountOperatorAction")
        .ok_or("missing ProviderAccountOperatorAction")?;
    assert_eq!(input["additionalProperties"], false);
    assert_eq!(
        input["properties"]["cooldown_ms"]["anyOf"][0]["minimum"],
        1_000
    );
    assert_eq!(
        input["properties"]["cooldown_ms"]["anyOf"][0]["maximum"],
        86_400_000
    );
    let receipt = schemas
        .get("ProviderAccountOperatorActionReceipt")
        .ok_or("missing ProviderAccountOperatorActionReceipt")?;
    assert_eq!(receipt["additionalProperties"], false);

    let failure = schemas
        .get("ProviderAccountFailureItem")
        .ok_or("missing ProviderAccountFailureItem")?;
    assert_eq!(failure["additionalProperties"], false);
    let properties = object(failure, "properties")?;
    for forbidden in [
        "upstream_model",
        "url",
        "headers",
        "body",
        "cookie",
        "secret",
        "digest",
        "raw_error",
    ] {
        assert!(
            !properties.contains_key(forbidden),
            "ProviderAccountFailureItem exposes {forbidden}"
        );
    }
    assert_eq!(
        schemas["ProviderAccountFailurePage"]["properties"]["items"]["maxItems"],
        100
    );
    Ok(())
}

#[test]
fn provider_egress_status_contract_is_read_only_closed_and_value_free() -> TestResult {
    let document = document()?;
    let paths = object(&document, "paths")?;
    let path = &paths["/admin/operations/provider-egress-status"];
    let operation = &path["get"];
    assert_eq!(operation["operationId"], "listProviderEgressStatus");
    assert_eq!(operation["x-delivery-phase"], "P13-11E4");
    assert!(operation.get("requestBody").is_none());
    assert!(operation.get("x-audit-action").is_none());
    assert!(path.get("post").is_none());
    assert!(path.get("patch").is_none());
    assert!(path.get("delete").is_none());
    for status in ["200", "400", "409", "503", "default"] {
        assert!(
            operation["responses"].get(status).is_some(),
            "missing {status}"
        );
    }

    let parameters =
        operation_parameters(&document, "/admin/operations/provider-egress-status", "get")?;
    assert_eq!(parameters.len(), 9);
    assert!(
        parameters
            .iter()
            .any(|parameter| parameter["$ref"] == "#/components/parameters/ConfigVersion")
    );
    assert!(
        !parameters
            .iter()
            .any(|parameter| parameter["$ref"] == "#/components/parameters/IfMatch")
    );
    for name in [
        "ProviderEgressStatusProviderId",
        "ProviderEgressStatusUpstreamId",
        "ProviderEgressStatusChannelId",
        "ProviderEgressStatusDomain",
        "ProviderEgressStatusState",
        "ProviderEgressStatusCredentialId",
        "ProviderEgressStatusLimit",
        "ProviderEgressStatusCursor",
    ] {
        let reference = format!("#/components/parameters/{name}");
        assert!(
            parameters
                .iter()
                .any(|parameter| parameter["$ref"] == reference),
            "missing query parameter {name}"
        );
    }

    let schemas = object(&document["components"], "schemas")?;
    let page = &schemas["ProviderEgressStatusPage"];
    assert_eq!(page["additionalProperties"], false);
    assert_eq!(page["properties"]["items"]["maxItems"], 100);
    assert_eq!(
        page["properties"]["next_cursor"]["anyOf"][0]["maxLength"],
        4_096
    );
    assert_eq!(
        document["components"]["parameters"]["ProviderEgressStatusCursor"]["schema"]["maxLength"],
        4_096
    );
    let success = &document["components"]["responses"]["ProviderEgressStatusPage"];
    assert_eq!(
        success["headers"]["Cache-Control"]["schema"]["const"],
        "no-store"
    );
    assert!(success["headers"].get("ETag").is_some());
    assert_provider_egress_status_item_schemas(schemas)?;
    Ok(())
}

fn assert_provider_egress_status_item_schemas(
    schemas: &serde_json::Map<String, Value>,
) -> TestResult {
    let union = &schemas["ProviderEgressStatusItem"];
    assert_eq!(union["oneOf"].as_array().map(Vec::len), Some(3));
    assert_eq!(union["discriminator"]["propertyName"], "domain");
    for (schema_name, domain) in [
        ("ProviderEgressStatusEgressItem", "egress"),
        ("ProviderEgressStatusSessionItem", "session"),
        ("ProviderEgressStatusClearanceItem", "clearance"),
    ] {
        let schema = &schemas[schema_name];
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["properties"]["domain"]["enum"],
            serde_json::json!([domain])
        );
        let properties = object(schema, "properties")?;
        for forbidden in [
            "url",
            "proxy_url",
            "proxy_endpoint",
            "headers",
            "cookie",
            "token",
            "secret",
            "ciphertext",
            "request_body",
            "raw_error",
            "ticket",
            "generation",
        ] {
            assert!(
                !properties.contains_key(forbidden),
                "{schema_name} exposes {forbidden}"
            );
        }
    }
    Ok(())
}

#[test]
fn routing_price_policy_contract_is_closed_revisioned_and_value_free() -> TestResult {
    let document = document()?;
    let paths = object(&document, "paths")?;
    let path = &paths["/admin/billing/routing-price-policy"];
    assert_eq!(path["get"]["operationId"], "getRoutingPricePolicy");
    assert_eq!(path["put"]["operationId"], "setRoutingPricePolicy");
    assert_eq!(path["delete"]["operationId"], "clearRoutingPricePolicy");
    assert_eq!(path["put"]["x-audit-action"], "routing_price_policy_set");
    assert_eq!(
        path["delete"]["x-audit-action"],
        "routing_price_policy_cleared"
    );
    for method in ["put", "delete"] {
        assert!(
            operation_parameters(&document, "/admin/billing/routing-price-policy", method,)?
                .iter()
                .any(|parameter| parameter["$ref"] == "#/components/parameters/IfMatch")
        );
    }

    let schemas = object(&document["components"], "schemas")?;
    for schema_name in ["RoutingPricePolicyInput", "RoutingPricePolicy"] {
        let schema = &schemas[schema_name];
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["properties"]["comparison"]["enum"],
            serde_json::json!(["rate_dominance_v1"])
        );
        assert!(schema["properties"].get("cost_microunits").is_none());
        assert!(schema["properties"].get("token_estimate").is_none());
    }
    assert_eq!(
        schemas["RouteExplain"]["properties"]["candidates"]["items"]["properties"]["price_evidence"]
            ["enum"],
        serde_json::json!([
            "dominant",
            "equal",
            "dominated",
            "incomparable",
            "unpriced",
            "not_evaluated",
            "disabled"
        ])
    );
    assert_eq!(
        schemas["RouteExplain"]["required"],
        serde_json::json!(["route_id", "candidates", "price_policy"])
    );
    assert_eq!(
        schemas["RouteExplain"]["properties"]["candidates"]["items"]["required"],
        serde_json::json!(["candidate_id", "decision", "price_evidence"])
    );
    Ok(())
}

#[test]
fn channel_pin_contract_is_one_shot_revisioned_closed_and_value_free() -> TestResult {
    let document = document()?;
    let paths = object(&document, "paths")?;
    let path = &paths["/admin/operations/channel-pin"];
    let operation = &path["post"];
    assert_eq!(operation["operationId"], "executeChannelPin");
    assert_eq!(operation["x-delivery-phase"], "P13-08");
    assert_eq!(operation["x-audit-action"], "channel_pin_requested");
    let parameters = operation_parameters(&document, "/admin/operations/channel-pin", "post")?;
    assert!(
        parameters
            .iter()
            .any(|parameter| parameter["$ref"] == "#/components/parameters/ConfigVersion")
    );
    assert!(
        parameters
            .iter()
            .any(|parameter| parameter["$ref"] == "#/components/parameters/IfMatch")
    );

    let schemas = object(&document["components"], "schemas")?;
    let input = &schemas["ChannelPinInput"];
    assert_eq!(input["additionalProperties"], false);
    assert_eq!(
        input["properties"]["protocol"]["enum"],
        serde_json::json!([
            "openai_chat_completions",
            "openai_responses",
            "anthropic_messages"
        ])
    );
    assert_eq!(
        input["properties"]["mode"]["enum"],
        serde_json::json!(["json", "sse"])
    );
    let receipt = &schemas["ChannelPin"];
    assert_eq!(receipt["additionalProperties"], false);
    for required in [
        "request_id",
        "config_version_id",
        "config_revision",
        "provider_id",
        "channel_id",
        "route_id",
        "credential_id",
        "requested_model",
        "protocol",
        "mode",
        "outcome",
        "upstream_sent",
        "attempt_count",
        "response_started",
        "observed_at_ms",
    ] {
        assert!(
            receipt["required"]
                .as_array()
                .is_some_and(|fields| { fields.iter().any(|field| field == required) })
        );
    }
    let properties = object(receipt, "properties")?;
    for forbidden in [
        "url",
        "path",
        "headers",
        "cookies",
        "body",
        "token",
        "secret",
        "ciphertext",
        "digest",
        "raw_error",
        "status_code",
    ] {
        assert!(
            !properties.contains_key(forbidden),
            "ChannelPin exposes {forbidden}"
        );
    }
    Ok(())
}

#[test]
fn management_contract_is_separate_from_client_keys_and_uses_optimistic_transactions() -> TestResult
{
    let document = document()?;
    let security = document
        .get("security")
        .and_then(Value::as_array)
        .ok_or("missing root security")?;
    assert_eq!(security.len(), 1);
    assert_eq!(security[0]["ManagementKey"], Value::Array(Vec::new()));
    let management_key = &document["components"]["securitySchemes"]["ManagementKey"];
    assert_eq!(management_key["type"], "apiKey");
    assert_eq!(management_key["in"], "header");
    assert_eq!(management_key["name"], "X-Management-Key");

    for (path, method) in concurrent_write_operations() {
        let parameters = operation_parameters(&document, path, method)?;
        assert!(
            parameters
                .iter()
                .any(|parameter| { parameter["$ref"] == "#/components/parameters/ConfigVersion" }),
            "{method} {path} must select exactly one config version"
        );
        assert!(
            parameters
                .iter()
                .any(|parameter| parameter["$ref"] == "#/components/parameters/IfMatch"),
            "{method} {path} must use If-Match"
        );
    }
    Ok(())
}

#[test]
fn management_contract_exposes_secrets_only_at_their_explicit_write_or_once_boundary() -> TestResult
{
    let document = document()?;
    let schemas = object(&document["components"], "schemas")?;

    let credential = object(
        schemas.get("Credential").ok_or("missing Credential")?,
        "properties",
    )?;
    for forbidden in ["secret", "encrypted_secret", "token", "cookie"] {
        assert!(
            !credential.contains_key(forbidden),
            "Credential exposes {forbidden}"
        );
    }
    let credential_input = object(
        schemas
            .get("CredentialInput")
            .ok_or("missing CredentialInput")?,
        "properties",
    )?;
    assert_eq!(credential_input["secret"]["writeOnly"], true);
    assert_eq!(
        credential_input["secret"]["x-secret-lifecycle"],
        "write_only"
    );

    let client_key = object(
        schemas.get("ClientKey").ok_or("missing ClientKey")?,
        "properties",
    )?;
    for forbidden in ["key", "secret", "digest", "secret_digest"] {
        assert!(
            !client_key.contains_key(forbidden),
            "ClientKey exposes {forbidden}"
        );
    }
    let issued = object(
        schemas
            .get("IssuedClientKey")
            .ok_or("missing IssuedClientKey")?,
        "properties",
    )?;
    let issued_key = issued
        .get("key")
        .ok_or("IssuedClientKey must contain the one-time key")?;
    assert_eq!(issued_key["x-secret-lifecycle"], "display_once");
    assert_ne!(issued_key["writeOnly"], true);

    let error = object(schemas.get("Error").ok_or("missing Error")?, "properties")?;
    let error_payload = object(
        error.get("error").ok_or("missing error payload")?,
        "properties",
    )?;
    for forbidden in ["detail", "body", "headers", "url", "secret"] {
        assert!(
            !error_payload.contains_key(forbidden),
            "Error exposes {forbidden}"
        );
    }
    Ok(())
}

#[test]
fn attempt_stage_contract_is_optional_closed_and_value_free() -> TestResult {
    let document = document()?;
    let schemas = object(&document["components"], "schemas")?;
    let attempt = schemas.get("Attempt").ok_or("missing Attempt")?;
    let properties = object(attempt, "properties")?;
    let stage = properties.get("stage").ok_or("missing Attempt stage")?;
    let values = stage
        .get("enum")
        .and_then(Value::as_array)
        .ok_or("Attempt stage must be a closed enum")?;
    assert_eq!(
        values,
        &vec![
            Value::String("request_conversion".to_owned()),
            Value::String("egress_admission".to_owned()),
            Value::String("http_transport".to_owned()),
            Value::String("http_status".to_owned()),
            Value::String("content_type".to_owned()),
            Value::String("body_read".to_owned()),
            Value::String("decoder".to_owned()),
            Value::String("sse_bootstrap".to_owned()),
        ]
    );
    let required = attempt
        .get("required")
        .and_then(Value::as_array)
        .ok_or("Attempt must declare required fields")?;
    assert!(!required.iter().any(|value| value == "stage"));
    for forbidden in [
        "url",
        "header",
        "body",
        "model",
        "status_code",
        "error",
        "timestamp",
        "token",
        "digest",
    ] {
        assert!(
            !properties.contains_key(forbidden),
            "Attempt exposes {forbidden}"
        );
    }
    Ok(())
}

#[test]
fn output_schemas_are_closed_and_admit_their_declared_relationship_fields() -> TestResult {
    let document = document()?;
    let schemas = object(&document["components"], "schemas")?;
    for (schema_name, relationship_fields) in [
        ("Endpoint", &["upstream_id"][..]),
        ("Binding", &["endpoint_id", "upstream_id"][..]),
        ("Alias", &["public_model_id"][..]),
        ("Route", &["public_model_id"][..]),
        ("Candidate", &["route_id"][..]),
        ("AccessGroupRoute", &["access_group_id"][..]),
        ("IssuedClientKey", &["key"][..]),
    ] {
        let schema = schemas
            .get(schema_name)
            .and_then(Value::as_object)
            .ok_or_else(|| format!("missing output schema {schema_name}"))?;
        assert_eq!(schema.get("type"), Some(&Value::String("object".into())));
        assert_eq!(
            schema.get("additionalProperties"),
            Some(&Value::Bool(false))
        );
        assert!(
            schema.get("allOf").is_none(),
            "{schema_name} cannot extend a closed input schema via allOf"
        );
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("{schema_name} needs properties"))?;
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("{schema_name} needs required fields"))?;
        for field in relationship_fields {
            assert!(
                properties.contains_key(*field),
                "{schema_name} lacks {field}"
            );
            assert!(
                required.iter().any(|value| value == field),
                "{schema_name} does not require {field}"
            );
        }
    }
    Ok(())
}

#[test]
fn management_contract_defers_implementation_without_creating_an_implicit_admin_listener()
-> TestResult {
    let document = document()?;
    let paths = object(&document, "paths")?;
    let mut deferred = 0_usize;
    for path in [
        "/admin/endpoints/{endpoint_id}/test",
        "/admin/endpoints/{endpoint_id}/models/discover-preview",
        "/admin/routes/{route_id}/explain",
        "/admin/catalog/status",
        "/admin/audit-events",
        "/admin/backups/preflight",
        "/admin/restores",
    ] {
        let item = paths
            .get(path)
            .and_then(Value::as_object)
            .ok_or_else(|| format!("missing deferred path {path}"))?;
        let operation = item
            .values()
            .find(|candidate| candidate.get("x-delivery-phase").is_some())
            .ok_or_else(|| format!("missing delivery phase for {path}"))?;
        assert!(operation["x-delivery-phase"].as_str().is_some());
        deferred = deferred.saturating_add(1);
    }
    assert_eq!(deferred, 7);
    Ok(())
}

fn document() -> Result<Value, serde_json::Error> {
    serde_json::from_str(CONTRACT)
}

fn object<'a>(value: &'a Value, name: &str) -> Result<&'a Map<String, Value>, Box<dyn Error>> {
    value
        .get(name)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("missing object {name}").into())
}

fn operation_parameters(
    document: &Value,
    path: &str,
    method: &str,
) -> Result<Vec<Value>, Box<dyn Error>> {
    let paths = object(document, "paths")?;
    let item = paths
        .get(path)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("missing path {path}"))?;
    let mut parameters = item
        .get("parameters")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let operation = item
        .get(method)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("missing {method} {path}"))?;
    if let Some(operation_parameters) = operation.get("parameters").and_then(Value::as_array) {
        parameters.extend(operation_parameters.iter().cloned());
    }
    Ok(parameters)
}

fn required_operations() -> [(&'static str, &'static str); 58] {
    [
        ("/admin/config-versions", "get"),
        ("/admin/config-versions", "post"),
        ("/admin/config-versions/{config_version_id}/publish", "post"),
        ("/admin/config-versions/rollback", "post"),
        ("/admin/egress-policies", "post"),
        ("/admin/upstreams", "get"),
        ("/admin/upstreams", "post"),
        ("/admin/upstreams/{upstream_id}/endpoints", "post"),
        ("/admin/upstreams/{upstream_id}/credentials", "post"),
        ("/admin/endpoints/{endpoint_id}/credential-bindings", "post"),
        ("/admin/compatible-proxy-pools", "get"),
        ("/admin/compatible-proxy-pools", "post"),
        ("/admin/compatible-proxy-pools/{pool_id}", "get"),
        ("/admin/compatible-proxy-pools/{pool_id}", "patch"),
        ("/admin/compatible-proxy-pools/{pool_id}", "delete"),
        ("/admin/compatible-proxy-nodes", "get"),
        ("/admin/compatible-proxy-nodes", "post"),
        ("/admin/compatible-proxy-nodes/{node_id}", "get"),
        ("/admin/compatible-proxy-nodes/{node_id}", "patch"),
        ("/admin/compatible-proxy-nodes/{node_id}", "delete"),
        ("/admin/compatible-egress-bindings", "get"),
        ("/admin/compatible-egress-bindings", "post"),
        (
            "/admin/compatible-egress-bindings/{endpoint_id}/{credential_id}",
            "get",
        ),
        (
            "/admin/compatible-egress-bindings/{endpoint_id}/{credential_id}",
            "patch",
        ),
        (
            "/admin/compatible-egress-bindings/{endpoint_id}/{credential_id}",
            "delete",
        ),
        ("/admin/endpoints/{endpoint_id}/test", "post"),
        (
            "/admin/endpoints/{endpoint_id}/models/discover-preview",
            "post",
        ),
        (
            "/admin/endpoints/{endpoint_id}/models/discover-apply",
            "post",
        ),
        ("/admin/public-models", "post"),
        ("/admin/public-models/{public_model_id}/aliases", "post"),
        ("/admin/public-models/{public_model_id}/routes", "post"),
        ("/admin/routes/{route_id}/candidates", "post"),
        ("/admin/routes/{route_id}/validate", "post"),
        ("/admin/routes/{route_id}/explain", "get"),
        ("/admin/billing/routing-price-policy", "get"),
        ("/admin/billing/routing-price-policy", "put"),
        ("/admin/billing/routing-price-policy", "delete"),
        ("/admin/access-groups", "post"),
        ("/admin/access-groups/{access_group_id}/routes", "post"),
        ("/admin/client-keys", "post"),
        ("/admin/credentials/{credential_id}/oauth/start", "post"),
        ("/admin/credentials/{credential_id}/oauth/status", "get"),
        ("/admin/catalog/status", "get"),
        ("/admin/runtime/availability", "get"),
        ("/admin/runtime/quota/reset", "post"),
        ("/admin/operations/account-pools", "get"),
        ("/admin/operations/provider-egress-status", "get"),
        ("/admin/operations/channel-pin", "post"),
        ("/admin/operations/provider-account-pools", "get"),
        ("/admin/operations/provider-account-pools/actions", "post"),
        ("/admin/operations/provider-account-pools/failures", "get"),
        ("/admin/operations/usage", "get"),
        ("/admin/requests/{request_id}/attempts", "get"),
        ("/admin/audit-events", "get"),
        ("/admin/backups/preflight", "post"),
        ("/admin/restores/preflight", "post"),
        ("/admin/restores", "post"),
        ("/admin/client-keys/{client_key_id}", "delete"),
    ]
}

fn concurrent_write_operations() -> [(&'static str, &'static str); 31] {
    [
        ("/admin/egress-policies", "post"),
        ("/admin/egress-policies/{egress_policy_id}", "patch"),
        ("/admin/upstreams", "post"),
        ("/admin/upstreams/{upstream_id}", "patch"),
        ("/admin/upstreams/{upstream_id}/endpoints", "post"),
        ("/admin/endpoints/{endpoint_id}", "patch"),
        ("/admin/upstreams/{upstream_id}/credentials", "post"),
        ("/admin/credentials/{credential_id}", "patch"),
        ("/admin/endpoints/{endpoint_id}/credential-bindings", "post"),
        ("/admin/compatible-proxy-pools", "post"),
        ("/admin/compatible-proxy-pools/{pool_id}", "patch"),
        ("/admin/compatible-proxy-pools/{pool_id}", "delete"),
        ("/admin/compatible-proxy-nodes", "post"),
        ("/admin/compatible-proxy-nodes/{node_id}", "patch"),
        ("/admin/compatible-proxy-nodes/{node_id}", "delete"),
        ("/admin/compatible-egress-bindings", "post"),
        (
            "/admin/compatible-egress-bindings/{endpoint_id}/{credential_id}",
            "patch",
        ),
        (
            "/admin/compatible-egress-bindings/{endpoint_id}/{credential_id}",
            "delete",
        ),
        (
            "/admin/endpoints/{endpoint_id}/models/discover-apply",
            "post",
        ),
        ("/admin/public-models", "post"),
        ("/admin/public-models/{public_model_id}", "patch"),
        ("/admin/public-models/{public_model_id}/aliases", "post"),
        ("/admin/public-models/{public_model_id}/routes", "post"),
        ("/admin/routes/{route_id}", "patch"),
        ("/admin/routes/{route_id}/candidates", "post"),
        ("/admin/access-groups", "post"),
        ("/admin/access-groups/{access_group_id}/routes", "post"),
        ("/admin/client-keys", "post"),
        ("/admin/client-keys/{client_key_id}", "delete"),
        ("/admin/billing/routing-price-policy", "put"),
        ("/admin/billing/routing-price-policy", "delete"),
    ]
}

fn validate_references(document: &Value, value: &Value) -> TestResult {
    match value {
        Value::Object(object) => {
            if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
                let fragment = reference
                    .strip_prefix('#')
                    .ok_or_else(|| format!("non-local reference {reference}"))?;
                assert!(
                    document.pointer(fragment).is_some(),
                    "dangling local reference {reference}"
                );
            }
            for child in object.values() {
                validate_references(document, child)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                validate_references(document, child)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}
