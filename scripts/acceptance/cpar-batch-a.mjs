import assert from "node:assert/strict";

// Run through ego-browser nodejs with CPAR_A={spaceId,origin,scenario}.
// Only the local development fixture is admitted; all materials are synthetic.
const {spaceId,origin,scenario}=globalThis.CPAR_A;
assert.match(origin,/^http:\/\/127\.0\.0\.1:\d+$/u);
const task=await taskSpace(spaceId);
const page=task.page("p1");
await page.goto(`${origin}/#/unlock`);
await page.reload();
await page.fill('input[name="password"]',"Prism-demo-2026");
await page.click('loc=role:button[name="登录"]');
await page.waitForSelector("main.canvas");
if(scenario==="local-release") {
  await page.evaluate(async()=>{
    const url=performance.getEntriesByType('resource').map(row=>row.name).find(url=>/\/src\/dev\/fixtures\.ts\?t=/u.test(url))??"/src/dev/fixtures.ts";
    window.CPAR_A_FIXTURE_URL=url;
    const {trackFixtureOperationForTest}=await import(url);
    trackFixtureOperationForTest("POST /admin/operations/provider-account-pools/actions");
    trackFixtureOperationForTest("POST /admin/operations/channel-pin");
    location.hash="#/accounts?view=runtime";
  });
  const operations=()=>page.evaluate(async()=>{const {fixtureOperationCallsForTest}=await import(window.CPAR_A_FIXTURE_URL);return fixtureOperationCallsForTest("POST /admin/operations/provider-account-pools/actions");});
  const openRelease=async id=>{
    await page.waitForSelector(`tr[data-account-id="${id}"]`);
    await page.click(`loc=css:tr[data-account-id="${id}"] button`);
    await page.click('loc=role:button[name="请求恢复"]');
    await page.waitForSelector('loc=role:dialog[name="高级：解除本地隔离"]');
  };
  await openRelease("cred-relay-spare");
  assert.match(await page.evaluate(()=>document.querySelector('[role="dialog"]')?.textContent??""),/cooling.*允许再次尝试，尚未验证/u);
  await page.click('loc=role:button[name="确认解除本地隔离"]');
  assert.equal(await operations(),0,"confirmation is required before a local mutation");
  await page.click('loc=css:[role="dialog"] input[type="checkbox"]');
  await page.click('loc=role:button[name="确认解除本地隔离"]');
  await page.waitForFunction(()=>document.querySelector('main')?.textContent.includes("允许再次尝试，尚未验证"));
  await page.waitForFunction(()=>document.querySelector('tr[data-account-id="cred-relay-spare"]')?.textContent.includes("可用"));
  assert.equal(await operations(),1);
  await openRelease("cred-relay-quota");
  await page.click('loc=css:[role="dialog"] input[type="checkbox"]');
  await page.click('loc=role:button[name="确认解除本地隔离"]');
  await page.waitForFunction(()=>document.querySelector('main')?.textContent.includes("需要人工恢复"));
  assert.match(await page.evaluate(()=>document.querySelector('tr[data-account-id="cred-relay-quota"]')?.textContent??""),/配额|额度/u);
  await openRelease("cred-grok-oauth");
  await page.click('loc=css:[role="dialog"] input[type="checkbox"]');
  await page.click('loc=role:button[name="确认解除本地隔离"]');
  await page.waitForFunction(()=>document.querySelector('main')?.textContent.includes("被拒绝"));
  assert.equal(await operations(),3);
  assert.equal(await page.evaluate(async()=>{const {fixtureOperationCallsForTest}=await import(window.CPAR_A_FIXTURE_URL);return fixtureOperationCallsForTest("POST /admin/operations/channel-pin");}),0);
  console.log(JSON.stringify({scenario,result:"PASS",environment:"local-synthetic",spaceId}));
} else if(scenario==="catalog-matrix") {
  await page.evaluate(async()=>{
    const fixtureUrl=performance.getEntriesByType("resource").map(row=>row.name).find(url=>/\/src\/dev\/fixtures\.ts\?t=/u.test(url))??"/src/dev/fixtures.ts";
    const {prepareAccountPinForTest}=await import(fixtureUrl);prepareAccountPinForTest(true);
    const url=performance.getEntriesByType('resource').map(row=>row.name).find(url=>/\/src\/generated\/management-client\.ts\?t=/u.test(url))??"/src/generated/management-client.ts";
    const {ManagementApi}=await import(url);const original=ManagementApi.prototype.request;
    const now=Date.now();window.CPAR_A_CATALOG_AT=now;
    const rows=[
      {credential_id:"cred-relay-spare",freshness:"fresh",observation_state:"succeeded",model_count:2},
      {credential_id:"cred-relay-key",freshness:"fresh",observation_state:"empty",model_count:0},
      {credential_id:"catalog-unsupported",freshness:"missing",observation_state:"unsupported",model_count:null,last_success_at_ms:null,observed_at_ms:0},
      {credential_id:"catalog-failure",freshness:"stale",observation_state:"failed",model_count:1,last_failure_at_ms:now,last_failure_class:"transport"},
      {credential_id:"catalog-stale",freshness:"stale",observation_state:"succeeded",model_count:1},
    ].map(row=>({endpoint_id:"ep-relay-a-responses",source:"upstream_catalog",snapshot_version:3,observed_at_ms:now-900000,last_success_at_ms:now-900000,refresh_due:false,...row}));
    const reply=(status,body)=>new Response(JSON.stringify(body),{status,headers:{"Content-Type":"application/json"}});
    ManagementApi.prototype.request=async function(operation,request){
      if(operation==="getCatalogStatus")return reply(200,rows);
      if(operation==="listCatalogModels")return request.query?.cursor?reply(503,{error:{code:"management_runtime_unavailable",message:"synthetic later directory page unavailable"}}):reply(200,{config_version:request.headers["X-Config-Version"],revision:"rev-1",current_model_count:0,total_count:2,target:{endpoint_id:"ep-relay-a-responses",credential_id:"cred-relay-key",snapshot_version:3,observed_at_ms:now-900000,stale_at_ms:now+3600000,expires_at_ms:now+7200000},items:[{model:"retained-source-model",present_in_last_success:false}],next_cursor:"next-safe-page"});
      return original.call(this,operation,request);
    };
    location.hash="#/catalog";
  });
  await page.click('loc=css:summary:has-text("目录状态与诊断")');
  await page.waitForFunction(()=>document.querySelector('main')?.textContent.includes("成功，零模型"));
  for(const [id,state] of [["cred-relay-spare","succeeded"],["cred-relay-key","empty"],["catalog-unsupported","unsupported"],["catalog-failure","failed"],["catalog-stale","succeeded"]]){
    const index=await page.evaluate(id=>[...document.querySelectorAll("table.responsive-table tbody tr")].findIndex(row=>[...row.querySelectorAll("[data-resource-id]")].some(ref=>ref.getAttribute("data-resource-id")===id)),id);
    assert.ok(index>=0);
    await page.click(`loc=css:table.responsive-table tbody tr:nth-child(${index+1}) button`);
    const details=await page.evaluate(()=>document.querySelector('[role="dialog"]')?.textContent??"");
    assert.ok(details.includes("upstream_catalog")&&details.includes(state));
    assert.ok(details.includes("成功观测")&&details.includes("最近失败"));
    await page.click('loc=role:button[name="关闭"]');
  }
  await page.selectOption('loc=css:.upstream-model-browser select >> nth=1',"ep-relay-a-responses");
  await page.selectOption('loc=css:.upstream-model-browser select >> nth=2',"cred-relay-key");
  await page.waitForFunction(()=>document.querySelector('main')?.textContent.includes("旧模型仍在移除隔离期内"));
  await page.click('loc=role:button[name="加载更多模型"]');
  await page.waitForFunction(()=>document.querySelector('main')?.textContent.includes("后续目录页读取失败"));
  assert.match(await page.evaluate(()=>document.querySelector('.upstream-model-browser')?.textContent??""),/retained-source-model/u);
  console.log(JSON.stringify({scenario,result:"PASS",environment:"local-synthetic",spaceId}));
} else if(scenario==="catalog-mapping-conflict") {
  await page.evaluate(async()=>{
    const url=performance.getEntriesByType('resource').map(row=>row.name).find(url=>/\/src\/dev\/fixtures\.ts\?t=/u.test(url))??"/src/dev/fixtures.ts";
    window.CPAR_A_FIXTURE_URL=url;const {prepareModelMappingConflictForTest}=await import(url);prepareModelMappingConflictForTest();location.hash="#/catalog";
  });
  const snapshot=()=>page.evaluate(async()=>{const {modelMappingSnapshotForTest}=await import(window.CPAR_A_FIXTURE_URL);return modelMappingSnapshotForTest();});
  const before=await snapshot();
  await page.click('loc=role:link[name="手动批量接入"]');
  await page.waitForSelector('loc=role:dialog[name="接入模型"]');
  await page.fill('loc=css:[role="dialog"] textarea',"minimax-m3");
  await page.selectOption('loc=css:[role="dialog"] select',"ep-relay-second");
  await page.click('loc=css:#connect-model-form > fieldset:nth-of-type(2) input[type="checkbox"]');
  await page.click('loc=role:button[name="保存并应用"]');
  await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.textContent.includes("已有自定义模型映射"));
  assert.deepEqual(await snapshot(),before,"a mapping conflict cannot overwrite routes, accounts or permissions");
  await page.waitForFunction(()=>document.querySelector('[role="dialog"] textarea')?.disabled===false);
  await page.click('loc=role:button[name="取消"]');
  await page.waitForSelector('loc=role:button[name="放弃修改"]');
  await page.click('loc=role:button[name="放弃修改"]');
  await page.waitForSelector('loc=css:button:has-text("管理连接")');
  await page.click('loc=css:button:has-text("管理连接")');
  await page.click('loc=role:button[name="添加来源连接"]');
  await page.waitForSelector('loc=role:dialog[name="接入模型"]');
  assert.equal(await page.evaluate(()=>document.querySelector('[role="dialog"] textarea')?.value),"upstream-custom-model");
  await page.selectOption('loc=css:[role="dialog"] select',"ep-relay-second");
  await page.click('loc=css:#connect-model-form > fieldset:nth-of-type(2) input[type="checkbox"]');
  await page.click('loc=role:button[name="保存并应用"]');
  await page.waitForSelector('loc=role:dialog[name="模型接入结果"]');
  const after=await snapshot();
  assert.equal(after.models.length,1);assert.equal(after.candidates.length,2);
  assert.deepEqual(after.candidates[0],before.candidates[0]);
  assert.equal(after.candidates[1].upstream_model,"upstream-custom-model");
  assert.deepEqual(after.credentials,before.credentials);assert.deepEqual(after.permissions,before.permissions);
  console.log(JSON.stringify({scenario,result:"PASS",environment:"local-synthetic",spaceId}));
} else if(scenario.startsWith("native-connection-")) {
  const count={"native-connection-zero":0,"native-connection-one":1,"native-connection-multiple":2}[scenario];
  assert.notEqual(count,undefined);
  await page.evaluate(async count=>{
    const url=performance.getEntriesByType('resource').map(row=>row.name).find(url=>/\/src\/dev\/fixtures\.ts\?t=/u.test(url))??"/src/dev/fixtures.ts";
    window.CPAR_A_FIXTURE_URL=url;
    const {prepareNativeConnectionMatchesForTest}=await import(url);prepareNativeConnectionMatchesForTest(count);
  },count);
  const snapshot=()=>page.evaluate(async()=>{const {apiConnectionSnapshotForTest}=await import(window.CPAR_A_FIXTURE_URL);return apiConnectionSnapshotForTest();});
  await page.click('loc=css:#main-navigation a[href="#/oauth"]');
  await page.click('loc=role:button[name="授权 Grok Build"]');
  await page.click('loc=role:button[name="开始 Grok 授权"]');
  await page.waitForSelector('[data-native-session-id]');
  const session=await page.evaluate(()=>document.querySelector('[data-native-session-id]')?.getAttribute('data-native-session-id'));
  await page.evaluate(async id=>{const {approveNativeDeviceForTest}=await import(window.CPAR_A_FIXTURE_URL);approveNativeDeviceForTest(id);},session);
  await page.waitForSelector('loc=role:button[name="配置渠道接口"]');
  await page.click('loc=role:button[name="配置渠道接口"]');
  const expected=count===0?"没有匹配连接，将新建。":count===1?"将复用 Fixture Grok 1":"找到 2 个匹配连接，请选择。";
  await page.waitForFunction(expected=>document.querySelector('main')?.textContent.includes(expected),expected);
  const before=await snapshot();
  if(count===2){
    await page.click('loc=role:button[name="保存连接"]');
    assert.deepEqual(await snapshot(),before,"multiple native connections require an explicit choice");
    await page.selectOption('loc=css:select[required]',"batch-a-native-2-endpoint");
  }
  await page.click('loc=role:button[name="保存连接"]');
  await page.waitForSelector('loc=role:button[name="返回账号接入"]');
  const after=await snapshot();
  assert.equal(after.providers.length,count||1);assert.equal(after.endpoints.length,count||1);
  if(count>0){assert.deepEqual(after.providers,before.providers);assert.deepEqual(after.endpoints,before.endpoints);}
  console.log(JSON.stringify({scenario,result:"PASS",environment:"local-synthetic",spaceId}));
} else if(scenario.startsWith("api-")) {
  const count={"api-zero":0,"api-one":1,"api-multiple":2,"api-duplicate-response-lost":1}[scenario];
  assert.notEqual(count,undefined,`unknown API case: ${scenario}`);
  await page.evaluate(async count=>{
    const url=performance.getEntriesByType('resource').map(row=>row.name).find(url=>/\/src\/dev\/fixtures\.ts\?t=/u.test(url))??"/src/dev/fixtures.ts";
    window.CPAR_A_FIXTURE_URL=url;
    const {prepareApiConnectionMatchesForTest}=await import(url);
    prepareApiConnectionMatchesForTest(count);
  },count);
  await page.click('loc=css:#main-navigation a[href="#/upstreams"]');
  await page.click('loc=role:button[name="添加 AI 提供商"]');
  const expected=count===0?"没有匹配连接，将新建。":count===1?"将复用 Fixture API 1 · batch-a-api-1-endpoint":"找到 2 个匹配连接，请选择。";
  await page.waitForFunction(expected=>document.querySelector('main')?.textContent.includes(expected),expected);
  await page.fill('input[placeholder="例如：我的 OpenAI"]',`Fixture API acceptance ${scenario}`);
  await page.fill('textarea[autocomplete="off"]',"synthetic-local-key");
  if(count===2){
    const before=await page.evaluate(async()=>{const {apiConnectionSnapshotForTest}=await import(window.CPAR_A_FIXTURE_URL);return apiConnectionSnapshotForTest();});
    await page.click('loc=role:button[name="保存并接入"]');
    assert.deepEqual(await page.evaluate(async()=>{const {apiConnectionSnapshotForTest}=await import(window.CPAR_A_FIXTURE_URL);return apiConnectionSnapshotForTest();}),before,"multiple matches require an explicit target");
    await page.selectOption('loc=css:select[required]',"batch-a-api-2-endpoint");
  }
  await page.click('loc=role:button[name="保存并接入"]');
  await page.waitForFunction(()=>document.querySelector('main')?.textContent.includes("提供商创建结果"));
  const snapshot=await page.evaluate(async()=>{const {apiConnectionSnapshotForTest}=await import(window.CPAR_A_FIXTURE_URL);return apiConnectionSnapshotForTest();});
  assert.equal(snapshot.providers.length,count||1);
  assert.equal(snapshot.endpoints.length,count||1);
  assert.equal(snapshot.accounts.length,1);
  assert.equal(snapshot.bindings.length,1);
  if(count>0){assert.equal(snapshot.providers[0].enabled,false,"existing disabled provider stays disabled");assert.equal(snapshot.endpoints[0].enabled,false,"existing disabled endpoint stays disabled");}
  if(count===2)assert.equal(snapshot.bindings[0].endpoint_id,"batch-a-api-2-endpoint","explicit selection owns the new account binding");
  if(scenario==="api-duplicate-response-lost"){
    const savedId=snapshot.accounts[0].id;
    await page.click('loc=role:button[name="查看工作草稿"]');
    await page.waitForSelector('loc=role:button[name="添加 AI 提供商"]');
    await page.evaluate(async()=>{
      const url=performance.getEntriesByType('resource').map(row=>row.name).find(url=>/\/src\/generated\/management-client\.ts\?t=/u.test(url))??"/src/generated/management-client.ts";
      const {ManagementApi}=await import(url);const original=ManagementApi.prototype.request;
      window.CPAR_A_IMPORTS=0;window.CPAR_A_IMPORT_RECEIPTS=0;
      ManagementApi.prototype.request=async function(operation,request){
        if(operation==="importChannelAccount")window.CPAR_A_IMPORTS+=1;
        if(operation==="getAccountImportReceipt")window.CPAR_A_IMPORT_RECEIPTS+=1;
        const response=await original.call(this,operation,request);
        if(operation==="importChannelAccount")throw new Error("synthetic lost duplicate API import response");
        return response;
      };
    });
    await page.click('loc=role:button[name="添加 AI 提供商"]');
    await page.waitForFunction(()=>document.querySelector('main')?.textContent.includes("将复用 Fixture API 1"));
    await page.fill('input[placeholder="例如：我的 OpenAI"]',"Recovered API");
    await page.fill('textarea[autocomplete="off"]',"synthetic-local-key");
    await page.click('loc=role:button[name="保存并接入"]');
    await page.waitForSelector('loc=role:button[name="只读取 API 授权结果"]');
    await page.click('loc=role:button[name="只读取 API 授权结果"]');
    await page.waitForFunction(id=>document.querySelector('main')?.textContent.includes(`API 授权已保存（${id}）`),savedId);
    assert.equal(await page.evaluate(()=>window.CPAR_A_IMPORTS),1);
    assert.equal(await page.evaluate(()=>window.CPAR_A_IMPORT_RECEIPTS),1);
    await page.click('loc=role:button[name="核对合并清单"]');
    await page.click('loc=css:section[aria-label="本次配置核对"] input[type="checkbox"]');
    await page.click('loc=role:button[name="按这份清单继续连接"]');
    await page.click('loc=role:button[name="核对合并清单"]');
    await page.click('loc=css:section[aria-label="本次配置核对"] input[type="checkbox"]');
    await page.click('loc=role:button[name="校验并应用这份清单"]');
    await page.waitForFunction(()=>document.querySelector('main')?.textContent.includes("接入修改已应用"));
    const after=await page.evaluate(async()=>{const {apiConnectionSnapshotForTest}=await import(window.CPAR_A_FIXTURE_URL);return apiConnectionSnapshotForTest();});
    assert.equal(after.accounts.length,1);assert.equal(after.accounts[0].id,savedId);assert.equal(after.bindings.length,1);
  }
  console.log(JSON.stringify({scenario,result:"PASS",environment:"local-synthetic",spaceId}));
} else if(["oauth-codex-binding-failed","oauth-claude-binding-failed","device-kimi-binding-failed","device-kiro-binding-failed"].includes(scenario)) {
  const channel=scenario.includes("codex")?"codex":scenario.includes("claude")?"claude":scenario.includes("kimi")?"kimi":"kiro";
  const label={codex:"Codex / ChatGPT",claude:"Claude",kimi:"Kimi Coding",kiro:"Kiro"}[channel];
  await page.evaluate(async()=>{
    const url=performance.getEntriesByType('resource').map(row=>row.name).find(url=>/\/src\/generated\/management-client\.ts\?t=/u.test(url))??"/src/generated/management-client.ts";
    const {ManagementApi}=await import(url);const original=ManagementApi.prototype.request;
    window.CPAR_A_CONSUMING=0;window.CPAR_A_BINDINGS=0;window.CPAR_A_PUBLISHES=0;
    ManagementApi.prototype.request=async function(operation,request){
      if(["completeCodexEnrollment","completeClaudeEnrollment","pollKimiEnrollment","pollKiroEnrollment"].includes(operation))window.CPAR_A_CONSUMING+=1;
      if(operation==="createEndpointCredentialBinding"&&++window.CPAR_A_BINDINGS===1)return new Response(JSON.stringify({error:{code:"management_revision_conflict",message:"synthetic first binding rejected"}}),{status:409,headers:{"Content-Type":"application/json"}});
      if(operation==="publishConfigVersion")window.CPAR_A_PUBLISHES+=1;
      return original.call(this,operation,request);
    };
  });
  await page.click('loc=css:#main-navigation a[href="#/oauth"]');
  await page.click(`loc=role:button[name="授权 ${label}"]`);
  await page.click('loc=role:button[name="开始授权"]');
  if(channel==="codex"||channel==="claude"){
    await page.waitForSelector('textarea[aria-label="回调地址"]');
    const href=await page.evaluate(()=>document.querySelector('[role="dialog"] a')?.href);
    const state=new URL(href).searchParams.get("state");
    await page.fill('textarea[aria-label="回调地址"]',`http://127.0.0.1:1455/auth/callback?code=synthetic-local&state=${state}`);
    await page.click('loc=role:button[name="完成授权"]');
  }
  await page.waitForSelector('loc=role:button[name="核对合并清单"]');
  const consumed=await page.evaluate(()=>window.CPAR_A_CONSUMING);
  assert.ok(consumed>=1);
  assert.equal(await page.evaluate(()=>window.CPAR_A_PUBLISHES),0);
  assert.ok(!(await page.evaluate(()=>document.querySelector('[role="dialog"]')?.textContent??"")).includes("已保存并连接接口"));
  await page.click('loc=role:button[name="核对合并清单"]');
  await page.click('loc=css:section[aria-label="本次配置核对"] input[type="checkbox"]');
  await page.click('loc=role:button[name="按这份清单继续连接"]');
  await page.waitForFunction(()=>window.CPAR_A_BINDINGS===2);
  assert.equal(await page.evaluate(()=>window.CPAR_A_PUBLISHES),0,"new binding still requires publication review");
  await page.click('loc=role:button[name="核对合并清单"]');
  await page.click('loc=css:section[aria-label="本次配置核对"] input[type="checkbox"]');
  await page.click('loc=role:button[name="校验并应用这份清单"]');
  await page.waitForFunction(()=>window.CPAR_A_PUBLISHES===1);
  assert.equal(await page.evaluate(()=>window.CPAR_A_CONSUMING),consumed,"review may not consume authorization again");
  console.log(JSON.stringify({scenario,result:"PASS",environment:"local-synthetic",spaceId}));
} else if(["oauth-codex","oauth-claude","oauth-response-lost"].includes(scenario)) {
  const label=scenario==="oauth-claude"?"Claude":"Codex / ChatGPT";
  await page.evaluate(async scenario=>{
    const url=performance.getEntriesByType('resource').map(row=>row.name).find(url=>/\/src\/generated\/management-client\.ts\?t=/u.test(url))??"/src/generated/management-client.ts";
    const {ManagementApi}=await import(url);
    const original=ManagementApi.prototype.request;
    window.CPAR_A_CALLBACKS=0;
    ManagementApi.prototype.request=async function(operation,request){
      const callback=operation==="completeCodexEnrollment"||operation==="completeClaudeEnrollment";
      if(callback)window.CPAR_A_CALLBACKS+=1;
      const response=await original.call(this,operation,request);
      if(callback&&scenario==="oauth-response-lost")throw new Error("synthetic lost OAuth completion response");
      return response;
    };
  },scenario);
  await page.click('loc=css:#main-navigation a[href="#/oauth"]');
  await page.waitForSelector(`loc=role:button[name="授权 ${label}"]`);
  await page.click(`loc=role:button[name="授权 ${label}"]`);
  await page.click('loc=role:button[name="开始授权"]');
  await page.waitForSelector('textarea[aria-label="回调地址"]');
  const href=await page.evaluate(()=>document.querySelector('[role="dialog"] a')?.href);
  const state=new URL(href).searchParams.get("state");
  assert.ok(state);
  await page.fill('textarea[aria-label="回调地址"]',`http://127.0.0.1:1455/auth/callback?code=synthetic-local&state=${state}`);
  await page.click('loc=role:button[name="完成授权"]');
  if(scenario==="oauth-response-lost"){
    await page.waitForSelector('loc=role:button[name="核对授权结果"]');
    await page.click('loc=role:button[name="核对授权结果"]');
    await page.waitForSelector('loc=role:button[name="继续连接与应用"]');
    await page.click('loc=role:button[name="继续连接与应用"]');
  }
  await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.textContent?.includes("账号授权已保存并连接接口"));
  assert.equal(await page.evaluate(()=>window.CPAR_A_CALLBACKS),1,"recovery may not replay a consumed callback");
  console.log(JSON.stringify({scenario,result:"PASS",environment:"local-synthetic",spaceId}));
} else if(["device-kimi-unknown","device-kiro-unknown"].includes(scenario)) {
  const channel=scenario==="device-kimi-unknown"?"kimi-coding":"kiro";
  const label=channel==="kiro"?"Kiro":"Kimi Coding";
  await page.evaluate(async channel=>{
    const url=performance.getEntriesByType('resource').map(row=>row.name).find(url=>/\/src\/dev\/fixtures\.ts\?t=/u.test(url))??"/src/dev/fixtures.ts";
    const {failNextDevicePollForTest}=await import(url);
    failNextDevicePollForTest(channel);
  },channel);
  await page.click('loc=css:#main-navigation a[href="#/oauth"]');
  await page.waitForSelector(`loc=role:button[name="授权 ${label}"]`);
  await page.click(`loc=role:button[name="授权 ${label}"]`);
  await page.click('loc=role:button[name="开始授权"]');
  await page.waitForSelector('loc=role:button[name="核对授权结果"]');
  await page.click('loc=role:button[name="核对授权结果"]');
  await page.waitForFunction(()=>document.querySelector('[aria-label="授权结果恢复"]')?.textContent?.includes("授权结果待确认"));
  assert.match(await page.evaluate(()=>document.querySelector('[role="dialog"]')?.textContent??""),/不会重复提交授权请求/u);
  console.log(JSON.stringify({scenario,result:"PASS",environment:"local-synthetic",spaceId}));
} else if(scenario==="native-build-apply-failed") {
  await page.evaluate(async()=>{
    const url=performance.getEntriesByType('resource').map(row=>row.name).find(url=>/\/src\/dev\/fixtures\.ts\?t=/u.test(url))??"/src/dev/fixtures.ts";
    window.CPAR_A_FIXTURE_URL=url;
    const {failNextNativeRuntimeApplyForTest}=await import(url);
    failNextNativeRuntimeApplyForTest();
  });
  await page.click('loc=css:#main-navigation a[href="#/oauth"]');
  await page.waitForSelector('loc=role:button[name="授权 Grok Build"]');
  await page.click('loc=role:button[name="授权 Grok Build"]');
  await page.click('loc=role:button[name="开始 Grok 授权"]');
  await page.waitForSelector('[data-native-session-id]');
  const id=await page.evaluate(()=>document.querySelector('[data-native-session-id]')?.getAttribute('data-native-session-id'));
  assert.ok(id);
  await page.evaluate(async id=>{const {approveNativeDeviceForTest}=await import(window.CPAR_A_FIXTURE_URL);approveNativeDeviceForTest(id);},id);
  await page.waitForSelector('loc=role:button[name="应用运行配置"]');
  assert.match(await page.evaluate(()=>document.querySelector('[role="dialog"]')?.textContent??""),/授权已保存.*运行配置暂未应用/u);
  assert.match(await page.evaluate(()=>document.querySelector('[role="dialog"]')?.textContent??""),/新请求被全局阻断/u);
  await page.click('loc=role:button[name="应用运行配置"]');
  await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.textContent?.includes("授权已载入运行配置"));
  console.log(JSON.stringify({scenario,result:"PASS",environment:"local-synthetic",spaceId}));
} else if(["pin-default-closed","pin-explicit-success"].includes(scenario)) {
  const allowed=scenario==="pin-explicit-success";
  await page.evaluate(async allowed=>{
    const url=performance.getEntriesByType('resource').map(row=>row.name).find(url=>/\/src\/dev\/fixtures\.ts\?t=/u.test(url))??"/src/dev/fixtures.ts";
    window.CPAR_A_FIXTURE_URL=url;
    const {prepareAccountPinForTest,trackFixtureOperationForTest}=await import(url);
    prepareAccountPinForTest(allowed);
    trackFixtureOperationForTest("POST /admin/operations/channel-pin");
  },allowed);
  await page.click('loc=css:#main-navigation a[href="#/upstreams"]');
  await page.waitForSelector('loc=role:button[name="模型"]');
  await page.click('loc=role:button[name="模型"]');
  const calls=()=>page.evaluate(async()=>{const {fixtureOperationCallsForTest}=await import(window.CPAR_A_FIXTURE_URL);return fixtureOperationCallsForTest("POST /admin/operations/channel-pin");});
  assert.equal(await calls(),0,"opening the account model panel must not call Provider");
  await page.selectOption('loc=css:[role="dialog"] select >> nth=0',"key-cli");
  await page.selectOption('loc=css:[role="dialog"] select >> nth=1',"pm-minimax");
  await page.selectOption('loc=css:[role="dialog"] select >> nth=2',"ep-relay-a-responses");
  assert.equal(await calls(),0,"selection must not call Provider");
  await page.click('loc=css:[role="dialog"] .check-row input[type="checkbox"]');
  await page.click('loc=role:button[name="执行一次调用测试"]');
  await page.waitForFunction(allowed=>document.querySelector('[role="dialog"]')?.textContent?.includes(allowed?"该身份下的调用观测":"当前 Key 无权调用所选模型"),allowed);
  assert.equal(await calls(),1,"one explicit click makes one Pin request");
  if(allowed){
    const receipt=await page.evaluate(()=>document.querySelector('[role="dialog"]')?.textContent??"");
    assert.match(receipt,/cred-relay-key.*ep-relay-a-responses.*pm-minimax|cred-relay-key.*ep-relay-a-responses/u);
    await page.selectOption('loc=css:[role="dialog"] select >> nth=3',"anthropic_messages");
    assert.doesNotMatch(await page.evaluate(()=>document.querySelector('[role="dialog"]')?.textContent??""),/该身份下的调用观测/u,"changed protocol invalidates the receipt");
    await page.click('loc=role:button[name="关闭"]');
    await page.waitForFunction(()=>document.querySelector('tr[data-account-key="cred-relay-key"]')?.textContent?.includes("最近调用通过"));
    await page.click('loc=role:button[name="模型"]');
    await page.waitForSelector('[aria-label="最近一次明确调用"]');
    assert.match(await page.evaluate(()=>document.querySelector('[aria-label="最近一次明确调用"]')?.textContent??""),/minimax-m3.*openai_responses/u);
    assert.equal(await calls(),1,"closing and reopening retained evidence must not repeat inference");
  }
  console.log(JSON.stringify({scenario,result:"PASS",environment:"local-synthetic",spaceId}));
} else if(scenario==="native-mixed-batch") {
  await page.evaluate(async()=>{
    const url=performance.getEntriesByType('resource').map(row=>row.name).find(url=>/\/src\/dev\/fixtures\.ts\?t=/u.test(url))??"/src/dev/fixtures.ts";
    window.CPAR_A_FIXTURE_URL=url;
    const {prepareNativeBatchForTest}=await import(url);await prepareNativeBatchForTest();
    const client=performance.getEntriesByType('resource').map(row=>row.name).find(url=>/\/src\/generated\/management-client\.ts\?t=/u.test(url))??"/src/generated/management-client.ts";
    const {ManagementApi}=await import(client);const original=ManagementApi.prototype.request;
    window.CPAR_A_NATIVE_IMPORTS=0;
    ManagementApi.prototype.request=async function(operation,request){
      if(operation!=="importNativeAccount")return original.call(this,operation,request);
      window.CPAR_A_NATIVE_IMPORTS+=1;
      const material=request.body?.secret;
      if(material==="synthetic-invalid"||material==="synthetic-conflict")return new Response(JSON.stringify({error:{code:material==="synthetic-invalid"?"invalid_management_request":"management_native_account_conflict",message:"synthetic batch rejection"}}),{status:material==="synthetic-invalid"?400:409,headers:{"Content-Type":"application/json"}});
      const response=await original.call(this,operation,request);
      if(material==="synthetic-lost-new-auth")throw new Error("synthetic lost duplicate update response");
      return response;
    };
  });
  await page.click('loc=css:#main-navigation a[href="#/accounts"]');
  await page.click('loc=role:button[name="导入账号文件"]');
  await page.click('loc=role:button[name*="Grok Console"]');
  await page.click('loc=role:button[name="选择文件"]');
  await page.evaluate(()=>{
    const data=new DataTransfer();
    for(const [label,material] of [["new.txt","synthetic-another-user"],["duplicate.txt","synthetic-existing"],["invalid.txt","synthetic-invalid"],["lost.txt","synthetic-lost-new-auth"],["conflict.txt","synthetic-conflict"]])data.items.add(new File([material],label,{type:"text/plain"}));
    const input=document.querySelector('[role="dialog"] input[type="file"]');input.files=data.files;input.dispatchEvent(new Event("change",{bubbles:true}));
  });
  await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.textContent?.includes("conflict.txt"));
  await page.click('loc=role:button[name="导入账号"]');
  await page.waitForSelector('loc=role:dialog[name="导入结果"]');
  const initial=await page.evaluate(()=>[...document.querySelectorAll('[role="dialog"] tbody tr')].map(row=>row.innerText));
  assert.match(initial[0],/已添加/u);assert.match(initial[1],/已存在/u);assert.match(initial[2],/未添加/u);assert.match(initial[3],/结果未确认/u);assert.match(initial[4],/未执行/u);
  assert.equal(await page.evaluate(()=>window.CPAR_A_NATIVE_IMPORTS),4);
  await page.click('loc=role:button[name="核对导入结果"]');
  await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.textContent?.includes("已回读账号，运行应用待核对"));
  assert.equal(await page.evaluate(()=>window.CPAR_A_NATIVE_IMPORTS),4,"receipt recovery must not repeat imports");
  await page.click('loc=role:button[name="应用运行配置"]');
  await page.waitForFunction(()=>document.querySelector('input[aria-label="重试 conflict.txt"]')?.disabled===false);
  await page.evaluate(()=>{const data=new DataTransfer();data.items.add(new File(["synthetic-conflict"],"conflict.txt",{type:"text/plain"}));const input=document.querySelector('input[aria-label="重试 conflict.txt"]');input.files=data.files;input.dispatchEvent(new Event("change",{bubbles:true}));});
  await page.waitForFunction(()=>window.CPAR_A_NATIVE_IMPORTS===5);
  await page.waitForFunction(()=>[...document.querySelectorAll('[role="dialog"] tbody tr')].at(-1)?.textContent?.includes("未添加"));
  const snapshot=await page.evaluate(async()=>{const {nativeBatchSnapshotForTest}=await import(window.CPAR_A_FIXTURE_URL);return nativeBatchSnapshotForTest();});
  assert.equal(snapshot.length,2);assert.equal(snapshot.find(row=>row.id==="native-existing").enabled,false);assert.equal(snapshot.find(row=>row.id==="native-existing").import_batch_id,"original-batch");
  console.log(JSON.stringify({scenario,result:"PASS",environment:"local-synthetic",spaceId}));
} else if(scenario==="delete-shared-account") {
  await page.evaluate(async()=>{
    const url=performance.getEntriesByType('resource').map(row=>row.name).find(url=>/\/src\/dev\/fixtures\.ts\?t=/u.test(url))??"/src/dev/fixtures.ts";
    window.CPAR_A_FIXTURE_URL=url;
    const {prepareSharedDeletionForTest}=await import(url);
    prepareSharedDeletionForTest();
  });
  await page.click('loc=css:#main-navigation a[href="#/upstreams"]');
  await page.waitForSelector('tr[data-account-key="cred-relay-key"]');
  await page.click('loc=css:tr[data-account-key="cred-relay-key"] .account-list-actions button:has-text("删除")');
  await page.waitForFunction(()=>document.querySelector('[aria-label="删除影响预览"]')?.textContent?.includes("cred-relay-spare"));
  const impact=await page.evaluate(()=>document.querySelector('[aria-label="删除影响预览"]')?.textContent??"");
  assert.match(impact,/保留共享接口 ep-relay-a-responses/u);
  assert.match(impact,/保留路由：rt-minimax/u);
  assert.match(impact,/保留客户端 Key：key-cli/u);
  assert.equal(await page.evaluate(()=>[...document.querySelectorAll('[role="dialog"] button')].find(x=>x.textContent?.includes("确认删除"))?.disabled),true);
  await page.click('loc=css:[aria-label="删除影响预览"] input[type="checkbox"]');
  await page.click('loc=role:button[name="确认删除"]');
  await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.textContent?.includes("处理结果"));
  const result=await page.evaluate(async()=>{const {apiConnectionSnapshotForTest}=await import(window.CPAR_A_FIXTURE_URL);return apiConnectionSnapshotForTest();});
  assert.deepEqual(result.accounts.map(row=>row.id),["cred-relay-spare"]);
  assert.equal(result.endpoints.length,1);
  assert.equal(result.bindings.length,1);
  assert.equal(result.bindings[0].credential_id,"cred-relay-spare");
  await page.click('loc=role:button[name="完成"]');
  await page.click('loc=css:#main-navigation a[href="#/settings"]');
  await page.click('loc=css:[aria-label="工作区页面"] a[href="#/versions"]');
  await page.click('loc=role:button[name="回滚到上一版本"]');
  await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.textContent?.includes("将恢复已删除的本地账号"));
  const restoration=await page.evaluate(()=>document.querySelector('[role="dialog"]')?.textContent??"");
  assert.match(restoration,/cred-relay-key/u);
  assert.match(restoration,/恢复的是历史版本中的授权与连接/u);
  assert.equal(await page.evaluate(()=>[...document.querySelectorAll('[role="dialog"] button')].find(x=>x.textContent?.includes("确认回滚"))?.disabled),true);
  await page.click('loc=css:[role="dialog"] input[type="checkbox"]');
  await page.click('loc=role:button[name="确认回滚"]');
  await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.textContent?.includes("配置回滚已确认"));
  const restored=await page.evaluate(async()=>{const {apiConnectionSnapshotForTest}=await import(window.CPAR_A_FIXTURE_URL);return apiConnectionSnapshotForTest();});
  assert.deepEqual(restored.accounts.map(row=>row.id).sort(),["cred-relay-key","cred-relay-spare"]);
  assert.equal(restored.endpoints.length,1);
  assert.equal(restored.bindings.length,2);
  console.log(JSON.stringify({scenario,result:"PASS",environment:"local-synthetic",spaceId}));
} else {
await page.click('loc=css:#main-navigation a[href="#/accounts"]');
await page.waitForSelector('loc=role:button[name="导入账号文件"]');

if(scenario.startsWith("merge-")) {
  await page.click('loc=css:#main-navigation a[href="#/settings"]');
  await page.click('loc=css:[aria-label="工作区页面"] a[href="#/versions"]');
  await page.click('loc=css:[data-version-id="draft-2026-08"] button:has-text("编辑草稿")');
  await page.click('loc=role:button[name="接续草稿"]');
  await page.click('loc=css:#main-navigation a[href="#/accounts"]');
}

if(["import-all-failed","import-connection-failed","import-publication-unknown","import-response-lost","merge-review","merge-conflict"].includes(scenario)) {
  await page.evaluate(async scenario=>{
    const url=performance.getEntriesByType('resource').map(row=>row.name).find(url=>/\/src\/generated\/management-client\.ts\?t=/u.test(url))??"/src/generated/management-client.ts";
    const {ManagementApi}=await import(url);
    const original=ManagementApi.prototype.request;
    ManagementApi.prototype.request=async function(operation,request) {
      if(operation==="importChannelAccount")window.CPAR_A_IMPORTS=(window.CPAR_A_IMPORTS??0)+1;
      if(operation==="publishConfigVersion") {
        window.CPAR_A_PUBLICATIONS=(window.CPAR_A_PUBLICATIONS??0)+1;
        if(scenario==="merge-conflict"&&window.CPAR_A_PUBLICATIONS===1)return new Response(JSON.stringify({error:{code:"configuration_revision_conflict",message:"synthetic concurrent change"}}),{status:409,headers:{"Content-Type":"application/json"}});
      }
      if(scenario==="import-all-failed"&&operation==="importChannelAccount")return new Response(JSON.stringify({error:{code:"fixture_import_rejected",message:"synthetic rejection"}}),{status:400,headers:{"Content-Type":"application/json"}});
      if(scenario==="import-connection-failed"&&operation==="createEndpointCredentialBinding")return new Response(JSON.stringify({error:{code:"fixture_binding_unavailable",message:"synthetic binding failure"}}),{status:503,headers:{"Content-Type":"application/json"}});
      const response=await original.call(this,operation,request);
      if(scenario==="import-publication-unknown"&&operation==="publishConfigVersion")throw new Error("synthetic lost publication response");
      if(scenario==="import-response-lost"&&operation==="importChannelAccount")throw new Error("synthetic lost import response");
      return response;
    };
  },scenario);
  await page.click('loc=role:button[name="导入账号文件"]');
  await page.click('loc=role:button[name*="Codex / ChatGPT"]');
  await page.fill('textarea[name="secret"]',"synthetic-rejected-material");
  await page.click('loc=role:button[name="导入账号"]');
  await page.waitForSelector('loc=role:dialog[name="导入结果"]');
  await page.waitForFunction(scenario=>document.querySelector('[role="dialog"]')?.textContent.includes(scenario==="import-all-failed"?"未添加":scenario==="import-connection-failed"?"已保存，连接未完成":scenario==="import-response-lost"?"结果未确认":"已添加"),scenario);
  if(scenario==="import-response-lost") {
    await page.click('loc=role:button[name="核对导入结果"]');
    await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.textContent.includes("已保存，连接与应用待核对"));
    assert.equal(await page.evaluate(()=>window.CPAR_A_IMPORTS),1,"readback must not replay the import");
  }
  if(scenario.startsWith("merge-")) {
    assert.equal(await page.evaluate(()=>window.CPAR_A_PUBLICATIONS??0),0,"existing changes cannot be silently published");
    await page.click('loc=role:button[name="核对合并清单"]');
    await page.waitForFunction(()=>document.querySelector('[aria-label="本次配置核对"]')?.textContent.includes("将应用的完整清单"));
    await page.click('loc=css:[aria-label="本次配置核对"] input[type="checkbox"]');
    await page.click('loc=role:button[name="校验并应用这份清单"]');
    if(scenario==="merge-conflict") {
      await page.waitForFunction(()=>document.querySelector('[aria-label="本次配置核对"]')?.textContent.includes("synthetic concurrent change"));
      assert.equal(await page.evaluate(()=>document.querySelector('[aria-label="本次配置核对"] input[type="checkbox"]')?.checked??false),false);
      assert.equal(await page.evaluate(()=>window.CPAR_A_PUBLICATIONS),1);
    } else await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.textContent.includes("配置已生效"));
    console.log(JSON.stringify({scenario,result:"PASS",environment:"local-synthetic",spaceId}));
    await page.click('loc=role:button[name="完成"]');
    process.exitCode=0;
  } else {
  await page.click('loc=role:button[name="完成"]');
  const notice=await page.evaluate(()=>document.querySelector("main")?.textContent??"");
  if(scenario==="import-all-failed"){assert.match(notice,/本次未保存账号/u);assert.doesNotMatch(notice,/已确认保存 1 个账号/u,"a prepared draft is not a saved account receipt");}
  else assert.match(notice,/已确认保存 1 个账号/u,"binding or publication failure must preserve the save receipt");
  assert.doesNotMatch(notice,/synthetic-rejected-material/u);
  console.log(JSON.stringify({scenario,result:"PASS",environment:"local-synthetic",spaceId}));
  }
} else throw new Error(`Unknown scenario: ${scenario}`);
}
