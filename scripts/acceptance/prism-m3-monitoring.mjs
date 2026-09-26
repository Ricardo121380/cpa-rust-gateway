// Targeted real-gateway regression for ledger/failure initial and refresh errors.
const fs=await import("node:fs/promises");const path=await import("node:path");
const {spaceId,receipts}=globalThis.PRISM_M3;
const info=JSON.parse(await fs.readFile(path.join(receipts,"local-preview.json"),"utf8"));
if(!info.synthetic)throw Error("Synthetic gateway required");
const origin=`http://127.0.0.1:${info.admin_port}`;
const page=(await taskSpace(spaceId)).page("p1");
if(new URL(await page.url()).origin!==origin)throw Error("Unexpected origin");
const install=()=>{const original=window.fetch.bind(window);window.__m3Other={fail:true};window.fetch=async(...args)=>{
 const url=new URL(args[0] instanceof Request?args[0].url:String(args[0]),location.href);
 if(window.__m3Other.fail&&url.origin===location.origin&&["/admin/operations/billing","/admin/operations/provider-account-pools/failures"].includes(url.pathname))return new Response(JSON.stringify({error:{code:"management_operations_busy"}}),{status:503,headers:{"Content-Type":"application/json"}});
 return original(...args);
};};
const {identifier}=await page.cdp("Page.addScriptToEvaluateOnNewDocument",{source:`(${install.toString()})();`});
const checks=[];
try{
 await page.reload();await page.waitForSelector('input[name="password"]',{state:"visible"});
 await page.fill('input[name="password"]',(await fs.readFile(path.join(info.root,"qa-password"),"utf8")).trim());
 await page.keyboard.press("Escape");await page.click('loc=role:heading[name="管理员登录"]');await page.click('loc=role:button[name="登录"]');
 await page.waitForSelector("main.canvas",{state:"visible"});
 for(const [tab,refresh] of [["ledger","重新读取账本"],["failures","重新读取失败记录"]]){
  await page.evaluate(()=>{window.__m3Other.fail=true;});await page.goto(origin+"/admin-ui/#/monitoring?tab="+tab);
  await page.waitForFunction(()=>document.querySelector('.read-status strong')?.textContent==="读取失败");
  await page.evaluate(()=>{window.__m3Other.fail=false;});await page.click('.read-status button');
  await page.waitForFunction(()=>!document.querySelector('.read-status'));
  const before=await page.evaluate(()=>({rows:document.querySelectorAll('.mon-table tbody tr').length,filters:document.querySelector('.mon-filters')?.innerText,summary:document.querySelector('.mon-summary')?.innerText}));
  await page.evaluate(()=>{window.__m3Other.fail=true;});await page.click(`loc=role:button[name="${refresh}"]`);
  await page.waitForFunction(()=>document.querySelector('.read-status strong')?.textContent==="刷新失败");
  const after=await page.evaluate(()=>({rows:document.querySelectorAll('.mon-table tbody tr').length,filters:document.querySelector('.mon-filters')?.innerText,summary:document.querySelector('.mon-summary')?.innerText}));
  if(JSON.stringify(before)!==JSON.stringify(after))throw Error(tab+" discarded its previous snapshot");
  await page.screenshot({path:path.join(receipts,tab+"-refresh-failure.png")});
  await page.evaluate(()=>{window.__m3Other.fail=false;});await page.click('.read-status button');
  await page.waitForFunction(()=>!document.querySelector('.read-status'));
  checks.push({tab,initialRecovery:true,retainedSnapshot:true,rows:before.rows});
 }
 await fs.writeFile(path.join(receipts,"monitoring-recovery.json"),JSON.stringify({browser:"EgoLite",realGateway:true,passed:true,checks},null,2));
 console.log({passed:true,checks});
}finally{await page.cdp("Page.removeScriptToEvaluateOnNewDocument",{identifier});}
