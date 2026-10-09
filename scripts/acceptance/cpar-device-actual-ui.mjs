import assert from "node:assert/strict";
import {readFile,writeFile} from "node:fs/promises";
import {dirname} from "node:path";
import {execFileSync} from "node:child_process";
import {operationsHarness} from "./cpar-operations-support.mjs";

const options=globalThis.CPAR_OPERATIONS;
const h=await operationsHarness(options),{page,d,facts,clear,record,deviceFacts}=h;
const channel=options.channel??"kiro",label=channel==="kiro"?"Kiro":"Kimi Coding";
const prefix=channel==="kiro"?"kiro":"kimi",mode=options.mode??"success";
const fault=options.fault??true;
await writeFile(`${dirname(options.descriptor)}/device-control.json`,JSON.stringify({mode})+"\n");
const before=await deviceFacts();
function database(id,version){
  return JSON.parse(execFileSync("python3",["-c",String.raw`import json,sqlite3,sys
with sqlite3.connect(sys.argv[1]) as db:
 print(json.dumps(db.execute("SELECT id,upstream_id,status,revision FROM upstream_credentials WHERE id=? AND config_version_id=?",(sys.argv[2],sys.argv[3])).fetchall()))`,`${d.state}/control.sqlite3`,id,version],{encoding:"utf8"}));
}
async function screenshot(name){return page.screenshot({path:`${options.out.replace(/\.json$/u,"")}-${name}.png`});}
try{
  // This stays in one EgoLite invocation until paused responses have been failed/continued.
  await page.goto(`${d.origin}/admin-ui/#/oauth`);await clear();
  await page.click(`loc=role:button[name="授权 ${label}"]`);
  if(fault)await page.cdp("Fetch.enable",{patterns:[{urlPattern:`*/${prefix}-authorization/poll`,requestStage:"Response"}]});
  await page.click('loc=role:button[name="开始授权"]');
  await page.waitForFunction(()=>window.CPAR_SEO_RECORDS.some(row=>row.path.endsWith("authorization/start")&&row.status===200),undefined,{timeout:12000});
  const start=(await facts()).records.find(row=>row.path.endsWith("authorization/start")&&row.status===200);
  let paused;
  if(fault){
    for(const deadline=Date.now()+20000;Date.now()<deadline&&!paused;){
      paused=(await page.events()).find(event=>event.method==="Fetch.requestPaused"&&event.params?.request?.url.endsWith(`/${prefix}-authorization/poll`));
      if(!paused)await new Promise(resolve=>setTimeout(resolve,100));
    }
    assert(paused,"EgoLite did not capture the response pause; cannot claim response loss");assert.equal(paused.params.responseStatusCode,201);
    const saved=database(start.body.id,start.config);assert.equal(saved.length,1);assert.equal(saved[0][0],start.body.id);
    const committed=await page.evaluate(async({session,config})=>{
      const r=await fetch(`/admin/account-authorization/${session}`,{headers:{...window.CPAR_SEO_HEADERS,"X-Config-Version":config}});
      if(r.status!==200)throw new Error(`receipt ${r.status}`);return r.json();
    },{session:start.response.session_id,config:start.config});
    delete committed.actor;assert.equal(committed.state,"completed");assert.equal(committed.credential_id,start.body.id);
    await page.cdp("Fetch.failRequest",{requestId:paused.params.requestId,errorReason:"ConnectionClosed"});
    await page.cdp("Fetch.disable");
    await page.waitForSelector('loc=role:button[name="关闭并标记结果未确认"]');
    const lost=await facts();assert.equal(lost.records.filter(row=>row.path.endsWith("authorization/poll")&&row.status===201).length,0);
    const lossImage=await screenshot(`${channel}-commit-loss`);
    await page.click('loc=role:button[name="关闭并标记结果未确认"]');await page.waitForFunction(()=>!document.querySelector('[role="dialog"]'));
    await page.click('loc=css:#main-navigation a[href="#/accounts"]');
    await page.click(`loc=role:button[name="返回 ${label} 授权任务"]`);
    await page.click('loc=role:button[name="核对授权结果"]');
    await page.waitForSelector('loc=role:button[name="继续连接与应用"]');
    const recovered=await facts(),receipt=recovered.records.filter(row=>row.path===`/admin/account-authorization/${start.response.session_id}`&&row.status===200).at(-1);
    assert.equal(receipt.response.credential_id,start.body.id);assert.equal(receipt.response.config_version,start.config);
    assert.deepEqual(database(start.body.id,start.config),saved);
    const negative=await page.evaluate(async start=>{
      const path=`/admin/account-authorization/${start.response.session_id}`;
      const noAuth=await fetch(path);
      const replay=await fetch(start.path.replace(/\/start$/u,"/poll"),{method:"POST",headers:{...window.CPAR_SEO_HEADERS,"Content-Type":"application/json","X-Config-Version":start.config,"If-Match":start.receiptRevision},body:JSON.stringify({id:start.body.id,session_id:start.response.session_id,replace_existing:false})});
      return {unauthenticatedReceiptStatus:noAuth.status,completedPollReplayStatus:replay.status};
    },{...start,receiptRevision:receipt.response.revision});
    assert.equal(negative.unauthenticatedReceiptStatus,404);assert.equal(negative.completedPollReplayStatus,409);
    await page.click('loc=role:button[name="继续连接与应用"]');
    await page.waitForSelector('loc=role:button[name="完成"]');
    const applied=await facts();assert(applied.records.some(row=>row.method==="POST"&&row.path.endsWith("/publish")&&row.status===200));
    const finished=await h.config();assert.equal(finished.active.id,start.config);
    assert.equal(finished.accounts.filter(row=>row.id===start.body.id).length,1);
    assert.equal(applied.records.filter(row=>row.method==="POST"&&row.path.endsWith("authorization/start")).length,1);
    assert.equal(applied.records.filter(row=>row.method==="POST"&&row.path.endsWith("authorization/cancel")).length,0);
    const exchanges=await deviceFacts({ordinal:before.sessions.length+1,requests:1}),session=exchanges.sessions.find(row=>row.ordinal===before.sessions.length+1);
    assert.equal(session.token_requests,1);assert.equal(exchanges.sessions.length,before.sessions.length+1);
    const completedImage=await screenshot(`${channel}-restored-applied`);
    await record({scenario:`${channel}-actual-stock-device-commit-loss-close-return-apply`,result:"PASS",faultLayer:"CDP response-stage ConnectionClosed before delivery to generated client",start,committed,databaseBeforeLoss:saved,databaseAfterRecovery:database(start.body.id,start.config),lost,recovered,negative,applied,active:finished.active,exchanges:session,screenshots:[lossImage,completedImage]});
    await page.click('loc=role:button[name="完成"]');
  }else{
    const terminal=mode==="denied"?"授权被拒绝":mode==="expired"?"授权已过期":mode==="failed"?"授权未完成":mode==="ttl"||mode==="transport"?"关闭并标记结果未确认":"完成";
    if(terminal==="完成")await page.waitForSelector('loc=role:button[name="完成"]',{timeout:20000});
    else await page.waitForFunction(terminal=>document.querySelector('[role="dialog"]')?.textContent.includes(terminal),terminal,{timeout:20000});
    const result=await facts(),saved=database(start.body.id,start.config);
    assert.equal(saved.length,mode==="success"||mode==="pending_then_success"?1:0);
    if(mode==="ttl"||mode==="transport"){
      await page.click('loc=role:button[name="核对授权结果"]');
      await page.waitForFunction(()=>window.CPAR_SEO_RECORDS.some(row=>row.path.startsWith("/admin/account-authorization/")&&row.status===200));
    }
    const boundaryFacts=await facts();
    const receipt=boundaryFacts.records.filter(row=>row.path===`/admin/account-authorization/${start.response.session_id}`&&row.status===200).at(-1);
    if(mode==="ttl")assert.equal(receipt?.response.state,"expired");
    if(mode==="transport")assert.equal(receipt?.response.state,"unknown");
    const expectedRequests=mode==="ttl"?0:mode==="pending_then_success"?2:1;
    const current=await deviceFacts({ordinal:before.sessions.length+1,requests:expectedRequests}),session=current.sessions.find(row=>row.ordinal===before.sessions.length+1);
    assert.equal(session.token_requests,expectedRequests);
    await record({scenario:`${channel}-actual-device-${mode}`,result:"PASS",start,facts:boundaryFacts,database:saved,session});
    await page.click(`loc=role:button[name="${mode==="transport"?"关闭并标记结果未确认":mode==="success"||mode==="pending_then_success"?"完成":"关闭"}"]`);
  }
  console.log(await page.snapshot());
}catch(error){
  await record({scenario:`${channel}-${mode}-${fault?"commit-loss":"boundary"}`,result:"FAIL",error:String(error),facts:await facts(),device:await deviceFacts()});
  throw error;
}finally{if(fault)await page.cdp("Fetch.disable");}
