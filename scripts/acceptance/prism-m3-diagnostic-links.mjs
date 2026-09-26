// EgoLite: actual persisted request links, synthetic embedded gateway only.
const fs=await import("node:fs/promises");const path=await import("node:path");
const {spaceId,receipts}=globalThis.PRISM_M3;
const info=JSON.parse(await fs.readFile(path.join(receipts,"local-preview.json"),"utf8"));
if(!info.synthetic)throw Error("Synthetic gateway required");
const base=`http://127.0.0.1:${info.admin_port}/admin-ui/#/`;
const page=(await taskSpace(spaceId)).page("p1");
if(!String(await page.url()).startsWith(base))throw Error("Unexpected origin");
await page.goto(base+"unlock");await page.reload();
await page.fill('input[name="password"]',(await fs.readFile(path.join(info.root,"qa-password"),"utf8")).trim());
await page.keyboard.press("Escape");await page.click('loc=role:heading[name="管理员登录"]');
await page.click('loc=role:button[name="登录"]');await page.waitForSelector("main.canvas",{state:"visible"});
await page.cdp("Emulation.setDeviceMetricsOverride",{width:1440,height:900,deviceScaleFactor:1,mobile:false});
const checks=[];
for(const label of ["账号状态","提供商","模型目录","运行诊断"]){
 await page.goto(base+"monitoring");
 await page.waitForSelector(".request-table tbody tr button",{state:"visible"});
 await page.click('loc=css:.request-table tbody tr:first-child button');
 await page.waitForSelector(".request-attempts a",{state:"visible"});
 const href=await page.evaluate(name=>[...document.querySelectorAll(".request-attempts a")].find(a=>a.textContent===name).getAttribute("href"),label);
 await page.click(`loc=role:link[name="${label}"]`);
 await page.waitForFunction(()=>!document.querySelector('[role="dialog"]')&&!!document.querySelector("main.canvas"));
 await page.waitForFunction(()=>![...document.querySelectorAll('main.canvas [role="status"]')].some(e=>e.getBoundingClientRect().height&&/读取|加载/.test(e.textContent)));
 const state=await page.evaluate(name=>{
  const params=new URLSearchParams(location.hash.split("?")[1]);
  const details=[...document.querySelectorAll("details")].find(d=>d.querySelector("summary")?.textContent==="目录状态与诊断");
  return {hash:location.hash,exactAccount:params.get("account_id"),runtimeRows:[...document.querySelectorAll(".account-desktop tbody tr")].map(r=>r.dataset.accountId),providerOpen:!!document.querySelector("#provider-detail"),catalogOpen:details?.open,catalogRows:details?.querySelectorAll("tbody tr").length,target:!!document.querySelector('[aria-label="当前诊断对象"]'),errors:[...document.querySelectorAll('main.canvas [role="alert"]')].filter(e=>e.getBoundingClientRect().height).map(e=>e.textContent)};
 },label);
 if(state.hash!==href||state.errors.length)throw Error("Diagnostic destination failed");
 if(label==="账号状态"&&(state.runtimeRows.length!==1||state.runtimeRows[0]!==state.exactAccount))throw Error("Wrong account target");
 if(label==="提供商"&&!state.providerOpen)throw Error("Provider workspace not opened");
 if(label==="模型目录"&&(!state.catalogOpen||state.catalogRows>1))throw Error("Catalog target not scoped");
 if(label==="运行诊断"&&!state.target)throw Error("Runtime target missing");
 checks.push({label,...state});
}
await fs.writeFile(path.join(receipts,"diagnostic-links.json"),JSON.stringify({browser:"EgoLite",realGateway:true,realProviderCalls:0,passed:true,checks},null,2));
console.log({passed:true,links:checks.length});
