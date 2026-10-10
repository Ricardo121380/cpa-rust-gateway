import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
import {execFileSync} from "node:child_process";
import {dirname} from "node:path";
import {operationsHarness} from "./cpar-operations-support.mjs";

const options=globalThis.CPAR_OPERATIONS;
const h=await operationsHarness(options),{page,d,facts,clear,record}=h;
assert(d.native_sso_material_file,"Requires the owned native SSO loopback service");
const materials=JSON.parse(await readFile(d.native_sso_material_file,"utf8"));
const stored=()=>JSON.parse(execFileSync("python3",["-c",String.raw`import json,sqlite3,sys
with sqlite3.connect('file:'+sys.argv[1]+'?mode=ro',uri=True) as db:
 print(json.dumps({'accounts':db.execute('SELECT id,provider,revision,enabled,auth_status FROM grok_accounts ORDER BY id').fetchall(),'ordinary':db.execute('SELECT count(*) FROM upstream_credentials').fetchone()[0],'bindings':db.execute('SELECT count(*) FROM endpoint_credential_bindings').fetchone()[0],'events':db.execute('SELECT count(*) FROM gateway_event_log').fetchone()[0]}))`,`${d.state}/control.sqlite3`],{encoding:"utf8"}));
const receives=async()=>JSON.parse(await readFile(`${dirname(options.descriptor)}/provider-receives.json`,"utf8"));
const imports=value=>value.records.filter(row=>row.path==="/admin/native-accounts/import"&&row.method==="POST");
async function choose(channel){
  await page.click('loc=role:button[name="导入账号文件"]');
  await page.click(`loc=role:button[name*="Grok ${channel==="web"?"Web":"Console"}"]`);
  await page.waitForSelector('textarea[name="secret"]');
  const value=await facts();
  assert(value.dialogText.includes("SSO 凭据"));
  assert(!value.dialogText.includes("开始授权"));
  return value.dialogText;
}
async function discardResponse(path){
  await page.cdp("Fetch.enable",{patterns:[{urlPattern:`*${path}`,requestStage:"Response"}]});
}
async function dropCommitted(){
  for(const deadline=Date.now()+15000;Date.now()<deadline;){
    const paused=(await page.events()).filter(event=>event.method==="Fetch.requestPaused");
    if(paused.length){
      assert.equal(paused.length,1);assert([200,201].includes(paused[0].params.responseStatusCode));
      const snapshot=stored();
      await page.cdp("Fetch.failRequest",{requestId:paused[0].params.requestId,errorReason:"ConnectionClosed"});
      await page.cdp("Fetch.disable");return snapshot;
    }
    await new Promise(resolve=>setTimeout(resolve,50));
  }
  throw new Error("No genuine committed response observed; classify as driver/environment failure");
}
try{
  if(options.login)await h.login(options.firstLogin??false);
  await page.goto(`${d.origin}/admin-ui/#/accounts`);await page.waitForSelector("main.canvas");
  if(await page.evaluate(()=>!!document.querySelector('[role="dialog"]')))await page.click('loc=role:button[name="取消"]');
  for(const channel of ["web","console"]){
    for(const scenario of ["success","unavailable","loss"]){
      await clear();const before=stored(),entry=await choose(channel);
      await page.fill('textarea[name="secret"]',materials[`${channel}-${scenario}`]);
      if(scenario==="loss")await discardResponse("/admin/native-accounts/import");
      await page.click('loc=role:button[name="导入账号"]');
      const committed=scenario==="loss"?await dropCommitted():null;
      await page.waitForSelector('loc=role:button[name="完成"]');
      await page.waitForFunction(()=>!document.querySelector('button[form="account-import-form"]'));
      if(scenario==="loss"){
        await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.textContent.includes("结果未确认"));
        const uncertain=await facts();assert.equal(imports(uncertain).length,0);
        assert.equal(committed.accounts.length,before.accounts.length+1);
        await page.click('loc=role:button[name="核对导入结果"]');
        await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.textContent.includes("已回读账号，运行应用待核对"));
        const recovered=await facts();
        const reads=recovered.records.filter(row=>row.method==="GET"&&row.path.startsWith("/admin/native-account-imports/"));
        assert.equal(reads.length,1);assert.equal(reads[0].response.channel,`grok.${channel}`);
        assert.equal(reads[0].response.saved,true);assert.equal(reads[0].response.account_exists,true);
        assert(committed.accounts.some(row=>row[0]===reads[0].response.account_id&&row[1]===channel));
        assert.equal(imports(recovered).length,0);assert.deepEqual(stored(),committed);
        // Lose the actual apply response; a GET-only service read must precede any next write.
        await discardResponse("/admin/operations/runtime/apply");
        await page.click('loc=role:button[name="应用运行配置"]');await dropCommitted();
        await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.textContent.includes("应用结果未确认"));
        assert.equal(await page.evaluate(()=>[...document.querySelectorAll('button')].find(button=>button.textContent==="应用运行配置")?.disabled),true);
        await page.click('loc=role:button[name="读取服务状态后核对"]');
        await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.textContent.includes("先前应用请求的结果仍未确认"));
        const reviewed=await facts();assert.equal(reviewed.records.filter(row=>row.path==="/admin/system"&&row.method==="GET").length,1);
        assert.deepEqual(stored(),committed);
        await record({scenario:`${channel}-unknown-import-read-original-and-unknown-apply-read-first`,result:"PASS",entry,committed,uncertain,recovered,reviewed});
      }else{
        const value=await facts(),post=imports(value);assert.equal(post.length,1);
        assert.equal(post[0].body.channel,`grok.${channel}`);assert.equal(post[0].status,201);
        assert.equal(post[0].response.created,1);assert.equal(post[0].response.runtime_applied,true);
        assert.equal(post[0].response.identity_state,scenario==="success"?"observed":"unavailable");
        assert.equal(stored().accounts.length,before.accounts.length+1);
        if(scenario==="unavailable")assert(value.dialogText.includes("渠道暂未返回身份"));
        await record({scenario:`${channel}-${scenario}-stock-import`,result:"PASS",entry,value,stored:stored()});
      }
      const confirmed=stored();
      const screenshot=await page.screenshot({path:options.out.replace(/\.json$/u,`-${channel}-${scenario}.png`)});
      await page.click('loc=role:button[name="完成"]');await page.waitForFunction(()=>!document.querySelector('[role="dialog"]'));
      await page.reload();await page.waitForSelector('input[name="password"]');
      await page.fill('input[name="password"]',(await readFile(d.password_file,"utf8")).trim());
      await page.click('loc=role:button[name="登录"]');await page.waitForSelector("main.canvas");
      assert.deepEqual(stored(),confirmed);assert.equal(confirmed.ordinary,before.ordinary);assert.equal(confirmed.bindings,before.bindings);
      await page.waitForFunction(()=>[...document.querySelectorAll('main tr')].filter(row=>row.textContent.includes("Grok")).length>0);
      await record({scenario:`${channel}-${scenario}-close-reload-retains-confirmed`,result:"PASS",stored:confirmed,screenshot});
    }
  }
  const upstream=await receives();assert.equal(upstream.filter(row=>row.inference).length,0);
  assert(upstream.filter(row=>row.method==="POST").every(row=>row.quota&&row.path==="/rest/rate-limits"));
  assert.equal(stored().events,0);
  for(const channel of ["web","console"])for(const scenario of ["success","unavailable","loss"])
    assert.equal(upstream.filter(row=>row.channel===channel&&row.case===scenario).length,1);
  await record({scenario:"web-console-exact-six-identity-reads-zero-inference-no-ordinary-bindings",result:"PASS",upstream,quotaReads:upstream.filter(row=>row.quota).length,stored:stored(),realProviderCalls:0});
  console.log(await page.snapshot());
}catch(error){await record({scenario:"web-console-directed-page",result:"FAIL",error:String(error),facts:await facts(),stored:stored()});throw error;}
finally{await page.cdp("Fetch.disable");}
