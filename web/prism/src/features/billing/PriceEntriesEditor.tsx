import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { resourceName } from "../../utils/resourceNames";
import { useManagedInventory } from "../accounts/inventory";
import { MAX_ENTRIES, RATE_FIELDS, rateLabel, type RateField } from "./model";

type Draft = {provider_id:string;channel_id:string;model:string}&Record<RateField,string>;
type Quote = {provider:string;model:string;tiered:boolean}&Record<RateField,number|null>;
type Quotes = {source:string;currency:"USD";items:Quote[]};
const blank = ():Draft => ({provider_id:"",channel_id:"",model:"",...Object.fromEntries(RATE_FIELDS.map(f=>[f,""]))} as Draft);
export function priceDrafts(raw:string):Draft[] {
  try { const entries:unknown=JSON.parse(raw);if(!Array.isArray(entries))return [blank()];
    return entries.slice(0,MAX_ENTRIES).map(row=>({...blank(),...Object.fromEntries(Object.entries(row).map(([k,v])=>[k,v==null?"":String(v)]))}));
  } catch {return [blank()];}
}
export function serializePriceDrafts(rows:readonly Draft[]):string {
  return JSON.stringify(rows.map(row=>({...row,...Object.fromEntries(RATE_FIELDS.map(f=>[f,row[f].trim()===""?null:Number(row[f])]))})));
}
export function applyPriceQuote(row:Draft,quote:Quote):Draft {
  if(quote.tiered||quote.model!==row.model)return row;
  return {...row,...Object.fromEntries(RATE_FIELDS.map(f=>[f,quote[f]==null?row[f]:String(quote[f])]))};
}
export function PriceEntriesEditor({initial,disabled}:{initial?:string;disabled:boolean}) {
  const [rows,setRows]=useState(()=>priceDrafts(initial??""));
  const [advanced,setAdvanced]=useState(false),[raw,setRaw]=useState(initial??"");
  const [usd,setUsd]=useState(false);
  const endpoints=useManagedInventory("endpoints");
  const connections=endpoints.data?.pages.flatMap(p=>p.items)??[];
  const refresh=useMutation({mutationFn:()=>call<Quotes>("refreshPriceSource",{body:{models:[...new Set(rows.map(r=>r.model).filter(Boolean))]}})});
  const update=(index:number,patch:Partial<Draft>)=>setRows(current=>current.map((r,i)=>i===index?{...r,...patch}:r));
  const busy=disabled||refresh.isPending;
  return <div className="price-editor">
    <div className="page-actions"><button type="button" className="secondary" disabled={busy} onClick={()=>{if(!advanced)setRaw(serializePriceDrafts(rows));else setRows(priceDrafts(raw));setAdvanced(!advanced);}}>{advanced?"表格编辑":"高级 JSON"}</button>
    {!advanced?<button type="button" className="secondary" disabled={busy||!rows.some(r=>r.model)} onClick={()=>refresh.mutate()}>{refresh.isPending?"读取价格中…":"从 models.dev 读取价格"}</button>:null}</div>
    {advanced?<label>完整价格条目<textarea name="entries" value={raw} onChange={e=>setRaw(e.target.value)} disabled={busy} required rows={12}/></label>:<>
      <input type="hidden" name="entries" value={serializePriceDrafts(rows)}/>
      <p className="stat-sub">单位：microunits / 百万 token。六类费率分别填写；空白表示缺失，不能提交。</p>
      {refresh.isError?<p role="alert">{asAppError(refresh.error).message}</p>:null}
      {refresh.data?<><p role="status">已读取 models.dev，匹配 {refresh.data.items.length} 份来源报价。未写入价目表。</p><label className="check-row"><input type="checkbox" checked={usd} onChange={e=>setUsd(e.target.checked)} disabled={busy}/>本次目录采用 USD 计价；我已核对现有费用单位</label></>:null}
      {rows.map((row,index)=><fieldset className="price-entry" key={index} disabled={busy}><legend>价格 {index+1} {row.model}</legend>
        <div className="price-entry-fields"><label>接口连接<select required value={row.channel_id} onChange={e=>{const endpoint=connections.find(v=>v.id===e.target.value);update(index,{channel_id:e.target.value,provider_id:endpoint?.upstream_id??""});}}><option value="">选择接口</option>{row.channel_id&&!connections.some(c=>c.id===row.channel_id)?<option value={row.channel_id}>{resourceName(row.channel_id,"endpoint")}</option>:null}{connections.map(c=><option key={c.id} value={c.id}>{resourceName(c.upstream_id,"upstream")} · {c.api_format}</option>)}</select></label>
        <label>原始模型 ID<input required maxLength={512} value={row.model} onChange={e=>update(index,{model:e.target.value})}/></label>
        {RATE_FIELDS.map(field=><label key={field}>{rateLabel(field)}<input type="number" min="0" max={Number.MAX_SAFE_INTEGER} step="1" required value={row[field]} onChange={e=>update(index,{[field]:e.target.value})}/></label>)}</div>
        {refresh.data?<label>选择来源报价<select value="" disabled={!usd} onChange={e=>{const quote=refresh.data?.items[Number(e.target.value)];if(quote)setRows(current=>current.map((r,i)=>i===index?applyPriceQuote(r,quote):r));}}><option value="">仅填入来源明确给出的费率</option>{refresh.data.items.map((quote,i)=>quote.model===row.model?<option value={i} key={i} disabled={quote.tiered}>{quote.provider}{quote.tiered?" · 分段价格，需手动核对":""}</option>:null)}</select></label>:null}
        <button type="button" className="secondary" disabled={rows.length===1} onClick={()=>setRows(current=>current.filter((_,i)=>i!==index))}>移除此项</button>
      </fieldset>)}
      <div className="page-actions"><button type="button" className="secondary" disabled={busy||rows.length>=MAX_ENTRIES} onClick={()=>setRows(current=>[...current,blank()])}>添加价格</button>{endpoints.hasNextPage?<button type="button" className="secondary" disabled={endpoints.isFetching} onClick={()=>void endpoints.fetchNextPage()}>加载更多接口</button>:null}</div>
      {endpoints.isError?<p role="alert">{asAppError(endpoints.error).message}</p>:null}
    </>}
  </div>;
}
