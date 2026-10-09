import assert from "node:assert/strict";
import {readFile,writeFile} from "node:fs/promises";
import {dirname} from "node:path";
import {installManagementObserver} from "./cpar-management-observer.mjs";

export async function operationsHarness(options){
  const d=JSON.parse(await readFile(options.descriptor,"utf8"));
  assert.match(d.origin,/^http:\/\/127\.0\.0\.1:\d+$/u);
  const page=(await options.taskSpace(options.spaceId)).page(options.pageLabel??"p1");
  const facts=()=>page.evaluate(()=>({records:window.CPAR_SEO_RECORDS??[],text:document.querySelector("main")?.textContent??"",dialogText:document.querySelector('[role="dialog"]')?.textContent??""}));
  const clear=()=>page.evaluate(()=>{window.CPAR_SEO_RECORDS=[];});
  async function record(row){
    const prior=JSON.parse(await readFile(options.out,"utf8").catch(()=>"{\"results\":[]}"));
    prior.environment="stock serve /admin-ui/, synthetic loopback only";
    prior.sourceSha=d.source_sha;prior.binarySha256=d.binary_sha256;prior.origin=d.origin;
    prior.spaceId=options.spaceId;prior.timeUtc=new Date().toISOString();
    prior.results.push(row);await writeFile(options.out,JSON.stringify(prior,null,2)+"\n");
    console.log(JSON.stringify({scenario:row.scenario,result:row.result}));
  }
  async function config(){
    return page.evaluate(async()=>{
      const headers=window.CPAR_SEO_HEADERS;
      const versions=await(await fetch("/admin/config-versions",{headers})).json();
      const active=versions.find(row=>row.status==="active");
      const scoped={...headers,"X-Config-Version":active.id};
      async function get(path){const r=await fetch(path,{headers:scoped});if(!r.ok)throw new Error(`${path} ${r.status}`);return r.json();}
      async function list(path){const value=await get(path);return Array.isArray(value)?value:value.items;}
      const groups=await list("/admin/access-groups");
      return {active,upstreams:await get("/admin/upstreams"),endpoints:await get("/admin/endpoints?limit=100"),
        accounts:(await list("/admin/credentials?limit=100")).map(row=>row.credential??row),groups,grants:await Promise.all(groups.map(async row=>({group:row.id,routes:await list(`/admin/access-groups/${row.id}/routes`)}))),keys:await list("/admin/client-keys"),routes:await list("/admin/routes"),models:await list("/admin/public-models")};
    });
  }
  const key=(await readFile(`${d.root}/credentials/management-key`,"utf8")).trim();
  let scope,revision;
  const requests=[];
  async function api(method,path,body,expected=200,extra={}){
    const headers={"X-Management-Key":key,"Content-Type":"application/json",...(scope?{"X-Config-Version":scope}:{}),...(revision&&method!=="GET"?{"If-Match":revision}:{}),...extra};
    const r=await fetch(d.origin+path,{method,headers,body:body===undefined?undefined:JSON.stringify(body)});
    const value=await r.json();assert.equal(r.status,expected,`${method} ${path}: ${value.error?.code}`);
    if(method!=="GET"&&r.headers.get("etag"))revision=r.headers.get("etag");
    requests.push({method,path,status:r.status,scope,revision});return value;
  }
  async function seed(action,{publish=true}={}){
    const versions=await api("GET","/admin/config-versions");const active=versions.find(row=>row.status==="active");
    scope=active.id;revision=active.revision;
    const draft=await api("POST",`/admin/config-versions/${active.id}/fork`,{id:`ops-seed-${Date.now()}`,description:"Owned synthetic setup"},201);
    scope=draft.id;revision=draft.revision;await action(api);
    if(publish){
      assert((await api("POST",`/admin/config-versions/${scope}/validate`)).valid);
      const events=await api("GET","/admin/audit-events"),event=Math.max(0,...events.filter(row=>["config_published","config_rolled_back"].includes(row.action)).map(row=>row.id));
      await api("POST",`/admin/config-versions/${scope}/publish`,undefined,200,{"X-Expected-Active-Version":JSON.stringify(active.id),"X-Expected-Lifecycle-Event":String(event)});
    }
    return {requests:requests.splice(0),scope,source:"synthetic setup via real CLI-authenticated management handlers"};
  }
  async function deviceFacts({ordinal,requests=0}={}){
    for(const deadline=Date.now()+3000;Date.now()<deadline;){
      try{
        const value=JSON.parse(await readFile(`${dirname(options.descriptor)}/device-receives.json`,"utf8"));
        if(ordinal===undefined||value.sessions.some(row=>row.ordinal===ordinal&&row.token_requests>=requests))return value;
      }catch(error){if(!(error instanceof SyntaxError))throw error;}
      await new Promise(resolve=>setTimeout(resolve,100));
    }
    throw new Error("Synthetic device receive snapshot did not reach the observed terminal state");
  }
  async function login(first=false){
    await installManagementObserver(page);
    await page.goto(`${d.origin}/admin-ui/#/unlock`);await page.reload();
    await page.waitForSelector('input[name="password"]');
    const password=(await readFile(d.password_file,"utf8")).trim();
    const initial=first?(await readFile(d.initial_password_file,"utf8")).trim():password;
    await page.fill('input[name="password"]',initial);
    await page.click('loc=role:button[name="登录"]');
    if(first){
      await page.waitForSelector('input[autocomplete="new-password"]');
      await page.fill('input[name="password"]',initial);
      await page.fill('input[autocomplete="new-password"] >> nth=0',password);
      await page.fill('input[autocomplete="new-password"] >> nth=1',password);
      await page.click('loc=role:button[name="保存并重新登录"]');
      await page.waitForSelector('input[name="username"]');
      await page.fill('input[name="password"]',password);
      await page.click('loc=role:button[name="登录"]');
    }
    await page.waitForSelector("main.canvas");
    await page.waitForFunction(()=>!!window.CPAR_SEO_HEADERS?.["x-management-key"]);
  }
  return {d,page,facts,clear,record,config,api,seed,deviceFacts,login};
}
