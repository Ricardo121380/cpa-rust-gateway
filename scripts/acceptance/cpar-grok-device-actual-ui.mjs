import assert from "node:assert/strict";
import {writeFile} from "node:fs/promises";
import {execFileSync} from "node:child_process";
import {dirname} from "node:path";
import {operationsHarness} from "./cpar-operations-support.mjs";

const options=globalThis.CPAR_OPERATIONS;
const h=await operationsHarness(options),{page,d,facts,clear,record,deviceFacts}=h;
const database=()=>JSON.parse(execFileSync("python3",["-c",String.raw`import json,sqlite3,sys
with sqlite3.connect(sys.argv[1]) as db:
 print(json.dumps(db.execute("SELECT id,provider,revision,enabled,auth_status FROM grok_accounts ORDER BY id").fetchall()))`,`${d.state}/control.sqlite3`],{encoding:"utf8"}));
await writeFile(`${dirname(options.descriptor)}/device-control.json`,JSON.stringify({mode:"success"})+"\n");
let saved,sessionId;
try{
  if(options.login)await h.login(options.firstLogin??false);
  await page.goto(`${d.origin}/admin-ui/#/oauth`);await clear();
  console.log(await page.snapshot());
  await page.click('loc=role:button[name="授权 Grok Build"]');
  await page.cdp("Fetch.enable",{patterns:[{urlPattern:"*/native-account-authorizations/*/poll",requestStage:"Response"}]});
  await page.click('loc=role:button[name="开始 Grok 授权"]');
  await page.waitForFunction(()=>window.CPAR_SEO_RECORDS.some(row=>row.path==="/admin/native-account-authorizations"&&row.status===200));
  const start=(await facts()).records.find(row=>row.path==="/admin/native-account-authorizations"&&row.status===200);
  sessionId=start.response.session_id;
  let committed=false;
  for(const deadline=Date.now()+20000;Date.now()<deadline&&!committed;){
    const paused=(await page.events()).filter(event=>event.method==="Fetch.requestPaused");
    for(const event of paused){
      assert.equal(event.params.responseStatusCode,200);
      saved=database();
      if(saved.length){
        await page.cdp("Fetch.failRequest",{requestId:event.params.requestId,errorReason:"ConnectionClosed"});committed=true;
      }else await page.cdp("Fetch.continueResponse",{requestId:event.params.requestId});
    }
    if(!committed)await new Promise(resolve=>setTimeout(resolve,100));
  }
  assert(committed,"No successful durable native grant observed before fault; this is not behavioral RED");
  await page.cdp("Fetch.disable");
  await page.waitForFunction(()=>document.querySelector('[role="alert"]'));
  const loss=await facts(),transport=await deviceFacts({ordinal:1,requests:1});
  await record({scenario:"grok-build-real-page-commit-before-response-loss",result:"OBSERVED",sessionId,saved,loss,transport});
  assert(loss.dialogText.includes("授权结果待确认"),"Saved Grok grant is displayed as pending without an unknown-result recovery path");
  await page.click('loc=role:button[name="关闭并标记结果未确认"]');
  await page.waitForFunction(()=>!document.querySelector('[role="dialog"]'));
  await page.click('loc=css:#main-navigation a[href="#/accounts"]');
  await page.click('loc=role:button[name="返回 Grok Build 授权任务"]');
  await page.click('loc=role:button[name="核对授权结果"]');
  await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.textContent.includes("授权已保存"));
  const recovered=await facts();
  assert.deepEqual(database(),saved);
  const reads=recovered.records.filter(row=>row.path===`/admin/native-account-authorizations/${sessionId}`&&row.method==="GET"&&row.status===200);
  assert.equal(reads.length,1);assert.equal(reads[0].response.account_id,saved[0][0]);
  assert.equal(reads[0].response.credential_revision,saved[0][2]);
  assert(!Object.hasOwn(reads[0].response,"user_code"));
  assert.equal(recovered.records.filter(row=>row.method==="DELETE"&&row.path.includes("native-account-authorizations")).length,0);
  assert.equal(recovered.records.filter(row=>row.method==="POST"&&row.path==="/admin/native-account-authorizations").length,1);
  const after=await deviceFacts({ordinal:1,requests:1});assert.equal(after.sessions.length,1);assert.equal(after.sessions[0].token_requests,1);
  // The CLI management actor is different from the browser administrator actor.
  // Neither it nor an unauthenticated caller may read or consume this handle.
  await h.api("GET",`/admin/native-account-authorizations/${sessionId}`,undefined,404);
  await h.api("POST",`/admin/native-account-authorizations/${sessionId}/poll`,undefined,503);
  await h.api("DELETE",`/admin/native-account-authorizations/${sessionId}`,undefined,503);
  const denied=await fetch(`${d.origin}/admin/native-account-authorizations/${sessionId}`);assert.equal(denied.status,404);
  assert.deepEqual(database(),saved);assert.equal((await deviceFacts({ordinal:1,requests:1})).sessions[0].token_requests,1);
  const screenshot=await page.screenshot({path:options.out.replace(/\.json$/u,"-recovered.png")});
  await record({scenario:"grok-build-commit-loss-close-leave-return-read-original",result:"PASS",saved,recovered,transport:after,screenshot,foreignActorRead:404,foreignActorPollCancel:503,unauthenticatedRead:404});
  await page.click('loc=role:button[name="关闭"]');
  console.log(await page.snapshot());
}catch(error){
  await record({scenario:"grok-build-commit-loss-recovery",result:"FAIL",error:String(error),sessionId,saved,facts:await facts(),transport:await deviceFacts()});
  throw error;
}finally{await page.cdp("Fetch.disable");}
