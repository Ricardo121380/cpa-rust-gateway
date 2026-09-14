//! Read-only public price metadata; no credentials or internal resource IDs leave the gateway.
use super::{
    BTreeSet, Deserialize, Duration, HttpRequest, HttpResponse, ManagementOperationsError,
    ManagementResourceHttpState, StatusCode, error_response, invalid_input, parse_json, principal,
    read_operations, web,
};
use std::io::Read;
const URL: &str = "https://models.dev/api.json";
const MAX_BYTES: u64 = 16 * 1024 * 1024;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    models: Vec<String>,
}

pub(super) async fn refresh(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    if let Err(response) = principal(&request) {
        return response;
    }
    let input = match parse_json::<Input>(&body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if input.models.is_empty()
        || input.models.len() > 512
        || input.models.iter().any(|s| s.is_empty() || s.len() > 512)
    {
        return invalid_input();
    }
    let models: BTreeSet<_> = input.models.into_iter().collect();
    let result = read_operations(&state, move || {
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|_| ManagementOperationsError::SourceUnavailable)?;
        let response = client
            .get(URL)
            .send()
            .map_err(|_| ManagementOperationsError::SourceUnavailable)?;
        if !response.status().is_success()
            || response.content_length().is_some_and(|n| n > MAX_BYTES)
        {
            return Err(ManagementOperationsError::SourceUnavailable);
        }
        let mut bytes = Vec::new();
        response
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ManagementOperationsError::SourceUnavailable)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(ManagementOperationsError::SourceUnavailable);
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|_| ManagementOperationsError::SourceUnavailable)?;
        project(&value, &models)
    })
    .await;
    match result {
        Ok(items) => HttpResponse::Ok().json(serde_json::json!({"source":"models.dev","url":URL,"currency":"USD","unit":"microunits_per_million_tokens","items":items})),
        Err(ManagementOperationsError::ReadCapacityExceeded) => error_response(StatusCode::TOO_MANY_REQUESTS,"management_price_source_busy","价格读取繁忙，请稍后重试"),
        Err(_) => error_response(StatusCode::BAD_GATEWAY,"management_price_source_unavailable","价格来源读取失败，现有价目表未改变"),
    }
}
fn rate(value: Option<&serde_json::Value>) -> Option<u64> {
    let value = value?.as_f64()? * 1_000_000.0;
    if !value.is_finite() || !(0.0..=9_007_199_254_740_991.0).contains(&value) {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some(value.round() as u64)
}
fn project(
    value: &serde_json::Value,
    models: &BTreeSet<String>,
) -> Result<Vec<serde_json::Value>, ManagementOperationsError> {
    let providers = value
        .as_object()
        .ok_or(ManagementOperationsError::SourceUnavailable)?;
    let mut rows = Vec::new();
    for (provider, data) in providers {
        if provider.len() > 128 {
            continue;
        }
        let Some(entries) = data.get("models").and_then(serde_json::Value::as_object) else {
            continue;
        };
        for (id, model) in entries {
            if !models.contains(id) {
                continue;
            }
            let cost = &model["cost"];
            let tiered = cost.get("tiers").is_some() || cost.get("context_over_200k").is_some();
            // Tiered prices cannot be flattened into the current fixed-rate catalog.
            let read = |key| if tiered { None } else { rate(cost.get(key)) };
            rows.push(serde_json::json!({"provider":provider,"model":id,"tiered":tiered,
                "input_microunits_per_million":read("input"),"output_microunits_per_million":read("output"),
                "cache_read_microunits_per_million":read("cache_read"),"cache_creation_microunits_per_million":read("cache_write"),
                "reasoning_microunits_per_million":null,"cached_microunits_per_million":null}));
            if rows.len() > 4096 {
                return Err(ManagementOperationsError::SourceUnavailable);
            }
        }
    }
    Ok(rows)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_models_preserve_sources_missing_rates_and_tiers()
    -> Result<(), ManagementOperationsError> {
        let input = serde_json::json!({"one":{"models":{"Exact":{"cost":{"input":0,"output":1.25,"cache_read":0.1}},"exact":{"cost":{"input":9}}}},"two":{"models":{"Exact":{"cost":{"input":8,"tiers":[]}}}}});
        let rows = project(&input, &BTreeSet::from(["Exact".into()]))?;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["input_microunits_per_million"], 0);
        assert_eq!(rows[0]["output_microunits_per_million"], 1_250_000);
        assert!(rows[0]["reasoning_microunits_per_million"].is_null());
        assert!(rows[1]["input_microunits_per_million"].is_null());
        assert_eq!(rows[1]["tiered"], true);
        assert!(rate(Some(&serde_json::json!(-1))).is_none());
        Ok(())
    }
}
