//! Complete identity search across configured and native accounts, never over decrypted secrets.
use super::{
    ManagementResourceHttpState, error_response, internal_error, invalid_input,
    query_has_duplicate_keys, read_context, read_operations, resource_inventory, service,
};
use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use gateway_store::control_plane::ResourceInventoryQuery;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Params {
    q: Option<String>,
    category: Option<String>,
    status: Option<String>,
    upstream_id: Option<String>,
    sort: Option<String>,
    limit: Option<u16>,
    cursor: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: String,
    revision: i64,
    audit: i64,
    native: Option<i64>,
    filter: Params,
    offset: usize,
}
fn conflict() -> HttpResponse {
    error_response(
        StatusCode::CONFLICT,
        "management_account_inventory_conflict",
        "账号目录已改变，请重新读取",
    )
}
fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}
fn identity_name(v: &Value) -> String {
    ["email", "phone", "username"]
        .iter()
        .map(|k| text(v, k))
        .find(|v| !v.is_empty())
        .unwrap_or("")
        .to_owned()
}

pub(super) async fn list(
    request: HttpRequest,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let context = match read_context(&request) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let (params, cursor) = match parse(&request, context.version.as_str()) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let limit = params.limit.unwrap_or(50);
    let (reader, project) = match service(&state) {
        Ok(mut s) => (
            s.repository_mut().resource_inventory_reader(),
            s.account_identity_projector(),
        ),
        Err(r) => return r,
    };
    let Some(reader) = reader else {
        return internal_error();
    };
    let native = state.native_accounts.clone();
    let result=read_operations(&state,move ||{
  let mut items=Vec::new();let mut after=None;let mut stamp=cursor.as_ref().map(|c|(c.revision,c.audit));
  loop {
   let page=reader.credentials_with_identity(ResourceInventoryQuery{version:&context.version,upstream_id:params.upstream_id.as_deref(),search:"",limit:100,expected_snapshot:stamp,after:after.as_deref()},project.as_ref());
   let page=match page{Ok(p)=>p,Err(gateway_store::StoreError::ConfigVersionRevisionConflict)=>return Ok(Err("conflict")),Err(_)=>return Ok(Err("unavailable"))};stamp=Some((page.version.revision,page.audit_sequence));
   for row in page.items {
    let enabled=!matches!(row.status,gateway_store::control_plane::CredentialStatus::Disabled);
    let status=if !enabled{"disabled"}else if matches!(row.status,gateway_store::control_plane::CredentialStatus::Unauthorized){"reauth_required"}else{"enabled"};
    let value=resource_inventory::credential_value(row);
    let kind=text(&value["credential"],"kind");
    // Actions describe actual existing management capabilities. Unsupported OAuth entry points
    // are not advertised as available simply because a credential contains a token.
    let mut operations=vec!["details","update_credential","enable","disable","remove","models"];
    if kind=="oauth_json"&&text(&value,"category")=="codex"{operations.push("reauthorize")}
    items.push(json!({"id":value["credential"]["id"],"native":false,"identity":value["identity"],"name":identity_name(&value["identity"]),"category":value["category"],"provider":value["provider"],"status":status,"operations":operations,"managed":value,"native_account":null}));
   }
   after=page.next_after;if items.len()>10000||(items.len()==10000&&after.is_some()){return Ok(Err("capacity"))}
 if after.is_none(){break}
  }
  let mut native_stamp=None;
  if let Some(native)=native.as_ref().filter(|_|params.upstream_id.is_none()) {
   let mut after=String::new();let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0,|v|i64::try_from(v.as_millis()).unwrap_or(i64::MAX));
   loop {
    let page=match native.store.managed_account_page(100,&after,"",native_stamp.or_else(||cursor.as_ref().and_then(|c|c.native))){Ok(p)=>p,Err(provider_grok::GrokAccountPoolError::ExistingAccountConflict)=>return Ok(Err("conflict")),Err(_)=>return Ok(Err("unavailable"))};native_stamp=Some(page.stamp);
    for row in page.items {
     after.clone_from(&row.id);let Ok(identity)=native.store.observed_identity(&row.id,now) else{return Ok(Err("unavailable"))};
     let (channel,provider)=match row.provider{provider_grok::GrokAccountProvider::Build=>("grok_build","Grok Build"),provider_grok::GrokAccountProvider::Console=>("grok_console","Grok Console"),provider_grok::GrokAccountProvider::Web=>("grok_web","Grok Web")};
     let auth=match row.auth_status{provider_grok::GrokAccountAuthStatus::Active=>"active",provider_grok::GrokAccountAuthStatus::Disabled=>"disabled",provider_grok::GrokAccountAuthStatus::ReauthRequired=>"reauth_required"};
     let status=if !row.enabled||auth=="disabled"{"disabled"}else if auth=="reauth_required"{"reauth_required"}else{"enabled"};
     let account=json!({"id":row.id,"provider":channel,"auth_status":auth,"enabled":row.enabled,"revision":row.revision,"import_batch_id":row.import_batch_id,"identity":identity});
     let mut operations=vec!["details","update_credential","enable","disable","remove","models"];if channel=="grok_build"{operations.retain(|v|*v!="update_credential");operations.push("reauthorize")}
     items.push(json!({"id":account["id"],"native":true,"identity":account["identity"],"name":identity_name(&account["identity"]),"category":"grok","provider":provider,"status":status,"operations":operations,"managed":null,"native_account":account}));
    }
    if items.len()>10000{return Ok(Err("capacity"))}
 if !page.has_more{break}
   }
   if native.store.managed_account_page(1,"","",native_stamp).is_err(){return Ok(Err("conflict"))}
  }
  if reader.credentials(ResourceInventoryQuery{version:&context.version,upstream_id:None,search:"",limit:1,expected_snapshot:stamp,after:None}).is_err(){return Ok(Err("conflict"))}
  if cursor.as_ref().is_some_and(|c|c.native!=native_stamp){return Ok(Err("conflict"))}
  let query=params.q.as_deref().unwrap_or("").trim().to_lowercase();
  items.retain(|r| params.category.as_deref().is_none_or(|c|text(r,"category")==c)&&params.status.as_deref().is_none_or(|s|text(r,"status")==s)&&[text(r,"provider"),text(r,"category"),text(&r["identity"],"email"),text(&r["identity"],"phone"),text(&r["identity"],"username")].join(" ").to_lowercase().contains(&query));
  items.sort_by(|a,b|{
   let key=|v:&Value| if params.sort.as_deref()==Some("provider"){format!("{} {}",text(v,"provider"),text(v,"name")).to_lowercase()}else{text(v,"name").to_lowercase()};
   let cmp=key(a).cmp(&key(b)).then_with(||text(a,"id").cmp(text(b,"id"))).then_with(||a["native"].as_bool().cmp(&b["native"].as_bool()));
   if params.sort.as_deref()==Some("name_desc"){cmp.reverse()}else{cmp}
  });
  let mut counts=std::collections::BTreeMap::<String,usize>::new();for item in &items{*counts.entry(text(item,"category").to_owned()).or_default()+=1;}
  let total=items.len();let offset=cursor.as_ref().map_or(0,|c|c.offset);if offset>total{return Ok(Err("conflict"))}
  let end=(offset+usize::from(limit)).min(total);let (revision,audit)=stamp.unwrap_or((0,0));
  let next=if end<total{serde_json::to_vec(&Cursor{version:context.version.to_string(),revision,audit,native:native_stamp,filter:params,offset:end}).ok().map(|b|URL_SAFE_NO_PAD.encode(b))}else{None};
  Ok(Ok(json!({"config_version":context.version.to_string(),"revision":format!("rev-{revision}"),"total":total,"category_totals":counts,"items":items[offset..end],"next_cursor":next})))
 }).await;
    match result {
        Ok(Ok(value)) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(value),
        Ok(Err("capacity")) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "management_account_inventory_capacity",
            "目录超过本次完整查询范围，请按提供商查询",
        ),
        Ok(Err("unavailable")) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "management_account_inventory_unavailable",
            "暂时无法读取完整账号目录，请重试",
        ),
        Ok(Err(_)) => conflict(),
        Err(_) => internal_error(),
    }
}

fn parse(request: &HttpRequest, version: &str) -> Result<(Params, Option<Cursor>), HttpResponse> {
    if request.query_string().len() > 8192 || query_has_duplicate_keys(request.query_string()) {
        return Err(invalid_input());
    }
    let mut params = match web::Query::<Params>::from_query(request.query_string()) {
        Ok(p) => p.into_inner(),
        Err(_) => return Err(invalid_input()),
    };
    let cursor = match params.cursor.take() {
        None => None,
        Some(c) if c.len() <= 7000 => match URL_SAFE_NO_PAD
            .decode(c)
            .ok()
            .and_then(|b| serde_json::from_slice::<Cursor>(&b).ok())
        {
            Some(c) => Some(c),
            None => return Err(invalid_input()),
        },
        Some(_) => return Err(invalid_input()),
    };
    let limit = params.limit.unwrap_or(50);
    if !(1..=100).contains(&limit)
        || params.q.as_ref().is_some_and(|s| s.len() > 256)
        || params
            .upstream_id
            .as_ref()
            .is_some_and(|s| s.is_empty() || s.len() > 128)
        || params.category.as_ref().is_some_and(|s| {
            !["api", "codex", "claude", "kimi", "kiro", "grok"].contains(&s.as_str())
        })
        || params
            .status
            .as_ref()
            .is_some_and(|s| !["enabled", "disabled", "reauth_required"].contains(&s.as_str()))
        || params
            .sort
            .as_ref()
            .is_some_and(|s| !["name", "name_desc", "provider"].contains(&s.as_str()))
    {
        return Err(invalid_input());
    }
    if cursor
        .as_ref()
        .is_some_and(|c| c.version != version || c.filter != params || c.offset > 10000)
    {
        return Err(conflict());
    }

    Ok((params, cursor))
}
