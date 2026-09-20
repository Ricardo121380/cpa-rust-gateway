import { useEffect, useRef, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { resourceName } from "../../utils/resourceNames";
import { useManagedInventory } from "../accounts/inventory";
import { parseCatalogEntries, MAX_ENTRIES, RATE_FIELDS, rateLabel, type RateField } from "./model";

type Draft = {provider_id:string;channel_id:string;model:string}&Record<RateField,string>;
type Quote = {provider:string;model:string;tiered:boolean}&Record<RateField,number|null>;
type Quotes = {source:string;currency:"USD";items:Quote[]};
const blank = ():Draft => ({provider_id:"",channel_id:"",model:"",...Object.fromEntries(RATE_FIELDS.map(f=>[f,""]))} as Draft);
type DraftParse={ok:true;rows:Draft[]}|{ok:false;reason:string};
const draftFields=new Set<string>(["provider_id","channel_id","model",...RATE_FIELDS]);
export function parsePriceDrafts(raw:string):DraftParse {
  if(!raw.trim())return {ok:true,rows:[blank()]};
  let value:unknown;
  try{value=JSON.parse(raw);}catch{return {ok:false,reason:"价格条目不是有效 JSON；原文已保留。"};}
  if(!Array.isArray(value)||value.length<1||value.length>MAX_ENTRIES)return {ok:false,reason:`价格条目须为 1–${MAX_ENTRIES} 项数组；原文已保留。`};
  const rows:Draft[]=[];
  for(const [index,item] of value.entries()){
    if(!item||typeof item!=="object"||Array.isArray(item))return {ok:false,reason:`第 ${index+1} 条须为对象；原文已保留。`};
    const record=item as Record<string,unknown>;
    const unknown=Object.keys(record).find(key=>!draftFields.has(key));
    if(unknown)return {ok:false,reason:`第 ${index+1} 条含未支持字段 ${unknown}；原文已保留。`};
    const row=blank();
    for(const field of ["provider_id","channel_id","model"] as const){
      const input=record[field];
      if(input!==undefined&&typeof input!=="string")return {ok:false,reason:`第 ${index+1} 条 ${field} 须为字符串；原文已保留。`};
      row[field]=input??"";
    }
    for(const field of RATE_FIELDS){
      const input=record[field];
      if(input!==undefined&&input!==null&&!(typeof input==="number"&&Number.isSafeInteger(input)&&input>=0))return {ok:false,reason:`第 ${index+1} 条 ${field} 须为非负安全整数或 null；原文已保留。`};
      row[field]=input==null?"":String(input);
    }
    rows.push(row);
  }
  return {ok:true,rows};
}
export function priceDrafts(raw:string):Draft[] {
  const parsed=parsePriceDrafts(raw);
  if(!parsed.ok)throw new Error(parsed.reason);
  return parsed.rows;
}
export function serializePriceDrafts(rows:readonly Draft[]):string {
  return JSON.stringify(rows.map(row=>({...row,...Object.fromEntries(RATE_FIELDS.map(field=>{
    const raw=row[field].trim();
    const number=Number(raw);
    return [field,raw===""?null:/^(0|[1-9]\d*)$/u.test(raw)&&Number.isSafeInteger(number)?number:row[field]];
  }))})));
}
export function applyPriceQuote(row:Draft,quote:Quote):Draft {
  if(quote.tiered||quote.model!==row.model)return row;
  return {...row,...Object.fromEntries(RATE_FIELDS.map(f=>[f,quote[f]==null?row[f]:String(quote[f])]))};
}
export function PriceEntriesEditor({initial,disabled,onDirty,onBusyChange}:{initial?:string;disabled:boolean;onDirty?:()=>void;onBusyChange?:(busy:boolean)=>void}) {
  const [initialParsed]=useState(()=>parsePriceDrafts(initial??""));
  const [rows,setRows]=useState(()=>initialParsed.ok?initialParsed.rows:[blank()]);
  const [advanced,setAdvanced]=useState(!initialParsed.ok),[raw,setRaw]=useState(initial??"");
  const [rowIds,setRowIds]=useState(()=>rows.map(()=>crypto.randomUUID()));
  const [page,setPage]=useState(0);
  const root=useRef<HTMLDivElement>(null);
  const focusRow=useRef<number | undefined>(undefined);
  const pageCount=Math.max(1,Math.ceil(rows.length/50));
  const currentPage=Math.min(page,pageCount-1);
  useEffect(()=>{
    const form=root.current?.closest("form");
    if(!form||advanced)return;
    const validate=(event:Event)=>{
      const result=parseCatalogEntries(serializePriceDrafts(rows));
      if(result.ok)return;
      event.preventDefault();event.stopImmediatePropagation();
      setModeError(result.reason);
      if(result.entryIndex!==undefined){focusRow.current=result.entryIndex;setPage(Math.floor(result.entryIndex/50));}
    };
    form.addEventListener("submit",validate,true);
    return ()=>form.removeEventListener("submit",validate,true);
  },[rows,advanced]);
  useEffect(()=>{
    if(focusRow.current===undefined)return;
    const fieldset=root.current?.querySelector<HTMLElement>(`[data-price-index="${focusRow.current}"]`);
    if(!fieldset)return;
    (fieldset.querySelector<HTMLElement>("input:invalid,select:invalid")??fieldset).focus();
    focusRow.current=undefined;
  });
  const [modeError,setModeError]=useState<string>();
  const [usd,setUsd]=useState(false);
  const endpoints=useManagedInventory("endpoints");
  const connections=endpoints.data?.pages.flatMap(p=>p.items)??[];
  const refresh=useMutation({mutationFn:()=>call<Quotes>("refreshPriceSource",{body:{models:[...new Set(rows.map(r=>r.model).filter(Boolean))]}})});
  useEffect(()=>{onBusyChange?.(refresh.isPending);},[onBusyChange,refresh.isPending]);
  const update=(index:number,patch:Partial<Draft>)=>{onDirty?.();refresh.reset();setRows(current=>current.map((r,i)=>i===index?{...r,...patch}:r));};
  const busy=disabled||refresh.isPending;
  return <div className="price-editor" ref={root}>
    <div className="page-actions"><button type="button" className="secondary" disabled={busy} onClick={()=>{if(!advanced){setRaw(serializePriceDrafts(rows));setModeError(undefined);refresh.reset();setAdvanced(true);return;}const parsed=parsePriceDrafts(raw);if(!parsed.ok){setModeError(parsed.reason);return;}setRows(parsed.rows);setRowIds(parsed.rows.map(()=>crypto.randomUUID()));setPage(0);setModeError(undefined);refresh.reset();setAdvanced(false);}}>{advanced?"表格编辑":"高级 JSON"}</button>
    {!advanced?<button type="button" className="secondary" disabled={busy||!rows.some(r=>r.model)} onClick={()=>refresh.mutate()}>{refresh.isPending?"读取价格中…":"从 models.dev 读取价格"}</button>:null}</div>
    {modeError?<p role="alert">{modeError}</p>:null}
    {advanced?<label>完整价格条目<textarea name="entries" value={raw} onChange={e=>{onDirty?.();setModeError(undefined);refresh.reset();setRaw(e.target.value);}} disabled={busy} required rows={12}/></label>:<>
      <input type="hidden" name="entries" value={serializePriceDrafts(rows)}/>
      <p className="stat-sub">单位：microunits / 百万 token。六类费率分别填写；空白表示缺失，不能提交。</p>
      {refresh.isError?<p role="alert">{asAppError(refresh.error).message}</p>:null}
      {refresh.data?<><p role="status">已读取 models.dev，匹配 {refresh.data.items.length} 份来源报价。未写入价目表。</p><label className="check-row"><input type="checkbox" checked={usd} onChange={e=>{onDirty?.();setUsd(e.target.checked);}} disabled={busy}/>本次目录采用 USD 计价；我已核对现有费用单位</label></>:null}
      {rows.slice(currentPage*50,currentPage*50+50).map((row,offset)=>{const index=currentPage*50+offset;return <fieldset className="price-entry" data-price-index={index} tabIndex={-1} key={rowIds[index]} disabled={busy}><legend>价格 {index+1} {row.model}</legend>
        <div className="price-entry-fields"><label>接口连接<select required value={row.channel_id} onChange={e=>{const endpoint=connections.find(v=>v.id===e.target.value);update(index,{channel_id:e.target.value,provider_id:endpoint?.upstream_id??""});}}><option value="">选择接口</option>{row.channel_id&&!connections.some(c=>c.id===row.channel_id)?<option value={row.channel_id}>{resourceName(row.channel_id,"endpoint")}</option>:null}{connections.map(c=><option key={c.id} value={c.id}>{resourceName(c.upstream_id,"upstream")} · {c.api_format}</option>)}</select></label>
        <label>计费模型名（已解析的公开模型）<input required maxLength={512} value={row.model} onChange={e=>update(index,{model:e.target.value})}/></label>
        {RATE_FIELDS.map(field=><label key={field}>{rateLabel(field)}<input type="number" min="0" max={Number.MAX_SAFE_INTEGER} step="1" required value={row[field]} onChange={e=>update(index,{[field]:e.target.value})}/></label>)}</div>
        {refresh.data?<label>选择来源报价<select value="" disabled={!usd} onChange={e=>{const quote=refresh.data?.items[Number(e.target.value)];if(quote){onDirty?.();setRows(current=>current.map((r,i)=>i===index?applyPriceQuote(r,quote):r));}}}><option value="">仅填入来源明确给出的费率</option>{refresh.data.items.map((quote,i)=>quote.model===row.model?<option value={i} key={i} disabled={quote.tiered}>{quote.provider}{quote.tiered?" · 分段价格，需手动核对":""}</option>:null)}</select></label>:null}
        <button type="button" className="secondary" disabled={rows.length===1} onClick={()=>{onDirty?.();setRows(current=>current.filter((_,i)=>i!==index));setRowIds(current=>current.filter((_,i)=>i!==index));}}>移除此项</button>
      </fieldset>;})}
      <nav className="bill-actions" aria-label="价格编辑分页"><button type="button" className="secondary" disabled={busy||currentPage===0} onClick={()=>setPage(currentPage-1)}>上一页</button><label>编辑页码<select value={currentPage} disabled={busy} onChange={event=>setPage(Number(event.target.value))}>{Array.from({length:pageCount},(_,index)=><option key={index} value={index}>第 {index+1} / {pageCount} 页</option>)}</select></label><button type="button" className="secondary" disabled={busy||currentPage+1===pageCount} onClick={()=>setPage(currentPage+1)}>下一页</button><span role="status">{currentPage*50+1}–{Math.min(currentPage*50+50,rows.length)} / {rows.length} 条</span></nav>
      <div className="page-actions"><button type="button" className="secondary" disabled={busy||rows.length>=MAX_ENTRIES} onClick={()=>{onDirty?.();setRows(current=>[...current,blank()]);setRowIds(current=>[...current,crypto.randomUUID()]);setPage(Math.floor(rows.length/50));}}>添加价格</button>{endpoints.hasNextPage?<button type="button" className="secondary" disabled={endpoints.isFetching} onClick={()=>void endpoints.fetchNextPage()}>加载更多接口</button>:null}</div>
      {endpoints.isError?<p role="alert">{asAppError(endpoints.error).message}</p>:null}
    </>}
  </div>;
}
