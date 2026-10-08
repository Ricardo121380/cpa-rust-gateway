import assert from "node:assert/strict";

// EgoLite only. The supplied origin must be this task's local synthetic fixture.
const {spaceId,origin,scenario}=globalThis.CPAR_P2;
assert.match(origin,/^http:\/\/127\.0\.0\.1:\d+$/u);
const task=await taskSpace(spaceId),page=task.page("p1");
await page.goto(`${origin}/#/unlock`);await page.reload();
await page.fill('input[name="password"]',"Prism-demo-2026");
await page.click('loc=role:button[name="登录"]');await page.waitForSelector("main.canvas");
await page.waitForFunction(()=>document.querySelector('main[data-context-version]'));
await page.evaluate(async()=>{
  const url=path=>performance.getEntriesByType("resource").map(row=>row.name).find(name=>name.includes(path+"?t="))??path;
  window.CPAR_P2_URLS={fixture:url("/src/dev/fixtures.ts"),api:url("/src/generated/management-client.ts"),versions:url("/src/features/config-versions/versionStore.ts")};
  window.CPAR_P2_COUNTS={};
  const {ManagementApi}=await import(window.CPAR_P2_URLS.api),original=ManagementApi.prototype.request;
  ManagementApi.prototype.request=async function(operation,request){
    window.CPAR_P2_COUNTS[operation]=(window.CPAR_P2_COUNTS[operation]??0)+1;
    return original.call(this,operation,request);
  };
});
const counts=()=>page.evaluate(()=>window.CPAR_P2_COUNTS);
const content=()=>page.evaluate(()=>document.body.textContent??"");
const versions=()=>page.evaluate(async()=>{const {useVersionStore}=await import(window.CPAR_P2_URLS.versions);const {context,pending}=useVersionStore.getState();return {context,pending};});
const openApi=async key=>{
  await page.click('loc=role:button[name="添加 AI 提供商"]');
  await page.waitForFunction(()=>document.querySelector("main")?.textContent.includes("没有匹配连接，将新建。")||document.querySelector("main")?.textContent.includes("将复用"));
  await page.fill('input[placeholder="例如：我的 OpenAI"]',"Synthetic P2 API");
  await page.fill('textarea[autocomplete="off"]',key);
  await page.click('loc=role:button[name="保存并接入"]');
};

if(["a03-close","a04-selected-empty","a04-tracked-empty","a04-nonempty"].includes(scenario)){
  await page.evaluate(async scenario=>{
    const {prepareApiConnectionMatchesForTest}=await import(window.CPAR_P2_URLS.fixture);prepareApiConnectionMatchesForTest(0);
    if(scenario.startsWith("a04-")){
      const {call}=await import("/src/api/client.ts"),{useVersionStore}=await import(window.CPAR_P2_URLS.versions);
      const current=useVersionStore.getState().context;
      let draft=await call("forkConfigVersion",{path:{config_version_id:current.configVersionId},headers:{"X-Config-Version":current.configVersionId,"If-Match":current.revision},body:{id:"p2-existing-draft",description:"Synthetic existing draft"}});
      if(scenario==="a04-nonempty"){
        await call("createUpstream",{headers:{"X-Config-Version":draft.id,"If-Match":draft.revision},body:{id:"prior-user-change",name:"Prior user change",kind:"openai-compatible",enabled:false,tags:[],egress_policy_id:null}});
        draft=await call("getConfigVersion",{path:{config_version_id:draft.id}});
      }
      if(scenario==="a04-selected-empty")useVersionStore.getState().select(draft);else useVersionStore.getState().rememberPending(draft);
    }
    location.hash="#/upstreams";
  },scenario);
  await openApi("synthetic-p2-first");
  await page.waitForFunction(()=>document.querySelector("main")?.textContent.includes("接入修改已应用")||document.querySelector("main")?.textContent.includes("服务与接口已保存到草稿"));
  if(scenario==="a04-nonempty"){
    assert.match(await content(),/服务与接口已保存到草稿/u);
    await page.click('loc=role:button[name="核对合并清单"]');
    await page.waitForFunction(()=>document.querySelector('[aria-label="本次配置核对"]')?.textContent.includes("prior-user-change"));
    assert.equal((await counts()).publishConfigVersion??0,0);
  }else{
    assert.match(await content(),/接入修改已应用/u);
    await page.click('loc=role:button[name="关闭"]');
    const state=await versions();assert.equal(state.context.status,"active");assert.equal(state.pending,undefined,"A-03 matching pending must settle without navigation");
    await openApi("synthetic-p2-second");
    await page.waitForFunction(()=>document.querySelector("main")?.textContent.includes("接入修改已应用")||document.querySelector("main")?.textContent.includes("待应用配置已变化"));
    assert.match(await content(),/接入修改已应用/u);assert.equal((await counts()).importChannelAccount,2);
  }
}else if(["a02-oauth","a02-import","a02-api"].includes(scenario)){
  await page.evaluate(async scenario=>{
    const {ManagementApi}=await import(window.CPAR_P2_URLS.api),original=ManagementApi.prototype.request;
    window.CPAR_P2_RECEIPT=undefined;
    ManagementApi.prototype.request=async function(operation,request){
      const consuming=operation==="completeCodexEnrollment"||operation==="importChannelAccount";
      if(window.CPAR_P2_RECEIPT&&(scenario==="a02-oauth"?operation==="getConfigVersion":operation==="getCredential"))return new Response(JSON.stringify({error:{code:"synthetic_read_unavailable",message:"synthetic follow-up unavailable"}}),{status:503,headers:{"Content-Type":"application/json"}});
      const response=await original.call(this,operation,request);
      if(operation==="getAccountAuthorizationReceipt"||operation==="getAccountImportReceipt")window.CPAR_P2_RECEIPT=await response.clone().json();
      if(consuming)throw new Error("synthetic saved-then-lost response");
      return response;
    };
    if(scenario==="a02-api"){const {prepareApiConnectionMatchesForTest}=await import(window.CPAR_P2_URLS.fixture);prepareApiConnectionMatchesForTest(0);location.hash="#/upstreams";}else location.hash=scenario==="a02-oauth"?"#/oauth":"#/accounts";
  },scenario);
  if(scenario==="a02-oauth"){
    await page.click('loc=role:button[name="授权 Codex / ChatGPT"]');await page.click('loc=role:button[name="开始授权"]');
    await page.waitForSelector('textarea[aria-label="回调地址"]');
    const href=await page.evaluate(()=>document.querySelector('[role="dialog"] a')?.href),state=new URL(href).searchParams.get("state");assert.ok(state);
    await page.fill('textarea[aria-label="回调地址"]',`http://127.0.0.1:1455/auth/callback?code=synthetic&state=${state}`);
    await page.click('loc=role:button[name="完成授权"]');await page.waitForSelector('loc=role:button[name="核对授权结果"]');await page.click('loc=role:button[name="核对授权结果"]');
    await page.waitForFunction(()=>document.body.textContent.includes("synthetic follow-up unavailable")||document.body.textContent.includes("账号授权已保存"));
    assert.match(await content(),/账号授权已保存/u);assert.equal((await counts()).completeCodexEnrollment,1);
  }else if(scenario==="a02-import"){
    await page.click('loc=role:button[name="导入账号文件"]');await page.click('loc=role:button[name*="Codex / ChatGPT"]');
    await page.fill('textarea[name="secret"]',"synthetic-local-material");await page.click('loc=role:button[name="导入账号"]');
    await page.waitForSelector('loc=role:dialog[name="导入结果"]');await page.click('loc=role:button[name="核对导入结果"]');
    await page.waitForFunction(()=>window.CPAR_P2_RECEIPT?.state==="completed"&&![...document.querySelectorAll("button")].some(button=>button.textContent.includes("核对导入结果")&&button.disabled));
    assert.match(await content(),/已保存，连接与应用待核对/u);assert.equal((await counts()).importChannelAccount,1);
  }else{
    await openApi("synthetic-p2-api");await page.waitForSelector('loc=role:button[name="只读取 API 授权结果"]');await page.click('loc=role:button[name="只读取 API 授权结果"]');
    await page.waitForFunction(()=>document.body.textContent.includes("synthetic follow-up unavailable"));
    assert.match(await content(),/API 授权已保存（/u);assert.equal((await counts()).importChannelAccount,1);
  }
  assert.equal(await page.evaluate(()=>window.CPAR_P2_RECEIPT.state),"completed");
  assert.ok(await page.evaluate(()=>window.CPAR_P2_RECEIPT.credential_id));
}else if(scenario==="a05-kiro"||scenario==="a05-kimi"){
  const channel=scenario==="a05-kiro"?"kiro":"kimi-coding",label=channel==="kiro"?"Kiro":"Kimi Coding";
  await page.evaluate(async channel=>{const {failNextDevicePollForTest}=await import(window.CPAR_P2_URLS.fixture);failNextDevicePollForTest(channel);location.hash="#/oauth";},channel);
  await page.click(`loc=role:button[name="授权 ${label}"]`);await page.click('loc=role:button[name="开始授权"]');
  await page.waitForSelector('loc=role:button[name="关闭并标记结果未确认"]');
  const before=await counts();await page.click('loc=role:button[name="关闭并标记结果未确认"]');
  await page.waitForFunction(()=>!document.querySelector('[role="dialog"]'));
  assert.match(await content(),new RegExp(`${label}.*授权结果待确认`,"u"));
  await page.click(`loc=role:button[name="返回 ${label} 授权任务"]`);await page.click('loc=role:button[name="核对授权结果"]');
  await page.waitForFunction(()=>document.querySelector('[aria-label="授权结果恢复"]')?.textContent.includes("授权结果待确认"));
  const after=await counts();assert.equal(after.getAccountAuthorizationReceipt,1);assert.equal(after.startKiroEnrollment??after.startKimiEnrollment,before.startKiroEnrollment??before.startKimiEnrollment);assert.equal(after.pollKiroEnrollment??after.pollKimiEnrollment,before.pollKiroEnrollment??before.pollKimiEnrollment);
}else throw new Error(`Unknown scenario ${scenario}`);
console.log(JSON.stringify({scenario,result:"PASS",environment:"local-synthetic",spaceId,counts:await counts()}));
