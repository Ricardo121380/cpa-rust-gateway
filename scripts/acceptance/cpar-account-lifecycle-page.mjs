import assert from "node:assert/strict";
import {mkdir,writeFile} from "node:fs/promises";

// Actual product controls with a local, redacted fixture backend. Provider and
// held-stream assertions live in the separately indexed public HTTP receipts.
const {spaceId,origin,outputDir,commit,only}=globalThis.CPAR_LIFECYCLE;
assert.match(origin,/^http:\/\/127\.0\.0\.1:\d+$/u);
await mkdir(outputDir,{recursive:true});
const page=(await taskSpace(spaceId)).page("p1");
const rows=[];
const cases=[
  [false,"enable","applied"],[false,"disable","applied"],
  [false,"disable","draft"],[false,"disable","publish-failed"],
  [false,"disable","rejected"],[false,"disable","lost"],
  [false,"remove","applied"],[false,"remove","publish-failed"],
  [false,"remove","rejected"],[false,"remove","batch-lost"],
  [true,"enable","applied"],[true,"disable","applied"],
  [true,"enable","unapplied"],[true,"disable","rejected"],
  [true,"disable","lost"],[true,"remove","applied"],
  [true,"remove","unapplied"],[true,"remove","rejected"],
  [true,"remove","batch-lost"],
].map(([native,action,mode])=>({native,action,mode,id:`${native?"native":"ordinary"}-${action}-${mode}`}));
const text=()=>page.evaluate(()=>document.querySelector('[role="dialog"]')?.textContent??"");
const outcomes=()=>page.evaluate(()=>[...document.querySelectorAll('[aria-label="账号操作结果"] [data-outcome]')].map(row=>row.dataset.outcome));
const shot=async name=>page.screenshot({path:`${outputDir}/${name}.png`});
const snapshot=async name=>writeFile(`${outputDir}/${name}.snapshot.json`,JSON.stringify(await page.snapshot({scope:"full_page"}),null,2)+"\n");
for(const item of cases.filter(item=>!only||item.id===only)){
  const row={...item,result:"FAIL",environment:"development-fixture-product-controls",commit,spaceId};
  rows.push(row);
  try{
    await page.goto(`${origin}/#/unlock`);await page.reload();
    await page.fill('input[name="password"]',"Prism-demo-2026");await page.click('loc=role:button[name="登录"]');await page.waitForSelector("main.canvas");
    await page.evaluate(async item=>{
      const resource=pattern=>performance.getEntriesByType("resource").map(row=>row.name).find(url=>pattern.test(url));
      const fixture=await import(resource(/\/src\/dev\/fixtures\.ts\?t=/u)??"/src/dev/fixtures.ts");
      fixture.prepareAccountLifecycleForTest(item.native,item.action!=="enable");
      window.CPAR_LIFECYCLE_FIXTURE=fixture;
      const {ManagementApi}=await import(resource(/\/src\/generated\/management-client\.ts\?t=/u)??"/src/generated/management-client.ts");
      const original=ManagementApi.prototype.request;
      window.CPAR_LIFECYCLE_HTTP={calls:[],mode:item.mode,mutations:0,working:undefined,applyLost:false};
      const mutation=item.native?(item.action==="remove"?"deleteNativeAccount":"updateNativeAccount"):(item.action==="remove"?"deleteCredential":"updateCredentialStatus");
      if(item.mode==="unapplied")fixture.failNextNativeRuntimeApplyForTest();
      ManagementApi.prototype.request=async function(operation,request){
        const state=window.CPAR_LIFECYCLE_HTTP,entry={operation,id:request.path?.account_id??request.path?.credential_id,version:request.headers?.["X-Config-Version"]};
        state.calls.push(entry);
        if(operation===mutation){state.mutations+=1;state.working=entry.version;if(state.mode==="rejected")return new Response(JSON.stringify({error:{code:"management_invalid_input",message:"controlled lifecycle rejection"}}),{status:400,headers:{"Content-Type":"application/json"}});}
        if(operation==="publishConfigVersion"&&state.mode==="publish-failed")return new Response(JSON.stringify({error:{code:"management_runtime_unavailable",message:"controlled publication failure"}}),{status:503,headers:{"Content-Type":"application/json"}});
        const response=await original.call(this,operation,request);entry.status=response.status;
        if(operation===mutation&&response.ok&&(state.mode==="lost"||(state.mode==="batch-lost"&&state.mutations===2)))throw new TypeError("controlled response lost after durable fixture mutation");
        if(operation==="applyRuntimeConfiguration"&&state.applyLost&&response.ok)throw new TypeError("controlled runtime apply response lost");
        return response;
      };
      if(item.mode==="draft"){
        const {call,callRevisioned}=await import("/src/api/client.ts");const {useVersionStore}=await import("/src/features/config-versions/versionStore.ts");
        const current=useVersionStore.getState().context;
        const draft=await call("forkConfigVersion",{path:{config_version_id:current.configVersionId},body:{id:"lifecycle-existing-draft",description:"Pre-existing unrelated change"},headers:{"X-Config-Version":current.configVersionId,"If-Match":current.revision}});
        const provider=fixture.accountLifecycleSnapshotForTest(draft.id).providers[0];
        const result=await callRevisioned("updateUpstream",{path:{upstream_id:provider.id},body:{...provider,name:"Existing draft modification"},headers:{"X-Config-Version":draft.id,"If-Match":draft.revision}});
        useVersionStore.getState().select({...draft,revision:result.revision});
      }
      location.hash=item.native?"#/accounts":"#/upstreams";
    },item);
    const ids=item.native?["native-lifecycle-a","native-lifecycle-b","native-lifecycle-c"]:["cred-relay-key","cred-relay-spare","cred-relay-third"];
    await page.waitForSelector(`tr[data-account-key="${ids[0]}"]`);
    await snapshot(`${item.id}-before`);
    const before=await page.evaluate(async()=>{const {useVersionStore}=await import("/src/features/config-versions/versionStore.ts");return window.CPAR_LIFECYCLE_FIXTURE.accountLifecycleSnapshotForTest(useVersionStore.getState().context.configVersionId);});
    await page.click('loc=role:button[name="批量管理"]');
    const selected=item.mode==="batch-lost"?ids:[ids[0]];
    for(const id of selected)await page.click(`loc=css:tr[data-account-key="${id}"] input[type="checkbox"]`);
    const label={enable:"启用",disable:"停用",remove:"删除"}[item.action];
    await page.click(`loc=css:[aria-label="批量账号操作"] button:has-text("${label}")`);
    await page.waitForSelector(`loc=role:dialog[name="${label} ${selected.length} 份授权"]`);
    if(item.action==="remove"){
      await page.waitForFunction(()=>document.querySelector('[aria-label="删除影响预览"] input')?.disabled===false);
      if(item.native)await page.waitForFunction(()=>![...document.querySelectorAll('[aria-label="删除影响预览"] .resource-identity')].some(row=>row.textContent?.includes("读取名称")));
      const preview=await text();assert(preview.includes("历史请求"));
      if(item.native){assert(preview.includes("a@example.test")&&preview.includes("Fixture Grok 1")&&preview.includes("Responses")&&preview.includes("cli-chat-proxy.grok.com")&&preview.includes("minimax-m3")&&preview.includes("grok-lifecycle-model"));assert.match(preview,/不能保证|不保证/u);if(selected.length===1)assert(preview.includes("c@example.test"));for(const id of [...ids,"batch-a-native-1","batch-a-native-2-endpoint","rt-minimax"])assert(!preview.includes(id),`internal reference must not be ordinary display text: ${id}`);}
      else assert(preview.includes(ids[0])&&preview.includes("ep-relay-a-responses")&&preview.includes("rt-minimax"));
      row.preview=preview;await snapshot(`${item.id}-preview`);await shot(`${item.id}-preview`);
      assert.equal(await page.evaluate(label=>[...document.querySelectorAll('[role="dialog"] button')].find(button=>button.textContent===`确认${label}`)?.disabled,label),true);
      await page.click('loc=css:[aria-label="删除影响预览"] input[type="checkbox"]');
    }
    if(item.action==="enable")assert.match(await text(),/调度|授权|许可/u);
    await page.click(`loc=role:button[name="确认${label}"]`);
    await page.waitForFunction(()=>[...document.querySelectorAll('[aria-label="账号操作结果"] [data-outcome]')].every(row=>row.dataset.outcome!=="pending"));
    await page.waitForSelector('loc=role:button[name="完成"]');
    const expected=item.mode==="draft"?"saved_draft":item.mode==="publish-failed"||item.mode==="unapplied"?"saved_unapplied":item.mode==="rejected"?"rejected":item.mode==="lost"?"unconfirmed":"applied";
    const seen=await outcomes();
    if(item.mode==="batch-lost")assert.deepEqual(seen,[item.native?"applied":"saved_unapplied","unconfirmed","unexecuted"]);else assert.deepEqual(seen,[expected]);
    row.initialOutcomes=seen;
    const counters=()=>page.evaluate(()=>structuredClone(window.CPAR_LIFECYCLE_HTTP));
    if(["lost","batch-lost"].includes(item.mode)){
      const prior=await counters();
      assert.equal(await page.evaluate(()=>[...document.querySelectorAll('[role="dialog"] button')].filter(button=>button.textContent?.includes("重试")||button.textContent?.startsWith("确认删除")).length),0);
      await page.click('loc=role:button[name="只读取账号状态核对未知结果"]');
      await page.waitForFunction(()=>![...document.querySelectorAll('[data-outcome]')].some(row=>row.dataset.outcome==="unconfirmed"));
      assert.equal((await counters()).mutations,prior.mutations,"readback must not replay an account mutation");
      const after=await outcomes();assert.equal(after[item.mode==="batch-lost"?1:0],"saved_application_unconfirmed");if(item.mode==="batch-lost")assert.equal(after[2],"unexecuted");row.readbackOutcomes=after;
    }
    if(item.mode==="unapplied"){
      await page.evaluate(()=>window.CPAR_LIFECYCLE_HTTP.applyLost=true);
      await page.click('loc=role:button[name="应用运行配置"]');await page.waitForSelector('loc=role:button[name="读取服务状态后核对"]');
      assert.equal(await page.evaluate(()=>[...document.querySelectorAll('[role="dialog"] button')].find(button=>button.textContent==="应用运行配置")?.disabled),true);
      const count=(await counters()).calls.filter(call=>call.operation==="applyRuntimeConfiguration").length;
      await page.click('loc=role:button[name="读取服务状态后核对"]');await page.waitForFunction(()=>[...document.querySelectorAll('[role="dialog"] button')].find(button=>button.textContent==="应用运行配置")?.disabled===false);
      assert.equal((await counters()).calls.filter(call=>call.operation==="applyRuntimeConfiguration").length,count);
      await page.evaluate(()=>window.CPAR_LIFECYCLE_HTTP.applyLost=false);await page.click('loc=role:button[name="应用运行配置"]');await page.waitForFunction(()=>[...document.querySelectorAll('[data-outcome]')].some(row=>row.dataset.outcome==="applied"));row.runtimeReadBeforeExplicitApply=true;
    }
    row.counters=await counters();
    const after=await page.evaluate(()=>window.CPAR_LIFECYCLE_FIXTURE.accountLifecycleSnapshotForTest(window.CPAR_LIFECYCLE_HTTP.working));
    for(const key of ["providers","endpoints","models","candidates","permissions"])assert.deepEqual(after[key],before[key],`shared ${key} preserved`);
    const accountRows=item.native?after.native:after.accounts;
    if(item.mode!=="rejected"){
      if(item.action==="remove"){assert(!accountRows.some(account=>account.id===ids[0]));assert(accountRows.some(account=>account.id===ids[2]));}
      else assert.equal(item.native?accountRows.find(account=>account.id===ids[0]).enabled:accountRows.find(account=>account.id===ids[0]).status!=="disabled",item.action==="enable");
    }else assert.deepEqual(accountRows,item.native?before.native:before.accounts);
    row.savedMetadata=after;row.result="PASS";await snapshot(`${item.id}-result`);await shot(`${item.id}-result`);
    console.log(JSON.stringify({case:item.id,result:row.result,outcomes:await outcomes(),mutations:row.counters.mutations}));
  }catch(error){row.error=String(error);await snapshot(`${item.id}-failed`).catch(()=>{});await shot(`${item.id}-failed`).catch(()=>{});throw error;}
  finally{await writeFile(`${outputDir}/lifecycle-page.json`,JSON.stringify({commit,spaceId,result:rows.every(row=>row.result==="PASS")?"PASS":"FAIL",cases:rows},null,2)+"\n");}
}
