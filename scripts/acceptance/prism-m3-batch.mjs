// Real embedded loopback gateway: first draft write succeeds, second is rejected
// by a synthetic 409 before reaching the server; later items must not execute.
const fs=await import("node:fs/promises");const path=await import("node:path");
const {spaceId,receipts}=globalThis.PRISM_M3;
const info=JSON.parse(await fs.readFile(path.join(receipts,"local-preview.json"),"utf8"));
if(!info.synthetic)throw Error("Synthetic gateway required");
const origin=`http://127.0.0.1:${info.admin_port}`;
const page=(await taskSpace(spaceId)).page("p1");
if(new URL(await page.url()).origin!==origin)throw Error("Unexpected browser origin");
const install=()=>{
 const original=window.fetch.bind(window);window.__m3Batch={writes:0,accepted:0,publications:0};
 window.fetch=async(...args)=>{
  const url=new URL(args[0] instanceof Request?args[0].url:String(args[0]),location.href);
  if(url.origin===location.origin&&url.pathname.endsWith("/publish"))window.__m3Batch.publications++;
  if(url.origin!==location.origin||!/^\/admin\/credentials\/[^/]+\/status$/.test(url.pathname))return original(...args);
  const state=window.__m3Batch;state.writes++;
  if(state.writes===2)return new Response(JSON.stringify({error:{code:"management_revision_conflict",message:"Synthetic concurrent change"}}),{status:409,headers:{"Content-Type":"application/json"}});
  const response=await original(...args);if(response.ok)state.accepted++;return response;
 };
};
const {identifier}=await page.cdp("Page.addScriptToEvaluateOnNewDocument",{source:`(${install.toString()})();`});
try{
 await page.reload();await page.waitForSelector('input[name="password"]',{state:"visible"});
 await page.fill('input[name="password"]',(await fs.readFile(path.join(info.root,"qa-password"),"utf8")).trim());
 await page.keyboard.press("Escape");await page.click('loc=role:heading[name="管理员登录"]');await page.click('loc=role:button[name="登录"]');
 await page.waitForSelector("main.canvas",{state:"visible"});await page.goto(origin+"/admin-ui/#/accounts");
 await page.click('loc=role:button[name="批量管理"]');
 for(const index of [0,1,2])await page.click(`.account-list input[type="checkbox"] >> nth=${index}`);
 await page.click('loc=css:.account-batch-toolbar button:text-is("停用")');
 await page.click('loc=role:button[name="确认停用"]');
 await page.waitForSelector('loc=role:button[name="完成"]',{state:"visible"});
 const result=await page.evaluate(()=>({...window.__m3Batch,outcomes:[...document.querySelectorAll('[data-outcome]')].map(e=>e.getAttribute("data-outcome"))}));
 if(JSON.stringify(result.outcomes)!==JSON.stringify(["saved_unapplied","rejected","unexecuted"])||result.writes!==2||result.accepted!==1||result.publications!==0)throw Error(JSON.stringify(result));
 await page.evaluate(async()=>{await Promise.all(document.getAnimations().filter(a=>a.effect?.getComputedTiming().iterations!==Infinity).map(a=>a.finished.catch(()=>{})));});
 await page.screenshot({path:path.join(receipts,"batch-partial.png")});
 await page.click('loc=role:button[name="完成"]');
 await page.waitForFunction(()=>!document.querySelector('[role="dialog"]')&&document.body.innerText.includes("已停用"));
 const reread=await page.evaluate(()=>({context:document.querySelector("main.canvas")?.getAttribute("data-context-version"),disabled:document.body.innerText.includes("已停用"),pending:document.body.innerText.includes("待应用")}));
 if(!reread.pending)throw Error("Draft result lost its pending-apply state");
 await fs.writeFile(path.join(receipts,"batch.json"),JSON.stringify({browser:"EgoLite",realGateway:true,syntheticSecondConflict:true,passed:true,result,reread},null,2));
 console.log({passed:true,result,reread});
}finally{await page.cdp("Page.removeScriptToEvaluateOnNewDocument",{identifier});}
