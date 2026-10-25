//! Reviewed public prices. Source reads never change catalogs; confirmation uses retained content.
use super::{
    BillingCatalogImport, BillingCatalogSource, BillingPriceEntry, ConfigRevision, ConfigVersionId,
    MAX_BILLING_JSON_INTEGER, ManagementOperationsError, ManagementResourceHttpState,
    billing_catalog_mutation_response, bounded_text, error_response, internal_error, invalid_input,
    management_error, parse_json, principal, read_context, read_operations, revisioned_json,
    runtime_observed_at, service, write_context,
};
use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use gateway_control::billing_service::{find_effective_price_catalog, find_price_entry};
use gateway_store::billing_ledger::{BillingPriceCatalog, billing_catalog_fingerprint};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json, value::RawValue};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    sync::Arc,
    time::{Duration, Instant},
};

const URL: &str = "https://models.dev/api.json";
const MAX_BYTES: u64 = 16 * 1024 * 1024;
const TTL: Duration = Duration::from_mins(10);
const MAX_REVIEW_BYTES: usize = 524_288;

/// Server-owned source. No browser-supplied URL, authentication or private routing identity is sent.
pub trait PriceSource: Send + Sync {
    /// Returns a bounded public models.dev-format document. Errors contain no source payload.
    /// # Errors
    /// A failed read returns SourceUnavailable and never changes a catalog.
    fn fetch(&self) -> Result<Vec<u8>, ManagementOperationsError>;
}
pub(super) struct ModelsDev;
impl PriceSource for ModelsDev {
    fn fetch(&self) -> Result<Vec<u8>, ManagementOperationsError> {
        let failure = || ManagementOperationsError::SourceUnavailable;
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|_| failure())?;
        let response = client.get(URL).send().map_err(|_| failure())?;
        if !response.status().is_success()
            || response.content_length().is_some_and(|n| n > MAX_BYTES)
        {
            return Err(failure());
        }
        let mut bytes = Vec::new();
        response
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| failure())?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(failure());
        }
        Ok(bytes)
    }
}
impl ManagementResourceHttpState {
    /// Installs an explicit server-owned source, primarily for deterministic controlled acceptance.
    #[must_use]
    pub fn with_price_source(mut self, source: Arc<dyn PriceSource>) -> Self {
        self.price_source = source;
        self
    }
}

pub(super) fn configure(config: &mut web::ServiceConfig) {
    config
        .route("/billing/price-source/preview", web::post().to(preview))
        .route("/billing/price-source/apply", web::post().to(apply))
        .route("/billing/catalogs/{id}/review", web::get().to(review));
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    models: Vec<String>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Mapping {
    source_provider: String,
    source_model: String,
    provider_id: String,
    channel_id: String,
    model: String,
    /// Explicit operator prices, in integer microunits per million, only for absent source rates.
    #[serde(default)]
    missing_rates: Rates,
}
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Rates {
    input: Option<u64>,
    output: Option<u64>,
    reasoning: Option<u64>,
    cache_read: Option<u64>,
    cache_creation: Option<u64>,
    cached: Option<u64>,
}
impl Rates {
    const fn values(&self) -> [Option<u64>; 6] {
        [
            self.input,
            self.output,
            self.reasoning,
            self.cache_read,
            self.cache_creation,
            self.cached,
        ]
    }
    fn entry(&self, mapping: &Mapping) -> Option<BillingPriceEntry> {
        Some(BillingPriceEntry {
            provider_id: mapping.provider_id.clone(),
            channel_id: mapping.channel_id.clone(),
            model: mapping.model.clone(),
            input_microunits_per_million: self.input?,
            output_microunits_per_million: self.output?,
            reasoning_microunits_per_million: self.reasoning?,
            cache_read_microunits_per_million: self.cache_read?,
            cache_creation_microunits_per_million: self.cache_creation?,
            cached_microunits_per_million: self.cached?,
        })
    }
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PreviewInput {
    catalog_version_id: String,
    effective_at_ms: u64,
    mappings: Vec<Mapping>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplyInput {
    preview_id: String,
    content_digest: String,
}
#[derive(Deserialize)]
struct SourceProvider {
    models: BTreeMap<String, SourceModel>,
}
#[derive(Deserialize)]
struct SourceModel {
    #[serde(default)]
    cost: BTreeMap<String, Box<RawValue>>,
    currency: Option<String>,
    unit: Option<String>,
}
struct Observation {
    source: BTreeMap<String, SourceProvider>,
    digest: String,
    observed_at_ms: u64,
}
#[derive(Clone)]
struct Retained {
    owner: String,
    version: ConfigVersionId,
    revision: ConfigRevision,
    deadline: Instant,
    source_digest: String,
    catalog_digest: String,
    content_digest: String,
    input: BillingCatalogImport,
    evidence: String,
}
#[derive(Default)]
pub(super) struct Previews(BTreeMap<String, Retained>);
impl Previews {
    fn prune(&mut self) {
        self.0.retain(|_, p| p.deadline > Instant::now());
    }
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn conflict() -> HttpResponse {
    error_response(
        StatusCode::CONFLICT,
        "management_price_preview_conflict",
        "价格、来源或预览已改变，请重新预览并确认",
    )
}
fn source_failure(error: ManagementOperationsError) -> HttpResponse {
    if matches!(error, ManagementOperationsError::ReadCapacityExceeded) {
        error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "management_price_source_busy",
            "价格读取繁忙，请稍后重试",
        )
    } else {
        error_response(
            StatusCode::BAD_GATEWAY,
            "management_price_source_unavailable",
            "价格来源读取失败，现有价目表未改变",
        )
    }
}
async fn observe(
    state: &web::Data<ManagementResourceHttpState>,
) -> Result<Observation, HttpResponse> {
    let source = Arc::clone(&state.price_source);
    let (parsed, hash) = read_operations(state, move || {
        let bytes = source.fetch()?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(ManagementOperationsError::SourceUnavailable);
        }
        let parsed = serde_json::from_slice::<BTreeMap<String, SourceProvider>>(&bytes)
            .map_err(|_| ManagementOperationsError::SourceUnavailable)?;
        Ok((parsed, digest(&bytes)))
    })
    .await
    .map_err(source_failure)?;
    let observed_at_ms =
        u64::try_from(runtime_observed_at(state)?).map_err(|_| internal_error())?;
    Ok(Observation {
        source: parsed,
        digest: hash,
        observed_at_ms,
    })
}

/// Exact decimal scaling from original JSON text; never rounds an imprecise or unknown price.
fn rate(value: &RawValue) -> Result<u64, &'static str> {
    let text = value.get();
    if text.starts_with('-') || text.len() > 128 {
        return Err("invalid_rate");
    }
    let (mantissa, exponent) = match text.split_once(['e', 'E']) {
        Some((m, e)) => (m, e.parse::<i32>().map_err(|_| "invalid_rate")?),
        None => (text, 0),
    };
    let decimals = mantissa.split_once('.').map_or(0, |(_, f)| f.len());
    let digits = mantissa.replace('.', "");
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err("invalid_rate");
    }
    let mut number = digits.parse::<u128>().map_err(|_| "rate_overflow")?;
    if number == 0 {
        return Ok(0);
    }
    let scale =
        6_i64 + i64::from(exponent) - i64::try_from(decimals).map_err(|_| "invalid_rate")?;
    if scale.unsigned_abs() > 38 {
        return Err("precision_conflict");
    }
    let power = 10_u128
        .checked_pow(u32::try_from(scale.unsigned_abs()).map_err(|_| "invalid_rate")?)
        .ok_or("rate_overflow")?;
    if scale < 0 {
        if !number.is_multiple_of(power) {
            return Err("precision_conflict");
        }
        number /= power;
    } else {
        number = number.checked_mul(power).ok_or("rate_overflow")?;
    }
    let number = u64::try_from(number).map_err(|_| "rate_overflow")?;
    if number > MAX_BILLING_JSON_INTEGER {
        return Err("rate_overflow");
    }
    Ok(number)
}
fn source_rates(model: &SourceModel) -> (Rates, Vec<&'static str>) {
    let mut issues = Vec::new();
    if model.currency.as_deref().is_some_and(|s| s != "USD")
        || model
            .unit
            .as_deref()
            .is_some_and(|s| s != "usd_per_million_tokens")
        || model.cost.contains_key("currency")
        || model.cost.contains_key("unit")
    {
        issues.push("unit_conflict");
    }
    if model.cost.keys().any(|s| {
        !matches!(
            s.as_str(),
            "input" | "output" | "cache_read" | "cache_write" | "currency" | "unit"
        )
    }) {
        issues.push("unsupported_tier_or_rate");
    }
    if !issues.is_empty() {
        return (Rates::default(), issues);
    }
    let mut read = |key: &str| match model.cost.get(key) {
        None => None,
        Some(v) if v.get() == "null" => None,
        Some(v) => match rate(v) {
            Ok(n) => Some(n),
            Err(e) => {
                issues.push(e);
                None
            }
        },
    };
    let rates = Rates {
        input: read("input"),
        output: read("output"),
        cache_read: read("cache_read"),
        cache_creation: read("cache_write"),
        ..Rates::default()
    };
    (rates, issues)
}
fn merge_rates(source: &Rates, explicit: &Rates, issues: &mut Vec<&'static str>) -> Rates {
    let mut merged = [None; 6];
    for (i, (observed, supplied)) in source
        .values()
        .into_iter()
        .zip(explicit.values())
        .enumerate()
    {
        if supplied.is_some_and(|n| n > MAX_BILLING_JSON_INTEGER) {
            issues.push("rate_overflow");
        }
        if observed.is_some() && supplied.is_some() && observed != supplied {
            issues.push("rate_conflict");
        }
        merged[i] = observed.or(supplied);
    }
    let [input, output, reasoning, cache_read, cache_creation, cached] = merged;
    Rates {
        input,
        output,
        reasoning,
        cache_read,
        cache_creation,
        cached,
    }
}

pub(super) async fn refresh(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    if let Err(e) = principal(&request) {
        return e;
    }
    let input = match parse_json::<Input>(&body) {
        Ok(v) => v,
        Err(e) => return e,
    };
    if input.models.is_empty()
        || input.models.len() > 512
        || input.models.iter().any(|s| s.is_empty() || s.len() > 512)
    {
        return invalid_input();
    }
    let observation = match observe(&state).await {
        Ok(o) => o,
        Err(e) => return e,
    };
    let mut items = Vec::new();
    for id in input.models.into_iter().collect::<BTreeSet<_>>() {
        let mut matched = false;
        for (provider, data) in &observation.source {
            if provider.len() > 128 {
                continue;
            }
            if let Some(model) = data.models.get(&id) {
                matched = true;
                let (rates, issues) = source_rates(model);
                items.push(json!({"provider":provider,"model":id,"tiered":issues.contains(&"unsupported_tier_or_rate"),"issues":issues,
                    "input_microunits_per_million":rates.input,"output_microunits_per_million":rates.output,
                    "cache_read_microunits_per_million":rates.cache_read,"cache_creation_microunits_per_million":rates.cache_creation,
                    "reasoning_microunits_per_million":rates.reasoning,"cached_microunits_per_million":rates.cached}));
                if items.len() > 4096 {
                    return source_failure(ManagementOperationsError::SourceUnavailable);
                }
            }
        }
        if !matched {
            items.push(json!({"provider":null,"model":id,"tiered":false,"issues":["unknown_model"],
                "input_microunits_per_million":null,"output_microunits_per_million":null,"reasoning_microunits_per_million":null,
                "cache_read_microunits_per_million":null,"cache_creation_microunits_per_million":null,"cached_microunits_per_million":null}));
        }
    }
    HttpResponse::Ok().json(json!({"source":"models.dev","url":URL,"currency":"USD","unit":"microunits_per_million_tokens",
        "observed_at_ms":observation.observed_at_ms,"source_digest":observation.digest,"items":items}))
}

fn validate_input(input: &PreviewInput, now: u64) -> Result<(), HttpResponse> {
    bounded_text(input.catalog_version_id.clone(), 128)?;
    if input.mappings.is_empty()
        || input.mappings.len() > 512
        || input.effective_at_ms < now
        || input.effective_at_ms > MAX_BILLING_JSON_INTEGER
    {
        return Err(invalid_input());
    }
    let mut keys = BTreeSet::new();
    for m in &input.mappings {
        for (s, bound) in [
            (&m.source_provider, 128),
            (&m.source_model, 512),
            (&m.provider_id, 128),
            (&m.channel_id, 128),
            (&m.model, 512),
        ] {
            bounded_text(s.clone(), bound)?;
        }
        if !keys.insert((&m.provider_id, &m.channel_id, &m.model)) {
            return Err(invalid_input());
        }
    }
    Ok(())
}
fn proposal(
    input: &PreviewInput,
    observation: &Observation,
    catalogs: &[BillingPriceCatalog],
) -> Result<(Value, Vec<BillingPriceEntry>, bool), HttpResponse> {
    let mut rows = Vec::new();
    let mut entries = Vec::new();
    let mut applicable = true;
    let mut proposed_keys = BTreeSet::new();
    for mapping in &input.mappings {
        proposed_keys.insert((&mapping.provider_id, &mapping.channel_id, &mapping.model));
        let matched = observation
            .source
            .get(&mapping.source_provider)
            .and_then(|p| p.models.get(&mapping.source_model));
        let (source, mut issues) =
            matched.map_or_else(|| (Rates::default(), vec!["unknown_model"]), source_rates);
        let rates = merge_rates(&source, &mapping.missing_rates, &mut issues);
        let entry = rates.entry(mapping);
        if entry.is_none() {
            issues.push("missing_rates");
        }
        let prior = find_effective_price_catalog(
            catalogs,
            &mapping.provider_id,
            &mapping.channel_id,
            &mapping.model,
            input.effective_at_ms,
        );
        if prior.is_some_and(|p| {
            p.effective_at_ms == input.effective_at_ms
                && p.catalog_version_id >= input.catalog_version_id
        }) {
            issues.push("shadowed_at_effective_time");
        }
        let before = prior.and_then(|p| {
            find_price_entry(p, &mapping.provider_id, &mapping.channel_id, &mapping.model)
        });
        let change = match (before, &entry) {
            (None, _) => "added",
            (Some(a), Some(b)) if a == b => "unchanged",
            _ => "changed",
        };
        applicable &= issues.is_empty();
        rows.push(json!({"mapping":mapping,"match":"operator_exact_mapping","source_rates":source,"rates":rates,"issues":issues,
            "change":change,"before":before,"before_catalog_version_id":prior.map(|p| &p.catalog_version_id)}));
        if issues.is_empty() {
            if let Some(entry) = entry {
                entries.push(entry);
            }
        }
    }
    // Catalogs are sparse overlays. An omitted tuple keeps its prior catalog; it is not a zero price.
    let channels: BTreeSet<_> = input
        .mappings
        .iter()
        .map(|m| (&m.provider_id, &m.channel_id))
        .collect();
    let mut seen = BTreeSet::new();
    for p in catalogs {
        for e in &p.entries {
            let key = (&e.provider_id, &e.channel_id, &e.model);
            if channels.contains(&(&e.provider_id, &e.channel_id))
                && !proposed_keys.contains(&key)
                && seen.insert(key)
            {
                if let Some(prior) = find_effective_price_catalog(
                    catalogs,
                    &e.provider_id,
                    &e.channel_id,
                    &e.model,
                    input.effective_at_ms,
                ) {
                    rows.push(json!({"provider_id":e.provider_id,"channel_id":e.channel_id,"model":e.model,
                        "change":"removed_from_proposal","effect":"retains_prior_price","before_catalog_version_id":prior.catalog_version_id}));
                }
            }
            if rows.len() > 4096 {
                return Err(invalid_input());
            }
        }
    }
    Ok((json!(rows), entries, applicable))
}

async fn preview(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(c) => c,
        Err(e) => return e,
    };
    let actor = match principal(&request) {
        Ok(a) => a.as_str().to_owned(),
        Err(e) => return e,
    };
    let input = match parse_json::<PreviewInput>(&body) {
        Ok(i) => i,
        Err(e) => return e,
    };
    let now = match runtime_observed_at(&state)
        .and_then(|t| u64::try_from(t).map_err(|_| internal_error()))
    {
        Ok(t) => t,
        Err(e) => return e,
    };
    if let Err(e) = validate_input(&input, now) {
        return e;
    }
    let catalogs = {
        let mut service = match service(&state) {
            Ok(s) => s,
            Err(e) => return e,
        };
        match service.list_billing_catalogs(&context.version) {
            Ok(c) if c.revision() == context.revision => c.into_parts().0,
            Ok(_) => return conflict(),
            Err(e) => return management_error(e),
        }
    };
    if catalogs
        .iter()
        .any(|c| c.catalog_version_id == input.catalog_version_id)
    {
        return conflict();
    }
    let observation = match observe(&state).await {
        Ok(o) => o,
        Err(e) => return e,
    };
    if input.effective_at_ms < observation.observed_at_ms {
        return conflict();
    }
    let catalog_digest = match billing_catalog_fingerprint(&catalogs) {
        Ok(d) => d,
        Err(_) => return internal_error(),
    };
    let (changes, entries, can_apply) = match proposal(&input, &observation, &catalogs) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let mut result = json!({"source":"models.dev","source_digest":observation.digest,"observed_at_ms":observation.observed_at_ms,
        "currency":"USD","source_unit":"usd_per_million_tokens","unit":"microunits_per_million_tokens",
        "config_version_id":context.version.as_str(),"revision":context.revision.as_i64(),"catalog_digest":catalog_digest,
        "catalog_version_id":input.catalog_version_id,"effective_at_ms":input.effective_at_ms,"can_apply":can_apply,"changes":changes,
        "scope":"new_materialization_by_exact_provider_channel_public_model_and_attempt_end_time; historical_ledger_and_frozen_snapshots_unchanged; routing_policy_unchanged"});
    let evidence = match serde_json::to_vec(&result) {
        Ok(v) if v.len() <= MAX_REVIEW_BYTES => v,
        _ => return invalid_input(),
    };
    let content_digest = digest(&evidence);
    result["content_digest"] = json!(content_digest);
    let evidence = match serde_json::to_vec(&result) {
        Ok(v) if v.len() <= MAX_REVIEW_BYTES => v,
        _ => return invalid_input(),
    };
    if can_apply {
        let mut previews = match state.price_previews.lock() {
            Ok(p) => p,
            Err(_) => return internal_error(),
        };
        previews.prune();
        if previews.0.len() >= 8 {
            return error_response(
                StatusCode::TOO_MANY_REQUESTS,
                "management_price_preview_busy",
                "价格预览容量已满，请稍后重试",
            );
        }
        let mut random = [0u8; 24];
        if getrandom::fill(&mut random).is_err() {
            return internal_error();
        }
        let id = URL_SAFE_NO_PAD.encode(random);
        result["preview_id"] = json!(id);
        result["expires_at_ms"] = json!(observation.observed_at_ms + 600_000);
        previews.0.insert(
            id,
            Retained {
                owner: actor,
                version: context.version,
                revision: context.revision,
                deadline: Instant::now() + TTL,
                source_digest: observation.digest,
                catalog_digest,
                content_digest,
                input: BillingCatalogImport {
                    catalog_version_id: input.catalog_version_id,
                    effective_at_ms: input.effective_at_ms,
                    source: BillingCatalogSource::Imported,
                    entries,
                },
                evidence: match String::from_utf8(evidence) {
                    Ok(s) => s,
                    Err(_) => return internal_error(),
                },
            },
        );
    }
    HttpResponse::Ok()
        .insert_header(("ETag", format!("\"rev-{}\"", context.revision.as_i64())))
        .json(result)
}

async fn apply(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match write_context(&request) {
        Ok(c) => c,
        Err(e) => return e,
    };
    let actor = match principal(&request) {
        Ok(a) => a,
        Err(e) => return e,
    };
    let input = match parse_json::<ApplyInput>(&body) {
        Ok(i) => i,
        Err(e) => return e,
    };
    let retained = {
        let mut previews = match state.price_previews.lock() {
            Ok(p) => p,
            Err(_) => return internal_error(),
        };
        previews.prune();
        match previews.0.get(&input.preview_id) {
            Some(p)
                if p.owner == actor.as_str()
                    && p.version == context.version
                    && p.revision == context.revision
                    && p.content_digest == input.content_digest =>
            {
                p.clone()
            }
            _ => return conflict(),
        }
    };
    let observation = match observe(&state).await {
        Ok(o) => o,
        Err(e) => return e,
    };
    if observation.digest != retained.source_digest
        || retained.input.effective_at_ms < observation.observed_at_ms
    {
        return conflict();
    }
    // No lock spans source I/O. Recheck and consume under the same lock as the service write.
    let mut previews = match state.price_previews.lock() {
        Ok(p) => p,
        Err(_) => return internal_error(),
    };
    previews.prune();
    if !previews.0.contains_key(&input.preview_id) {
        return conflict();
    }
    let mut service = match service(&state) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let result = service.import_reviewed_billing_catalog(
        &actor,
        &context.version,
        context.revision,
        retained.input,
        &retained.catalog_digest,
        &retained.evidence,
    );
    match result {
        Ok(receipt) => {
            previews.0.remove(&input.preview_id);
            revisioned_json(StatusCode::CREATED, receipt, |value| {
                billing_catalog_mutation_response(&value)
            })
        }
        Err(e) => management_error(e),
    }
}
async fn review(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(c) => c,
        Err(e) => return e,
    };
    let mut service = match service(&state) {
        Ok(s) => s,
        Err(e) => return e,
    };
    match service.billing_catalog_review(&context.version, &path.into_inner()) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(v) => HttpResponse::Ok().json(v),
            Err(_) => internal_error(),
        },
        Err(e) => management_error(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_decimal_rates_never_round_or_guess_tiers() -> Result<(), Box<dyn std::error::Error>> {
        for (s, n) in [
            ("1.25", 1_250_000),
            ("0", 0),
            ("0.000001", 1),
            ("12.0000000", 12_000_000),
            ("1.25e1", 12_500_000),
        ] {
            let raw: Box<RawValue> = serde_json::from_str(s)?;
            assert_eq!(rate(&raw), Ok(n));
        }
        for s in ["-1", "0.0000001", "9007199254.740992", "\"1.2\"", "1e40"] {
            let raw: Box<RawValue> = serde_json::from_str(s)?;
            assert!(rate(&raw).is_err(), "{s}");
        }
        let model: SourceModel =
            serde_json::from_value(json!({"cost":{"input":0,"output":1.25,"tiers":[]}}))?;
        let (_, issues) = source_rates(&model);
        assert_eq!(issues, vec!["unsupported_tier_or_rate"]);
        Ok(())
    }
}
