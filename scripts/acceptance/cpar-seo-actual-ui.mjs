import assert from "node:assert/strict";
import {readFile,writeFile} from "node:fs/promises";
import {dirname} from "node:path";

const {spaceId,pageLabel="p2",descriptor,out,taskSpace,phase="all",firstLogin=true,presets=["responses","chat","messages","kimi","grok-api"]}=globalThis.CPAR_SEO;
const d=JSON.parse(await readFile(descriptor,"utf8"));
assert.match(d.origin,/^http:\/\/127\.0\.0\.1:\d+$/u);
const page=(await taskSpace(spaceId)).page(pageLabel),results=[];
const password=(await readFile(d.password_file,"utf8")).trim();
const materials=JSON.parse(await readFile(d.api_material_file,"utf8"));
async function record(row){
  results.push(row);
  await writeFile(out,JSON.stringify({environment:"stock serve embedded /admin-ui/",spaceId,pageLabel,origin:d.origin,gatewayPid:d.gateway_pid,binarySha256:d.binary_sha256,sourceSha:d.source_sha,managementHandlerStorage:true,upstream:"synthetic loopback only",results},null,2)+"\n");
  console.log(JSON.stringify({scenario:row.scenario,result:row.result}));
}
async function text(){return page.evaluate(()=>document.querySelector("main")?.textContent??"");}
async function observed(){return page.evaluate(()=>({records:window.CPAR_SEO_RECORDS,text:document.querySelector("main")?.textContent}));}
async function install(){
  // Install before the generated client captures its transport. Never serialize session values.
  await page.cdp("Page.addScriptToEvaluateOnNewDocument",{source:String.raw`(()=>{
    const original=window.fetch.bind(window);window.CPAR_SEO_RECORDS=[];window.CPAR_SEO_HEADERS={};
    const scrub=value=>Array.isArray(value)?value.map(scrub):value&&typeof value==='object'?Object.fromEntries(Object.entries(value).filter(([key])=>!['actor','secret','key','password','token','access_token','refresh_token','csrf_token','management_key'].includes(key)).map(([key,value])=>[key,scrub(value)])):value;
    window.fetch=async(input,init)=>{
      const url=new URL(typeof input==='string'?input:input.url,location.href),headers=new Headers(init?.headers??(typeof input==='string'?undefined:input.headers));
      for(const name of ['x-management-key','x-management-csrf-token'])if(headers.has(name))window.CPAR_SEO_HEADERS[name]=headers.get(name);
      const method=init?.method??'GET';let body={};try{body=JSON.parse(init?.body??'{}')}catch{}
      if(window.CPAR_SEO_FAIL_CONFIG&&method==='GET'&&/^\/admin\/config-versions\/[^/]+$/.test(url.pathname)){
        window.CPAR_SEO_FAIL_CONFIG=false;window.CPAR_SEO_RECORDS.push({method,path:url.pathname,injected:'client follow-up read failure'});
        return new Response(JSON.stringify({error:{code:'management_runtime_unavailable',message:'Synthetic follow-up read failure'}}),{status:503,headers:{'content-type':'application/json'}});
      }
      const response=await original(input,init);
      if(url.pathname.startsWith('/admin/')&&!url.pathname.includes('/auth/')){
        const data=await response.clone().json().catch(()=>null);
        window.CPAR_SEO_RECORDS.push({method,path:url.pathname,config:headers.get('x-config-version'),ifMatch:headers.get('if-match'),authenticated:headers.has('x-management-key'),csrf:headers.has('x-management-csrf-token'),sameOrigin:url.origin===location.origin,body:scrub(body),status:response.status,response:scrub(data)});
        if(window.CPAR_SEO_LOSE_IMPORT&&method==='POST'&&url.pathname.endsWith('/account-import')&&response.status===201){window.CPAR_SEO_LOSE_IMPORT=false;throw new TypeError('Synthetic client response loss after genuine server commit');}
        if(window.CPAR_SEO_FAIL_AFTER_RECEIPT&&method==='GET'&&url.pathname.includes('/account-imports/')&&response.status===200){window.CPAR_SEO_FAIL_AFTER_RECEIPT=false;window.CPAR_SEO_FAIL_CONFIG=true;}
      }
      return response;
    };
  })()`});
}
async function login(first=false){
  await page.goto(`${d.origin}/admin-ui/#/unlock`);
  await page.reload();
  await page.waitForSelector('input[name="password"]');
  await page.fill('input[name="password"]',first?(await readFile(d.initial_password_file,"utf8")).trim():password);
  await page.click('loc=role:button[name="登录"]');
  if(first){
    await page.waitForSelector('input[autocomplete="new-password"]');
    await page.fill('input[name="password"]',(await readFile(d.initial_password_file,"utf8")).trim());
    await page.fill('input[autocomplete="new-password"] >> nth=0',password);
    await page.fill('input[autocomplete="new-password"] >> nth=1',password);
    await page.click('loc=role:button[name="保存并重新登录"]');
    await page.waitForSelector('input[name="username"]');
    await page.fill('input[name="password"]',password);await page.click('loc=role:button[name="登录"]');
  }
  await page.waitForSelector("main.canvas");
}
async function provider(preset,material){
  await page.goto(`${d.origin}/admin-ui/#/upstreams`);
  await page.click('loc=role:button[name="添加 AI 提供商"]');
  await page.selectOption('form#provider-setup-form select:not([required])',preset);
  await page.fill('input[placeholder="例如：我的 OpenAI"]',`Actual ${preset}`);
  if(preset!=="grok-api")await page.fill('input[type="url"]',d.targets[preset].base);
  await page.waitForFunction(()=>document.querySelector("main")?.textContent.includes("将复用"));
  await page.fill('textarea[autocomplete="off"]',material);
}
async function clear(){await page.evaluate(()=>{window.CPAR_SEO_RECORDS=[];});}
async function closeProvider(){await page.click('loc=role:button[name="关闭"]');await page.waitForFunction(()=>!document.querySelector(".inline-workspace"));}
async function prepare(kind){
  // Isolated setup through real management handlers, separately identified from GUI actions.
  const key=(await readFile(`${d.root}/credentials/management-key`,"utf8")).trim();
  let scope,revision;const requests=[];
  async function api(method,path,body,expected=200,extra={}){
    const headers={"X-Management-Key":key,"Content-Type":"application/json",...(scope?{"X-Config-Version":scope}:{}),...(revision&&method!=="GET"?{"If-Match":revision}:{}),...extra};
    const response=await fetch(d.origin+path,{method,headers,body:body===undefined?undefined:JSON.stringify(body)});
    const data=await response.json();assert.equal(response.status,expected,`${method} ${path}: ${data.error?.code}`);
    if(method!=="GET"&&response.headers.get("etag"))revision=response.headers.get("etag");
    requests.push({method,path,status:response.status,scope,revision});return data;
  }
  const active=(await api("GET","/admin/config-versions")).find(row=>row.status==="active");
  scope=active.id;revision=active.revision;
  const draft=await api("POST",`/admin/config-versions/${active.id}/fork`,{id:`ui-${kind}-${Date.now()}`,description:`Isolated ${kind} preparation`},201);
  scope=draft.id;revision=draft.revision;
  if(kind==="prior")await api("POST","/admin/access-groups",{id:"ui-prior-group",name:"Prior independent modification",status:"active",limits:{}},201);
  else{
    for(const id of ["ui-calls-group","ui-ungranted-group"]){
      await api("POST","/admin/access-groups",{id,name:id,status:"active",limits:{}},201);
      await api("POST","/admin/client-keys",{id:id==="ui-calls-group"?"ui-calls-key":"ui-ungranted-key",access_group_id:id,status:"active",expires_at_ms:null},201);
    }
    for(const preset of ["responses","chat","messages","kimi"]){
      const model=`ui-model-${preset}`,route=`ui-route-${preset}`;
      await api("POST","/admin/public-models",{id:model,model_name:model,display_name:model,status:"active",capabilities:{}},201);
      await api("POST",`/admin/public-models/${model}/routes`,{id:route,policy:"smooth_weighted_round_robin",max_attempts:1,bootstrap_timeout_ms:5000},201);
      await api("POST",`/admin/routes/${route}/candidates`,{id:`ui-candidate-${preset}`,endpoint_id:d.targets[preset].endpoint_id,upstream_model:`ui-wire-${preset}`,credential_scope:"all_active",transform_mode:"canonical_bridge",enabled:true,priority:0,weight:1,capability_override:{allow_unlisted_model:true}},201);
      await api("POST","/admin/access-groups/ui-calls-group/routes",{route_id:route,enabled:true},201);
    }
    assert((await api("POST",`/admin/config-versions/${scope}/validate`)).valid);
    const events=await api("GET","/admin/audit-events"),event=Math.max(0,...events.filter(row=>["config_published","config_rolled_back"].includes(row.action)).map(row=>row.id));
    await api("POST",`/admin/config-versions/${scope}/publish`,undefined,200,{"X-Expected-Active-Version":JSON.stringify(active.id),"X-Expected-Lifecycle-Event":String(event)});
  }
  return {kind,scope,revision,requests,source:"test setup via real management handler; owned temporary CLI key"};
}
await install();
if(phase==="all"||phase==="presets"){
  await login(firstLogin);
  await record({scenario:firstLogin?"administrator-first-change-and-login":"administrator-login",result:"PASS",actualPage:await page.url()});
  for(const preset of presets){
    await provider(preset,materials[preset]);await clear();
    await page.click('loc=role:button[name="保存并接入"]');
    await page.waitForFunction(()=>document.querySelector("main")?.textContent.includes("接入修改已应用"));
    const facts=await observed(),target=d.targets[preset];
    const imports=facts.records.filter(row=>row.method==="POST"&&row.path.endsWith("/account-import"));
    assert.equal(imports.length,1);assert.equal(imports[0].path,`/admin/upstreams/${target.upstream_id}/account-import`);
    const channel=preset==="kimi"?"kimi-api":preset==="grok-api"?"grok.official":preset==="messages"?"anthropic-compatible":"openai-compatible";
    assert.equal(imports[0].body.channel,channel);assert.equal(imports[0].status,201);
    const binding=facts.records.find(row=>row.method==="POST"&&row.path===`/admin/endpoints/${target.endpoint_id}/credential-bindings`);
    assert.equal(binding?.body.credential_id,imports[0].response.id);
    assert.equal(facts.records.filter(row=>row.method==="POST"&&row.path.endsWith("/publish")).length,1);
    assert.equal(facts.records.filter(row=>row.method==="POST"&&(row.path==="/admin/upstreams"||row.path.endsWith("/endpoints"))).length,0);
    const writes=facts.records.filter(row=>row.method==="POST");assert(writes.every(row=>row.authenticated&&row.csrf&&row.sameOrigin));
    const endpoint=facts.records.filter(row=>row.method==="GET"&&row.path==="/admin/endpoints"&&row.status===200).at(-1).response.items.find(row=>row.id===target.endpoint_id);
    assert.equal(endpoint.enabled,target.endpoint_enabled);assert.equal(endpoint.transport,"https");
    assert.equal(endpoint.upstream_id,target.upstream_id);assert.equal(endpoint.adapter_id,target.adapter);assert.equal(endpoint.api_format,target.format);
    assert.equal(endpoint.base_url,target.base);assert.equal(endpoint.inference_path,target.path);
    const owner=facts.records.filter(row=>row.method==="GET"&&row.path==="/admin/upstreams"&&row.status===200).at(-1).response.find(row=>row.id===target.upstream_id);
    assert.equal(owner.enabled,target.enabled);
    await record({scenario:`E-01-actual-${preset}-reuse-save-bind-apply`,result:"PASS",target,accountId:imports[0].response.id,channel,facts});
    await closeProvider();
  }
  const security=await page.evaluate(async()=>{
    const headers={...window.CPAR_SEO_HEADERS,"Content-Type":"application/json"};
    const noCsrf={...headers};delete noCsrf["x-management-csrf-token"];
    return {unauthenticated:(await fetch("/admin/config-versions")).status,noCsrfWrite:(await fetch("/admin/config-versions",{method:"POST",headers:noCsrf,body:JSON.stringify({id:"must-not-exist",description:"CSRF negative"})})).status};
  });
  const sessionHeaders=await page.evaluate(()=>window.CPAR_SEO_HEADERS);
  security.foreignOrigin=(await fetch(`${d.origin}/admin/config-versions`,{headers:{...sessionHeaders,Origin:"https://foreign.invalid"}})).status;
  assert.equal(security.unauthenticated,404);assert.equal(security.noCsrfWrite,404);
  assert.equal(security.foreignOrigin,404);
  await record({scenario:"actual-authentication-and-CSRF-negatives",result:"PASS",security});
}else await login();
if(phase==="all"||phase==="recovery"){
  await provider("responses",materials.lost);await clear();
  await page.evaluate(()=>{window.CPAR_SEO_LOSE_IMPORT=true;window.CPAR_SEO_FAIL_AFTER_RECEIPT=true;});
  await page.click('loc=role:button[name="保存并接入"]');
  await page.waitForSelector('loc=role:button[name="只读取 API 授权结果"]');
  const committed=await observed();
  const imports=committed.records.filter(row=>row.method==="POST"&&row.path.endsWith("/account-import"));
  assert.equal(imports.length,1);assert.equal(imports[0].status,201);
  await page.click('loc=role:button[name="只读取 API 授权结果"]');
  await page.waitForFunction(()=>document.querySelector("main")?.textContent.includes("API 授权已保存；后续配置或账号读取失败"));
  const recovered=await observed();
  const receipt=recovered.records.find(row=>row.method==="GET"&&row.path.includes("/account-imports/")&&row.status===200);
  assert.equal(receipt?.response.credential_id,imports[0].response.id);
  assert.equal(recovered.records.filter(row=>row.method==="POST"&&row.path.endsWith("/account-import")).length,1);
  await record({scenario:"A-02-actual-commit-response-discard-and-receipt-then-read-failure",result:"PASS",faultLayer:"browser discards genuine 201 after commit; one later browser read is synthetic 503",savedAccountId:imports[0].response.id,committed,recovered});
  // Continue by explicit full-diff review; do not repeat the import.
  for(let round=0;round<3&&!(await text()).includes("接入修改已应用");round+=1){
    await page.click('loc=role:button[name="核对合并清单"]');
    await page.waitForSelector('section[aria-label="本次配置核对"] input[type="checkbox"]');
    await page.click('section[aria-label="本次配置核对"] input[type="checkbox"]');
    const name=(await text()).includes("按这份清单继续连接")?"按这份清单继续连接":"校验并应用这份清单";
    await page.focus(`loc=role:button[name="${name}"]`);
    await page.press(`loc=role:button[name="${name}"]`,"Enter");
    await page.waitForFunction(()=>!document.querySelector('section[aria-label="本次配置核对"] input[type="checkbox"]'));
  }
  assert((await text()).includes("接入修改已应用"));
  const applied=await observed();assert.equal(applied.records.filter(row=>row.method==="POST"&&row.path.endsWith("/account-import")).length,1);
  await record({scenario:"A-02-A-04-actual-explicit-continue-bind-and-apply",result:"PASS",facts:applied});
  await closeProvider();
}
if(phase==="all"||phase==="kiro"){
  for(const kind of ["oauth","api_key"]){
    await page.goto(`${d.origin}/admin-ui/#/accounts`);await page.click('loc=role:button[name="导入账号文件"]');
    await page.click('button.account-channel-option:has-text("Kiro")');
    await page.waitForSelector('textarea[name="secret"]');
    const material=kind==="oauth"?await readFile(d.kiro_material_file,"utf8"):`ksk_${materials.new}`;
    await page.fill('textarea[name="secret"]',material);await clear();await page.click('loc=role:button[name="导入账号"]');
    await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.textContent.includes("配置已生效"));
    const imported=await observed();const save=imported.records.find(row=>row.method==="POST"&&row.path.endsWith("/account-import"));
    assert.equal(save?.path,"/admin/upstreams/kiro-us-east-1/account-import");
    const accountId=save.response.id;
    await page.click('loc=role:button[name="完成"]');
    await page.waitForFunction(()=>!document.querySelector('[role="dialog"]'));
    const inventory=await page.evaluate(async()=>{
      const versions=await (await fetch("/admin/config-versions",{headers:window.CPAR_SEO_HEADERS})).json();const active=versions.find(row=>row.status==="active");
      return (await fetch("/admin/accounts/inventory?category=kiro",{headers:{...window.CPAR_SEO_HEADERS,"X-Config-Version":active.id}})).json();
    });
    const account=inventory.items.find(row=>row.id===accountId);assert(account);
    assert.equal(account.managed.category,"kiro");assert.equal(account.managed.authentication,kind==="oauth"?"oauth":"api_key");
    assert.equal(account.operations.includes("reauthorize"),kind==="oauth");
    await record({scenario:`E-03-A-06-actual-Kiro-${kind}-classification`,result:"PASS",accountId,account,imported});
    if(kind==="oauth"){
      await page.click('loc=role:button[name="重新授权"]');await page.waitForSelector('loc=role:button[name="开始授权"]');
      const before=await observed();assert.equal(before.records.filter(row=>row.path.includes('/kiro-authorization/start')).length,0);
      await record({scenario:"A-05-A-06-actual-Kiro-reauthorization-entry-without-exchange",result:"PASS",facts:before});
      await page.click('loc=role:button[name="取消"]');
    }
  }
}
if(phase==="all"||phase==="prior"){
  const prepared=await prepare("prior");
  await page.goto(`${d.origin}/admin-ui/#/versions`);
  await page.click(`tr[data-version-id="${prepared.scope}"] button:text-is("编辑草稿")`);await page.waitForSelector('loc=role:button[name="接续草稿"]');
  await page.click('loc=role:button[name="接续草稿"]');await page.waitForFunction(()=>!document.querySelector('[role="dialog"]'));
  await provider("responses",materials.new);await clear();await page.click('loc=role:button[name="保存并接入"]');
  await page.waitForFunction(()=>document.querySelector('.inline-workspace')?.textContent.includes("服务与接口已保存到草稿，尚未应用"));
  const saved=await observed();assert.equal(saved.records.filter(row=>row.method==="POST"&&row.path.endsWith("/publish")).length,0);
  await page.click('loc=role:button[name="核对合并清单"]');await page.waitForSelector('section[aria-label="本次配置核对"] input[type="checkbox"]');
  const preview=await observed();assert.match(preview.text,/本次操作前的修改 · [1-9]\d* 项/u);assert(preview.text.includes("ui-prior-group"));
  await page.click('section[aria-label="本次配置核对"] input[type="checkbox"]');
  await page.focus('loc=role:button[name="校验并应用这份清单"]');await page.press('loc=role:button[name="校验并应用这份清单"]',"Enter");
  await page.waitForFunction(()=>document.querySelector('main')?.textContent.includes("接入修改已应用"));
  await record({scenario:"A-04-actual-existing-nonempty-draft-explicit-review-and-apply",result:"PASS",prepared,saved,preview,applied:await observed()});
  await closeProvider();
}
if(phase==="all"||phase==="calls"){
  const prepared=await prepare("routes");await login();
  for(const preset of ["responses","chat","messages","kimi"]){
    const before=JSON.parse(await readFile(`${dirname(descriptor)}/provider-receives.json`,"utf8"));
    await page.goto(`${d.origin}/admin-ui/#/upstreams?provider=${d.targets[preset].upstream_id}`);
    await page.waitForFunction(()=>[...document.querySelectorAll("button")].some(node=>node.textContent.trim()==="模型"&&node.getBoundingClientRect().width>0));
    const visibleModelIndex=await page.evaluate(()=>[...document.querySelectorAll("button")].filter(node=>node.textContent.trim()==="模型").findIndex(node=>node.getBoundingClientRect().width>0));
    await page.click(`button:text-is("模型") >> nth=${visibleModelIndex}`);
    await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.textContent.includes("1 个已连接模型"));
    // Reading an account and its models must leave the upstream receive count unchanged.
    const afterOpen=JSON.parse(await readFile(`${dirname(descriptor)}/provider-receives.json`,"utf8"));assert.equal(afterOpen.length,before.length);
    await page.selectOption('[role="dialog"] select >> nth=0',"ui-calls-key");
    await page.selectOption('[role="dialog"] select >> nth=1',`ui-model-${preset}`);
    await page.selectOption('[role="dialog"] select >> nth=2',d.targets[preset].endpoint_id);
    await page.selectOption('[role="dialog"] select >> nth=3',preset==="messages"?"anthropic_messages":preset==="responses"?"openai_responses":"openai_chat_completions");
    await page.click('[role="dialog"] input[type="checkbox"]');await clear();
    await page.click('loc=role:button[name="执行一次调用测试"]');
    await page.waitForFunction(()=>window.CPAR_SEO_RECORDS.some(row=>row.method==="POST"&&row.path==="/admin/operations/channel-pin"));
    const facts=await observed(),pin=facts.records.find(row=>row.method==="POST"&&row.path==="/admin/operations/channel-pin");
    assert.equal(pin.status,200);assert.equal(pin.response.outcome,"succeeded");assert.equal(pin.response.upstream_sent,true);assert.equal(pin.response.attempt_count,1);
    assert.equal(pin.response.credential_id,pin.body.credential_id);assert.equal(pin.response.channel_id,d.targets[preset].endpoint_id);
    let receives=[];
    for(let attempt=0;attempt<50;attempt+=1){
      receives=JSON.parse(await readFile(`${dirname(descriptor)}/provider-receives.json`,"utf8"));
      if(receives.length>=before.length+1)break;
      await new Promise(resolve=>setTimeout(resolve,100));
    }
    assert.equal(receives.length,before.length+1);
    assert.equal(receives.at(-1).model,`ui-wire-${preset}`);
    await record({scenario:`A-07-E-01-actual-${preset}-explicit-single-call`,result:"PASS",prepared,receivedBefore:before.length,receives,facts,pin});
    await page.click('[role="dialog"] button:text-is("关闭")');await page.waitForFunction(()=>!document.querySelector('[role="dialog"]'));
  }
  async function mappingFacts(){return page.evaluate(async()=>{
    const versions=await(await fetch("/admin/config-versions",{headers:window.CPAR_SEO_HEADERS})).json(),active=versions.find(row=>row.status==="active");
    const headers={...window.CPAR_SEO_HEADERS,"X-Config-Version":active.id};
    const candidates=await fetch("/admin/route-candidates",{headers}),grants=await fetch("/admin/access-groups/ui-calls-group/routes",{headers});
    if(candidates.status!==200||grants.status!==200)throw new Error("Mapping readback failed");
    return {active:active.id,candidates:await candidates.json(),grants:await grants.json()};
  });}
  const mappingBefore=await mappingFacts();
  await provider("responses",materials.responses);await page.fill('textarea[placeholder="每行一个真实模型 ID"]',"ui-model-responses");
  await page.click('#provider-setup-form input[type="checkbox"]');await clear();await page.click('loc=role:button[name="保存并接入"]');
  await page.waitForFunction(()=>document.querySelector('.inline-workspace')?.textContent.includes("此名称已有自定义模型映射"));
  const facts=await observed();assert.equal(facts.records.filter(row=>row.method==="POST"&&row.path.endsWith("/candidates")).length,0);
  assert.equal(facts.records.filter(row=>row.method==="POST"&&row.path.includes('/access-groups/')).length,0);
  assert.equal(facts.records.filter(row=>row.method==="POST"&&row.path.endsWith("/publish")).length,0);
  const mappingAfter=await mappingFacts();assert.deepEqual(mappingAfter,mappingBefore);
  await record({scenario:"A-08-actual-existing-custom-mapping-refused-without-route-or-grant-write",result:"PASS",facts,mappingBefore,mappingAfter});
  await page.click('loc=role:button[name="取消"]');await page.waitForFunction(()=>!document.querySelector('.inline-workspace'));
}
