import assert from "node:assert/strict";
import {mkdir,writeFile} from "node:fs/promises";

// UI identity failures project the separately tested management HTTP verdict.
// This fixture does not establish an observed provider subject or token proof.
const {spaceId,origin,outputDir,commit,only}=globalThis.CPAR_OAUTH_MATRIX;
assert.match(origin,/^http:\/\/127\.0\.0\.1:\d+$/u);await mkdir(outputDir,{recursive:true});
const page=(await taskSpace(spaceId)).page("p1"),rows=[];
const cases=["first","same","different","unknown","disabled","callback-lost","maintenance-lost","binding-failed","publish-failed"];
const contents=()=>page.evaluate(()=>document.querySelector('[role="dialog"]')?.textContent??"");
const waitText=text=>page.waitForFunction(text=>document.querySelector('[role="dialog"]')?.textContent.includes(text),text);
const counter=()=>page.evaluate(()=>structuredClone(window.CPAR_OAUTH_MATRIX_HTTP));
const metadata=()=>page.evaluate(()=>window.CPAR_OAUTH_MATRIX_FIXTURE.accountLifecycleSnapshotForTest(window.CPAR_OAUTH_MATRIX_HTTP.working));
const screenshot=async name=>page.screenshot({path:`${outputDir}/${name}.png`});
const submit=async(label,start)=>{
  await page.click(`loc=role:button[name="${start}"]`);await page.waitForSelector('textarea[aria-label="回调地址"]');
  const state=await page.evaluate(()=>new URL(document.querySelector('[role="dialog"] a')?.href).searchParams.get("state"));assert(state);
  await page.fill('textarea[aria-label="回调地址"]',`http://127.0.0.1:1455/auth/callback?code=synthetic-local&state=${state}`);
  await page.click('loc=role:button[name="完成授权"]');
};
const first=async(channel)=>{
  const label=channel==="codex"?"Codex / ChatGPT":"Claude";
  await page.evaluate(()=>location.hash="#/oauth");await page.waitForSelector(`loc=role:button[name="授权 ${label}"]`);
  // Semantic ground truth is read before entering the first new flow.
  await page.snapshot();await page.click(`loc=role:button[name="授权 ${label}"]`);await submit(label,"开始授权");
};
for(const channel of ["codex","claude"])for(const condition of cases){
  const id=`${channel}-${condition}`;if(only&&only!==id)continue;
  const row={id,channel,condition,result:"FAIL",environment:"development-fixture-product-controls",commit,spaceId};rows.push(row);
  try{
    await page.goto(`${origin}/#/unlock`);await page.reload();await page.fill('input[name="password"]',"Prism-demo-2026");await page.click('loc=role:button[name="登录"]');await page.waitForSelector("main.canvas");
    await page.evaluate(async()=>{
      const resource=pattern=>performance.getEntriesByType("resource").map(row=>row.name).find(url=>pattern.test(url));
      window.CPAR_OAUTH_MATRIX_FIXTURE=await import(resource(/\/src\/dev\/fixtures\.ts\?t=/u)??"/src/dev/fixtures.ts");
      const {ManagementApi}=await import(resource(/\/src\/generated\/management-client\.ts\?t=/u)??"/src/generated/management-client.ts"),original=ManagementApi.prototype.request;
      window.CPAR_OAUTH_MATRIX_HTTP={calls:[],mode:"same",working:undefined,saved:[]};
      ManagementApi.prototype.request=async function(operation,request){
        const state=window.CPAR_OAUTH_MATRIX_HTTP,entry={operation,id:request.path?.credential_id??request.body?.id,version:request.headers?.["X-Config-Version"]};state.calls.push(entry);
        const callback=["completeCodexEnrollment","completeClaudeEnrollment","completeCredentialOAuth"].includes(operation);
        if(callback)state.working=entry.version;
        if(operation==="createEndpointCredentialBinding"&&state.mode==="binding-failed")return new Response(JSON.stringify({error:{code:"management_invalid_input",message:"controlled binding failure after account save"}}),{status:400,headers:{"Content-Type":"application/json"}});
        if(operation==="publishConfigVersion"&&state.mode==="publish-failed")return new Response(JSON.stringify({error:{code:"management_runtime_unavailable",message:"controlled publication failure after account save"}}),{status:503,headers:{"Content-Type":"application/json"}});
        const response=await original.call(this,operation,request);entry.status=response.status;
        if(callback&&response.ok){const body=await response.clone().json();state.saved.push({id:body.id??body.credential_id,revision:body.revision,state:body.state});if(state.mode==="lost")throw new TypeError("controlled consumed OAuth callback response lost");}
        return response;
      };
    });
    const maintenance=["same","different","unknown","disabled","maintenance-lost"].includes(condition);
    if(maintenance){
      await first(channel);await waitText("账号授权已保存并连接接口");
      const enrolled=(await counter()).saved.at(-1).id;assert(enrolled);row.accountId=enrolled;
      await page.click('loc=role:button[name="完成"]');
      await page.evaluate(async({condition,enrolled})=>{
        const fixture=window.CPAR_OAUTH_MATRIX_FIXTURE;
        if(condition==="disabled")fixture.disableOAuthCredentialForTest(enrolled);
        fixture.setOAuthIdentityForTest(["different","unknown"].includes(condition)?condition:"same");
        window.CPAR_OAUTH_MATRIX_HTTP.mode=condition==="maintenance-lost"?"lost":"same";
        const {queryClient}=await import("/src/api/queryClient.ts");await queryClient.invalidateQueries();location.hash="#/accounts";
      },{condition,enrolled});
      await page.waitForSelector(`tr[data-account-key="${enrolled}"]`);await page.snapshot();
      const before=await metadata();row.before=before;
      await page.click(`loc=css:tr[data-account-key="${enrolled}"] .account-list-actions button:has-text("重新授权")`);
      await submit(channel==="codex"?"Codex":"Claude",channel==="codex"?"启动授权":"开始授权");
      if(["different","unknown"].includes(condition)){
        await waitText("原账号及连接保留");assert.doesNotMatch(await contents(),/本次授权回调已由服务端确认并保存|账号授权已保存并连接接口/u);
        await page.click(channel==="codex"?'loc=role:button[name="重新读取状态"]':'loc=role:button[name="核对授权结果"]');
        await page.waitForFunction(()=>!document.querySelector('textarea')?.value);
        const after=await metadata(),old=before.accounts.find(account=>account.id===enrolled),saved=after.accounts.find(account=>account.id===enrolled);
        assert.deepEqual(saved,old);for(const key of ["providers","endpoints","bindings","models","permissions"])assert.deepEqual(after[key],before[key]);row.after=after;row.identityVerdictRetained=true;
      }else if(condition==="maintenance-lost"){
        await waitText(channel==="codex"?"不会重复提交回调":"授权结果");const prior=await counter();
        await page.click(channel==="codex"?'loc=role:button[name="重新读取状态"]':'loc=role:button[name="核对授权结果"]');
        if(channel==="claude"){await page.waitForSelector('loc=role:button[name="继续连接与应用"]');await page.evaluate(()=>window.CPAR_OAUTH_MATRIX_HTTP.mode="same");await page.click('loc=role:button[name="继续连接与应用"]');await waitText("账号授权已保存");}
        else {await waitText("无法确认这项变化来自本次续授权");assert.doesNotMatch(await contents(),/本次授权回调已由服务端确认并保存/u);}
        assert.equal((await counter()).calls.filter(call=>call.operation.startsWith("complete")).length,prior.calls.filter(call=>call.operation.startsWith("complete")).length);
        row.lostResponseReadOnly=true;
      }else{
        await waitText(channel==="codex"?"本次授权回调已由服务端确认并保存":"账号授权已保存");
        const after=await metadata();const saved=after.accounts.find(account=>account.id===enrolled);assert.equal(saved.revision,before.accounts.find(account=>account.id===enrolled).revision+1);assert.equal(saved.status,condition==="disabled"?"disabled":"active");
        for(const key of ["providers","endpoints","bindings","models","permissions"])assert.deepEqual(after[key],before[key]);row.after=after;
      }
    }else{
      await page.evaluate(condition=>window.CPAR_OAUTH_MATRIX_HTTP.mode=condition==="callback-lost"?"lost":condition,condition);
      await first(channel);
      if(condition==="callback-lost"){
        await page.waitForSelector('loc=role:button[name="核对授权结果"]');await page.click('loc=role:button[name="核对授权结果"]');await page.waitForSelector('loc=role:button[name="继续连接与应用"]');await page.evaluate(()=>window.CPAR_OAUTH_MATRIX_HTTP.mode="same");await page.click('loc=role:button[name="继续连接与应用"]');row.lostResponseReadOnly=true;
      }else if(["binding-failed","publish-failed"].includes(condition)){
        await waitText("接口连接或配置尚未应用");row.savedBeforeApplyFailure=true;
        const saved=(await counter()).saved.at(-1);assert(saved?.id);assert((await metadata()).accounts.some(account=>account.id===saved.id));
        await page.evaluate(()=>window.CPAR_OAUTH_MATRIX_HTTP.mode="same");await page.click('loc=role:button[name="继续连接与应用"]');
      }
      await waitText("账号授权已保存并连接接口");row.after=await metadata();row.accountId=(await counter()).saved.at(-1).id;
    }
    row.counters=await counter();const callbacks=row.counters.calls.filter(call=>["completeCodexEnrollment","completeClaudeEnrollment","completeCredentialOAuth"].includes(call.operation));assert.equal(callbacks.length,maintenance?2:1,"each attempt consumes one callback; recovery never replays it");
    assert.equal(row.counters.calls.filter(call=>call.operation.startsWith("cancel")).length,0);row.result="PASS";await screenshot(`${id}-result`);
    row.feedback=await contents();console.log(JSON.stringify({case:id,result:"PASS",callbacks:callbacks.length,accountId:row.accountId}));
  }catch(error){row.error=String(error);await screenshot(`${id}-failed`).catch(()=>{});row.feedback=await contents().catch(()=>"");throw error;}
  finally{await writeFile(`${outputDir}/oauth-page-matrix.json`,JSON.stringify({commit,spaceId,result:rows.every(row=>row.result==="PASS")?"PASS":"FAIL",cases:rows},null,2)+"\n");}
}
