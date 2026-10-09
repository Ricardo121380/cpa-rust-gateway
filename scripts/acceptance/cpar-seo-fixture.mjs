import assert from "node:assert/strict";
import {writeFile} from "node:fs/promises";
const {spaceId,origin,out,taskSpace,deviceOnly,channels=["kiro","kimi-coding"]}=globalThis.CPAR_SEO;
assert.match(origin,/^http:\/\/127\.0\.0\.1:\d+$/u);
const task=await taskSpace(spaceId),page=task.page("p1"),results=[];
async function record(row){results.push(row);await writeFile(out,JSON.stringify({environment:"development fixture only",spaceId,results},null,2)+"\n");console.log(JSON.stringify(row));}
async function login(){
  await page.goto(`${origin}/#/unlock`);await page.reload();
  await page.fill('input[name="password"]',"Prism-demo-2026");await page.click('loc=role:button[name="登录"]');
  await page.waitForSelector("main.canvas");
  await page.evaluate(async()=>{
    const moduleUrl=path=>performance.getEntriesByType("resource").map(row=>row.name).find(name=>new URL(name).pathname===path)??path;
    window.CPAR_SEO_URLS={fixture:moduleUrl("/src/dev/fixtures.ts"),api:moduleUrl("/src/generated/management-client.ts"),versions:moduleUrl("/src/features/config-versions/versionStore.ts")};
    window.CPAR_SEO_RECORDS=[];window.CPAR_SEO_SESSION=undefined;
    const {ManagementApi}=await import(window.CPAR_SEO_URLS.api),original=ManagementApi.prototype.request;
    ManagementApi.prototype.request=async function(operation,request){
      const body=request.body??{};
      const row={operation,path:request.path,body:Object.fromEntries(Object.entries(body).filter(([key])=>["id","kind","adapter_id","api_format","base_url","inference_path","transport","channel","credential_id","session_id","endpoint_id","enabled"].includes(key)))};
      window.CPAR_SEO_RECORDS.push(row);
      const response=await original.call(this,operation,request);
      if(operation==="startKiroEnrollment"||operation==="startKimiEnrollment")window.CPAR_SEO_SESSION=(await response.clone().json()).session_id;
      if(operation==="getAccountAuthorizationReceipt")row.receipt=await response.clone().json();
      return response;
    };
  });
}
for(const presetId of deviceOnly?[]:["responses","chat","messages","kimi","grok-api"]){
  for(const count of [0,1,2]){
    await login();
    const before=await page.evaluate(async({presetId,count})=>{
      const fixture=await import(window.CPAR_SEO_URLS.fixture);fixture.prepareApiConnectionMatchesForTest(count,presetId);
      const presets=await import("/src/features/upstreams/connectionPresets.ts");
      return {snapshot:fixture.apiConnectionSnapshotForTest(),preset:presets.CONNECTION_PRESETS.find(row=>row.id===presetId)};
    },{presetId,count});
    await page.goto(`${origin}/#/upstreams`);await page.click('loc=role:button[name="添加 AI 提供商"]');
    await page.selectOption('form#provider-setup-form select:not([required])',presetId);
    await page.waitForFunction(count=>document.querySelector("main")?.textContent.includes(count===0?"没有匹配连接，将新建。":count===1?"将复用 Fixture API 1":"找到 2 个匹配连接"),count);
    await page.fill('input[placeholder="例如：我的 OpenAI"]',`Synthetic ${presetId} ${count}`);
    await page.fill('textarea[autocomplete="off"]',"synthetic-local-api-key");
    if(count===2){
      await page.click('loc=role:button[name="保存并接入"]');
      assert.equal(await page.evaluate(()=>window.CPAR_SEO_RECORDS.filter(row=>row.operation==="importChannelAccount").length),0);
      await page.selectOption('form#provider-setup-form select[required]',"batch-a-api-2-endpoint");
    }
    await page.click('loc=role:button[name="保存并接入"]');
    await page.waitForFunction(()=>document.querySelector("main")?.textContent.includes("接入修改已应用"));
    const observed=await page.evaluate(async()=>{
      const fixture=await import(window.CPAR_SEO_URLS.fixture);
      return {snapshot:fixture.apiConnectionSnapshotForTest(),records:window.CPAR_SEO_RECORDS,text:document.querySelector("main")?.textContent};
    });
    const {providers,endpoints,accounts,bindings}=observed.snapshot;
    assert.equal(providers.length,count||1);assert.equal(endpoints.length,count||1);
    assert.equal(accounts.length,1);assert.equal(bindings.length,1);
    const endpoint=endpoints.find(row=>row.id===bindings[0].endpoint_id),owner=providers.find(row=>row.id===endpoint.upstream_id);
    assert.equal(endpoint.adapter_id,before.preset.adapter);assert.equal(endpoint.api_format,before.preset.format);
    assert.equal(endpoint.inference_path,before.preset.path);assert.equal(owner.kind,before.preset.kind);
    const imports=observed.records.filter(row=>row.operation==="importChannelAccount");
    assert.equal(imports.length,1);assert.equal(imports[0].body.channel,before.preset.channel);assert.equal(imports[0].path.upstream_id,owner.id);
    assert.equal(observed.records.filter(row=>row.operation==="publishConfigVersion").length,1);
    if(count){assert.deepEqual(providers,before.snapshot.providers);assert.deepEqual(endpoints,before.snapshot.endpoints);}
    if(count===2)assert.equal(endpoint.id,"batch-a-api-2-endpoint");
    await record({scenario:`E-01-${presetId}-${count}`,result:"PASS",preset:before.preset,actual:observed});
  }
}
for(const channel of channels){
  await login();
  await page.evaluate(async channel=>{const fixture=await import(window.CPAR_SEO_URLS.fixture);fixture.loseNextDeviceCompletionResponseForTest(channel);},channel);
  await page.goto(`${origin}/#/oauth`);
  const label=channel==="kiro"?"Kiro":"Kimi Coding";
  await page.click(`loc=role:button[name="授权 ${label}"]`);await page.click('loc=role:button[name="开始授权"]');
  await page.waitForSelector('loc=role:button[name="关闭并标记结果未确认"]');
  const saved=await page.evaluate(async()=>{
    const {deviceCompletionSnapshotForTest}=await import(window.CPAR_SEO_URLS.fixture),saved=deviceCompletionSnapshotForTest(window.CPAR_SEO_SESSION);
    delete saved.receipt.actor;
    return {sessionId:window.CPAR_SEO_SESSION,saved,records:window.CPAR_SEO_RECORDS};
  });
  assert.equal(saved.saved.receipt.state,"completed");assert.equal(saved.saved.exchanges,1);assert.equal(saved.saved.accounts.length,1);
  await page.click('loc=role:button[name="关闭并标记结果未确认"]');
  await page.waitForFunction(()=>!document.querySelector('[role="dialog"]'));
  await page.goto(`${origin}/#/accounts`);await page.click(`loc=role:button[name="返回 ${label} 授权任务"]`);
  await page.click('loc=role:button[name="核对授权结果"]');
  await page.waitForFunction(()=>window.CPAR_SEO_RECORDS.some(row=>row.operation==="getAccountAuthorizationReceipt"&&row.receipt?.state==="completed")&&["账号已保存","授权已保存"].some(text=>document.querySelector('[role="dialog"]')?.textContent.includes(text)));
  const recovered=await page.evaluate(async()=>{
    const {deviceCompletionSnapshotForTest}=await import(window.CPAR_SEO_URLS.fixture),saved=deviceCompletionSnapshotForTest(window.CPAR_SEO_SESSION);
    delete saved.receipt.actor;
    return {sessionId:window.CPAR_SEO_SESSION,saved,records:window.CPAR_SEO_RECORDS,text:document.querySelector('[role="dialog"]')?.textContent};
  });
  assert.deepEqual(recovered.saved,saved.saved);
  const operation=channel==="kiro"?"startKiroEnrollment":"startKimiEnrollment",poll=channel==="kiro"?"pollKiroEnrollment":"pollKimiEnrollment";
  assert.equal(recovered.records.filter(row=>row.operation===operation).length,1);
  assert.equal(recovered.records.filter(row=>row.operation===poll).length,saved.records.filter(row=>row.operation===poll).length);
  assert.equal(recovered.records.find(row=>row.operation==="getAccountAuthorizationReceipt").path.session_id,saved.sessionId);
  await page.click('loc=role:button[name="继续连接与应用"]');
  await page.waitForSelector('loc=role:button[name="完成"]');await page.click('loc=role:button[name="完成"]');
  await page.waitForFunction(()=>!document.querySelector('[role="dialog"]'));
  const account=await page.evaluate(async id=>{
    const {call}=await import("/src/api/client.ts");
    const page=await call("listAccountInventory",{query:{limit:100}},{versionScoped:true});
    return page.items.find(row=>row.id===id);
  },saved.saved.accounts[0].id);
  assert.equal(account.managed.authentication,"oauth");assert.ok(account.operations.includes("reauthorize"));assert.equal(account.status,"enabled");
  if(channel==="kiro")assert.equal(account.managed.credential.kind,"bearer");
  await record({scenario:`E-02-${channel}-commit-loss-close-return`,result:"PASS",beforeLoss:saved,afterRecovery:recovered,appliedAccount:account});
}
console.log(JSON.stringify({result:"PASS",scenarios:results.length,spaceId,environment:"development fixture only"}));
