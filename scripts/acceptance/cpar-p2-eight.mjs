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
  window.CPAR_P2_URLS={fixture:url("/src/dev/fixtures.ts"),api:url("/src/generated/management-client.ts"),versions:url("/src/features/config-versions/versionStore.ts"),query:url("/src/api/queryClient.ts")};
  window.CPAR_P2_COUNTS={};window.CPAR_P2_SESSIONS={};window.CPAR_P2_RECEIPT_SESSIONS=[];
  const {ManagementApi}=await import(window.CPAR_P2_URLS.api),original=ManagementApi.prototype.request;
  ManagementApi.prototype.request=async function(operation,request){
    window.CPAR_P2_COUNTS[operation]=(window.CPAR_P2_COUNTS[operation]??0)+1;
    const response=await original.call(this,operation,request);
    if(operation==="startKiroEnrollment"||operation==="startKimiEnrollment")window.CPAR_P2_SESSIONS[operation]=(await response.clone().json()).session_id;
    if(operation==="getAccountAuthorizationReceipt")window.CPAR_P2_RECEIPT_SESSIONS.push(request.path.session_id);
    return response;
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
}else if(["a02-oauth","a02-import","a02-api","a02-import-config","a02-api-config"].includes(scenario)){
  await page.evaluate(async scenario=>{
    const {ManagementApi}=await import(window.CPAR_P2_URLS.api),original=ManagementApi.prototype.request;
    window.CPAR_P2_RECEIPT=undefined;window.CPAR_P2_FAILED_READS=0;
    ManagementApi.prototype.request=async function(operation,request){
      const consuming=operation==="completeCodexEnrollment"||operation==="importChannelAccount";
      if((window.CPAR_P2_RECEIPT||scenario==="a02-import-config")&&(scenario==="a02-oauth"||scenario.endsWith("-config")?operation==="getConfigVersion":operation==="getCredential")){window.CPAR_P2_FAILED_READS++;return new Response(JSON.stringify({error:{code:"synthetic_read_unavailable",message:"synthetic follow-up unavailable"}}),{status:503,headers:{"Content-Type":"application/json"}});}
      const response=await original.call(this,operation,request);
      if(operation==="getAccountAuthorizationReceipt"||operation==="getAccountImportReceipt")window.CPAR_P2_RECEIPT=await response.clone().json();
      if(consuming)throw new Error("synthetic saved-then-lost response");
      return response;
    };
    if(scenario.startsWith("a02-api")){const {prepareApiConnectionMatchesForTest}=await import(window.CPAR_P2_URLS.fixture);prepareApiConnectionMatchesForTest(0);location.hash="#/upstreams";}else location.hash=scenario==="a02-oauth"?"#/oauth":"#/accounts";
  },scenario);
  if(scenario==="a02-oauth"){
    await page.click('loc=role:button[name="授权 Codex / ChatGPT"]');await page.click('loc=role:button[name="开始授权"]');
    await page.waitForSelector('textarea[aria-label="回调地址"]');
    const href=await page.evaluate(()=>document.querySelector('[role="dialog"] a')?.href),state=new URL(href).searchParams.get("state");assert.ok(state);
    await page.fill('textarea[aria-label="回调地址"]',`http://127.0.0.1:1455/auth/callback?code=synthetic&state=${state}`);
    await page.click('loc=role:button[name="完成授权"]');await page.waitForSelector('loc=role:button[name="核对授权结果"]');await page.click('loc=role:button[name="核对授权结果"]');
    await page.waitForFunction(()=>document.body.textContent.includes("synthetic follow-up unavailable")||document.body.textContent.includes("账号授权已保存"));
    assert.match(await content(),/账号授权已保存/u);assert.equal((await counts()).completeCodexEnrollment,1);
  }else if(scenario.startsWith("a02-import")){
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
  assert.ok(await page.evaluate(()=>window.CPAR_P2_FAILED_READS)>0,"must actually exercise the failing read");
  assert.equal(await page.evaluate(()=>window.CPAR_P2_RECEIPT.state),"completed");
  const savedId=await page.evaluate(()=>window.CPAR_P2_RECEIPT.credential_id);assert.ok(savedId);assert.ok((await content()).includes(savedId),"exact saved credential ID must remain visible");
}else if(scenario==="a05-kiro"||scenario==="a05-kimi"){
  const channel=scenario==="a05-kiro"?"kiro":"kimi-coding",label=channel==="kiro"?"Kiro":"Kimi Coding";
  await page.evaluate(async channel=>{const {failNextDevicePollForTest}=await import(window.CPAR_P2_URLS.fixture);failNextDevicePollForTest(channel);location.hash="#/oauth";},channel);
  await page.click(`loc=role:button[name="授权 ${label}"]`);await page.click('loc=role:button[name="开始授权"]');
  await page.waitForSelector('loc=role:button[name="关闭并标记结果未确认"]');
  const before=await counts();const sessionId=await page.evaluate(()=>Object.values(window.CPAR_P2_SESSIONS)[0]);assert.ok(sessionId);await page.click('loc=role:button[name="关闭并标记结果未确认"]');
  await page.waitForFunction(()=>!document.querySelector('[role="dialog"]'));
  assert.match(await content(),new RegExp(`${label}.*授权结果待确认`,"u"));
  await page.evaluate(()=>{location.hash="#/accounts";});await page.waitForSelector(`loc=role:button[name="返回 ${label} 授权任务"]`);
  await page.click(`loc=role:button[name="返回 ${label} 授权任务"]`);await page.click('loc=role:button[name="核对授权结果"]');
  await page.waitForFunction(()=>document.querySelector('[aria-label="授权结果恢复"]')?.textContent.includes("授权结果待确认"));
  const retained=await page.evaluate(async()=>{const paths=performance.getEntriesByType("resource").map(row=>row.name);const path=paths.find(row=>row.includes("/src/features/accounts/deviceRecoveryStore.ts?t="))??"/src/features/accounts/deviceRecoveryStore.ts";const {useDeviceRecoveryStore}=await import(path);return useDeviceRecoveryStore.getState().entries.map(entry=>({sessionId:entry.sessionId,taskId:entry.task.version.id,keys:Object.keys(entry)}));});
  assert.equal(retained[0].sessionId,sessionId);assert.ok(retained[0].taskId);assert.ok(!retained[0].keys.some(key=>["secret","user_code","verification_uri"].includes(key)));assert.deepEqual(await page.evaluate(()=>window.CPAR_P2_RECEIPT_SESSIONS),[sessionId]);
  const after=await counts();assert.equal(after.getAccountAuthorizationReceipt,1);assert.equal(after.startKiroEnrollment??after.startKimiEnrollment,before.startKiroEnrollment??before.startKimiEnrollment);assert.equal(after.pollKiroEnrollment??after.pollKimiEnrollment,before.pollKiroEnrollment??before.pollKimiEnrollment);
}else if(scenario==="a06-materials"||scenario.startsWith("a08-")){
  await page.evaluate(async scenario=>{
    const {prepareAccountPinForTest}=await import(window.CPAR_P2_URLS.fixture);prepareAccountPinForTest(true);
    const {ManagementApi}=await import(window.CPAR_P2_URLS.api),original=ManagementApi.prototype.request;
    window.CPAR_P2_RUNTIME_FAIL=false;
    ManagementApi.prototype.request=async function(operation,request){
      if(operation==="listProviderAccountPools"&&window.CPAR_P2_RUNTIME_FAIL)return new Response(JSON.stringify({error:{code:"synthetic_runtime_unavailable",message:"synthetic runtime unavailable"}}),{status:503,headers:{"Content-Type":"application/json"}});
      const response=await original.call(this,operation,request);
      if(operation==="listAccountInventory"){
        const body=await response.clone().json(),base=body.items[0];
        if(scenario==="a06-materials"){
          base.managed.credential.status="unauthorized";
          const oauth=structuredClone(base);oauth.id="p2-oauth";oauth.managed.credential.id="p2-oauth";oauth.managed.authentication="oauth";
          // Deliberately identical channel/display classification; only material type differs.
          body.items=[base,oauth];body.total=2;
        }
        return new Response(JSON.stringify(body),{status:200,headers:response.headers});
      }
      if(operation==="listProviderAccountPools"&&scenario.startsWith("a08-")){
        const body=await response.clone().json(),row=body.items.find(row=>row.account_id==="cred-relay-key");
        body.items=[{...row,credential_revision:scenario==="a08-current"?2:1,auth_status:"expired",runtime_status:"expired"}];body.observed_at_ms=Date.now()-3600000;
        return new Response(JSON.stringify(body),{status:200,headers:response.headers});
      }
      return response;
    };
    location.hash="#/upstreams";
  },scenario);
  await page.waitForSelector('tr[data-account-key="cred-relay-key"]');
  if(scenario==="a06-materials"){
    await page.waitForSelector('tr[data-account-key="p2-oauth"]');
    const rows=await page.evaluate(()=>[...document.querySelectorAll("tr[data-account-key]")].map(row=>({id:row.dataset.accountKey,status:row.querySelector(".badge-status")?.textContent,text:row.textContent})));
    assert.match(rows.find(row=>row.id==="cred-relay-key").status,/更新或替换 API Key/u);
    assert.match(rows.find(row=>row.id==="p2-oauth").status,/重新授权/u);
  }else{
    await page.waitForFunction(()=>document.querySelector('tr[data-account-key="cred-relay-key"]')?.textContent.includes("快照"));
    await page.evaluate(async()=>{window.CPAR_P2_RUNTIME_FAIL=true;const {queryClient}=await import(window.CPAR_P2_URLS.query);void queryClient.refetchQueries({queryKey:["account-list-runtime"]});});
    await page.waitForFunction(()=>document.querySelector('tr[data-account-key="cred-relay-key"]')?.textContent.includes("运行状态读取失败"));
    const text=await page.evaluate(()=>document.querySelector('tr[data-account-key="cred-relay-key"]')?.textContent);
    if(scenario==="a08-current")assert.match(text,/更新或替换 API Key/u);
    else {assert.doesNotMatch(text,/需要重新授权|更新或替换 API Key/u);assert.match(text,/当前授权.*active.*revision 2/u);}
    assert.match(text,/旧观察.*revision [12]/u);
  }
  assert.equal((await counts()).executeChannelPin??0,0);
}else if(scenario==="a07-pending"||scenario==="a07-error"){
  await page.evaluate(async()=>{const {prepareAccountPinForTest}=await import(window.CPAR_P2_URLS.fixture);prepareAccountPinForTest(true);location.hash="#/upstreams";});
  await page.waitForSelector('loc=role:button[name="模型"]');await page.click('loc=role:button[name="模型"]');
  await page.selectOption('loc=css:[role="dialog"] select >> nth=0',"key-cli");
  await page.selectOption('loc=css:[role="dialog"] select >> nth=1',"pm-minimax");
  await page.selectOption('loc=css:[role="dialog"] select >> nth=2',"ep-relay-a-responses");
  await page.click('loc=css:[role="dialog"] .check-row input[type="checkbox"]');await page.click('loc=role:button[name="执行一次调用测试"]');
  await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.textContent.includes("该身份下的调用观测"));
  await page.evaluate(async()=>{
    const {ManagementApi}=await import(window.CPAR_P2_URLS.api),original=ManagementApi.prototype.request;
    window.CPAR_P2_HELD=0;window.CPAR_P2_RELEASES=[];window.CPAR_P2_RELEASED=false;
    ManagementApi.prototype.request=async function(operation,request){
      if(operation==="getCredential"){window.CPAR_P2_HELD++;if(!window.CPAR_P2_RELEASED)await new Promise(resolve=>{window.CPAR_P2_RELEASES.push(resolve);});return new Response(JSON.stringify({error:{code:"synthetic_read_unavailable",message:"synthetic follow-up unavailable"}}),{status:503,headers:{"Content-Type":"application/json"}});}
      return original.call(this,operation,request);
    };
    const {queryClient}=await import(window.CPAR_P2_URLS.query);void queryClient.refetchQueries({queryKey:["account-call-evidence"]});
  });
  await page.waitForFunction(()=>window.CPAR_P2_HELD>0);
  if(scenario==="a07-error"){
    await page.evaluate(()=>{window.CPAR_P2_RELEASED=true;window.CPAR_P2_RELEASES.forEach(resolve=>resolve());});
    await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.textContent.includes("当前状态读取失败"));
  }
  const text=await page.evaluate(()=>document.querySelector('[role="dialog"]')?.textContent);
  assert.match(text,/历史观测，当前有效性待核对/u);assert.doesNotMatch(text,/该身份下的调用观测/u);assert.equal((await counts()).executeChannelPin,1);
}else throw new Error(`Unknown scenario ${scenario}`);
console.log(JSON.stringify({scenario,result:"PASS",environment:"local-synthetic",spaceId,counts:await counts()}));
