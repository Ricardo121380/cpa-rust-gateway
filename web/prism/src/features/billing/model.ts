// 计费与价格目录 — pure model over the six billing operations.
//
// The shape here follows three facts that are easy to get wrong, and each one
// is surfaced on screen rather than designed around:
//
//   1. CATALOGS ARE GLOBAL. `listBillingCatalogs` requires X-Config-Version in
//      its header, but the service reads `list_billing_catalogs_bounded()` with
//      no version at all — the header only carries the revision back for
//      If-Match. Importing a catalog while a draft is selected is NOT scoped to
//      that draft: every config version sees it immediately.
//      Only the POLICY BINDING is per config version.
//
//   2. IMPORT IS WHOLE-CATALOG, INSERT-ONLY. There is no edit and no delete.
//      `importBillingCatalog` creates a NEW catalog_version_id carrying the
//      complete entry set (1..512 entries — an empty catalog is illegal), and
//      `rollbackBillingCatalog` also creates a new version that copies an old
//      one. Nothing is ever mutated in place or removed.
//
//   3. A FUTURE-DATED CATALOG CANNOT BE BOUND. set_routing_price_policy checks
//      `catalog.effective_at_ms > now` and fails with
//      RoutingPriceCatalogNotEffective. So the picker must not offer one.
//
// Rates are microunits per million tokens. The contract names NO CURRENCY
// anywhere, so nothing here formats money.

export type Tone = "good" | "warn" | "serious" | "critical" | "tint" | "muted";

/** Contract bounds, mirrored so the UI can reject before the round trip. */
export const MAX_ENTRIES = 512;
export const MIN_ENTRIES = 1;
export const MAX_CATALOGS = 256;
export const MAX_ID_LENGTH = 128;
export const MAX_MODEL_LENGTH = 512;
export const MAX_BILLING_INTEGER = Number.MAX_SAFE_INTEGER;

export function validBillingText(value: string, maximum: number): boolean {
  return value.length > 0 && value.trim() === value && [...value].length <= maximum
    && new TextEncoder().encode(value).length <= maximum && !/[\p{Cc}]/u.test(value);
}

export function validBillingTime(value: number): boolean {
  return Number.isSafeInteger(value) && value >= 0 && Number.isFinite(new Date(value).getTime());
}

/** `test` appears on read but is not writable — the UI offers only the two the
 *  contract accepts on import. */
export const WRITABLE_SOURCES = ["operator", "imported"] as const;
export type WritableSource = (typeof WRITABLE_SOURCES)[number];

export const RATE_FIELDS = [
  "input_microunits_per_million",
  "output_microunits_per_million",
  "reasoning_microunits_per_million",
  "cache_read_microunits_per_million",
  "cache_creation_microunits_per_million",
  "cached_microunits_per_million",
] as const;

export type RateField = (typeof RATE_FIELDS)[number];

const RATE_LABEL: Readonly<Record<RateField, string>> = {
  input_microunits_per_million: "输入",
  output_microunits_per_million: "输出",
  reasoning_microunits_per_million: "推理",
  cache_read_microunits_per_million: "缓存读",
  cache_creation_microunits_per_million: "缓存写",
  cached_microunits_per_million: "已缓存",
};

export function rateLabel(field: RateField): string {
  return RATE_LABEL[field];
}

export type CatalogEntry = Readonly<{
  provider_id: string;
  channel_id: string;
  model: string;
}> &
  Readonly<Record<RateField, number>>;

export type Catalog = Readonly<{
  catalog_version_id: string;
  effective_at_ms: number;
  source: string;
  created_at_ms: number;
  entries: readonly CatalogEntry[];
}>;

export type ImportReceipt = Readonly<{
  catalog_version_id: string;
  effective_at_ms: number;
  source: string;
  entry_count: number;
  operation: "imported" | "rolled_back";
  rolled_back_from: string | null;
}>;

export type PricePolicy = Readonly<{
  catalog_version_id: string;
  comparison: string;
}>;

/** The contract's comparison enum currently has exactly one member. */
export const COMPARISON = "rate_dominance_v1" as const;

// ---------------------------------------------------------------------------
// effectiveness
// ---------------------------------------------------------------------------

/** A catalog dated in the future exists and is listed, but binding it to the
 *  routing price policy fails closed. The picker uses this to disable it and
 *  say why, rather than letting the operator discover it from a 4xx. */
export function isEffective(catalog: Catalog, nowMs: number): boolean {
  return catalog.effective_at_ms <= nowMs;
}

/** Catalogs newest-effective first; ties broken by creation time then id so the
 *  list does not reshuffle between reads. */
export function sortCatalogs(catalogs: readonly Catalog[]): readonly Catalog[] {
  return [...catalogs].sort(
    (a, b) =>
      b.effective_at_ms - a.effective_at_ms ||
      b.created_at_ms - a.created_at_ms ||
      a.catalog_version_id.localeCompare(b.catalog_version_id),
  );
}

// ---------------------------------------------------------------------------
// entry parsing
// ---------------------------------------------------------------------------
//
// A catalog holds up to 512 entries, which are produced from a pricing sheet,
// not typed one field at a time. So the import takes JSON and validates it
// against the contract bounds here — naming the exact entry and field that
// fails, because "400 invalid_management_request" on a 512-row paste is
// useless to the person holding the sheet.

export type ParsedEntries =
  | Readonly<{ ok: true; entries: readonly CatalogEntry[] }>
  | Readonly<{ ok: false; reason: string; entryIndex?: number }>;

function badEntry(index: number, message: string): ParsedEntries {
  return { ok: false, reason: `第 ${index + 1} 条:${message}`, entryIndex: index };
}

export function parseCatalogEntries(raw: string): ParsedEntries {
  const trimmed = raw.trim();
  if (trimmed.length === 0) {
    return { ok: false, reason: "条目不能为空 —— 契约要求至少 1 条。" };
  }
  let decoded: unknown;
  try {
    decoded = JSON.parse(trimmed);
  } catch (cause) {
    return { ok: false, reason: `不是合法 JSON:${(cause as Error).message}` };
  }
  if (!Array.isArray(decoded)) {
    return { ok: false, reason: "顶层必须是数组:[{ provider_id, channel_id, model, …费率 }]。" };
  }
  if (decoded.length < MIN_ENTRIES) {
    return { ok: false, reason: `契约要求至少 ${MIN_ENTRIES} 条,收到 0 条。` };
  }
  if (decoded.length > MAX_ENTRIES) {
    return { ok: false, reason: `契约上限 ${MAX_ENTRIES} 条,收到 ${decoded.length} 条。` };
  }

  const entries: CatalogEntry[] = [];
  const seen = new Set<string>();
  for (const [index, item] of decoded.entries()) {
    if (typeof item !== "object" || item === null || Array.isArray(item)) {
      return badEntry(index, "必须是一个对象。");
    }
    const record = item as Record<string, unknown>;
    const allowed = new Set<string>(["provider_id", "channel_id", "model", ...RATE_FIELDS]);
    const unknown = Object.keys(record).find((field) => !allowed.has(field));
    if (unknown !== undefined) return badEntry(index, `不支持字段 ${unknown}；切换编辑模式前请先移除。`);
    const text: Record<string, string> = {};
    for (const [field, max] of [
      ["provider_id", MAX_ID_LENGTH],
      ["channel_id", MAX_ID_LENGTH],
      ["model", MAX_MODEL_LENGTH],
    ] as const) {
      const value = record[field];
      if (typeof value !== "string" || !validBillingText(value, max)) {
        return badEntry(index, `${field} 必须是无首尾空白、控制字符且不超过 ${max} 字节的非空字符串。`);
      }
      text[field] = value;
    }
    const rates: Record<string, number> = {};
    for (const field of RATE_FIELDS) {
      const value = record[field];
      if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
        return badEntry(index, `${field} 必须是 0–${MAX_BILLING_INTEGER} 的安全整数(单位:microunits / 百万 token)。`);
      }
      rates[field] = value;
    }
    // Two rows for the same target would make pricing ambiguous; the backend
    // has its own view of that, but a paste with an obvious duplicate is worth
    // catching before the round trip.
    const key = `${text["provider_id"]}\u0000${text["channel_id"]}\u0000${text["model"]}`;
    if (seen.has(key)) {
      return badEntry(index, "与前面某条的 provider/channel/model 完全重复。");
    }
    seen.add(key);
    entries.push({ ...text, ...rates } as unknown as CatalogEntry);
  }
  return { ok: true, entries };
}

/** Round-trips a catalog's entries back into the import box, so "copy an
 *  existing catalog, change two rates, import as a new version" does not mean
 *  retyping it. */
export function formatCatalogEntries(entries: readonly CatalogEntry[]): string {
  return JSON.stringify(entries, null, 2);
}

// ---------------------------------------------------------------------------
// formatting
// ---------------------------------------------------------------------------

/** Rates are microunits per million tokens. No currency is applied, because
 *  the contract declares none — not here and not in the ledger. */
export function formatRate(value: number): string {
  return value.toLocaleString("en-US");
}

export function formatCount(value: number): string {
  return value.toLocaleString("en-US");
}

/** UTC, minute precision. */
export function formatTime(ms: number): string {
  if (!validBillingTime(ms)) return "无效时间";
  return `${new Date(ms).toISOString().slice(0, 16).replace("T", " ")}Z`;
}

const SOURCE_LABEL: Readonly<Record<string, string>> = {
  operator: "运维录入",
  imported: "外部导入",
  test: "测试",
};

export function sourceLabel(source: string): string {
  return SOURCE_LABEL[source] ?? source;
}

/**
 * `404 management_resource_not_found` on getRoutingPricePolicy means NOT
 * CONFIGURED, which is a legitimate state (every candidate's `price_evidence`
 * then reads `disabled`) — not an error to paint red.
 *
 * The code is matched, not just the status: `404 management_access_denied` is
 * the gateway's fail-closed answer to a request from a disallowed browser
 * origin, and the client maps THAT to a session reset. Treating any 404 as
 * "no policy" would swallow it and show a calm empty state over a dead session.
 */
export function isPolicyUnset(error: unknown): boolean {
  if (typeof error !== "object" || error === null) {
    return false;
  }
  const candidate = error as { status?: unknown; code?: unknown };
  return candidate.status === 404 && candidate.code === "management_resource_not_found";
}

export type PriceChange = Readonly<{before?:CatalogEntry;after?:CatalogEntry;kind:"added"|"changed"|"removed"}>;
/** Same exact model on another provider/channel is a separate price, never an alias. */
export function compareCatalogEntries(before:readonly CatalogEntry[],after:readonly CatalogEntry[]):PriceChange[] {
  const key=(row:CatalogEntry)=>JSON.stringify([row.provider_id,row.channel_id,row.model]);
  const previous=new Map(before.map(row=>[key(row),row]));
  const changes:PriceChange[]=[];
  for(const row of after){
    const old=previous.get(key(row));previous.delete(key(row));
    if(!old)changes.push({kind:"added",after:row});
    else if(RATE_FIELDS.some(field=>old[field]!==row[field]))changes.push({kind:"changed",before:old,after:row});
  }
  for(const row of previous.values())changes.push({kind:"removed",before:row});
  return changes;
}
