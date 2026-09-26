// EgoLite only. PRISM_M3={spaceId,receipts}; start from an authenticated synthetic gateway.
const fs=await import("node:fs/promises");
const path=await import("node:path");
const {spaceId,receipts}=globalThis.PRISM_M3;
const info=JSON.parse(await fs.readFile(path.join(receipts,"local-preview.json"),"utf8"));
if(!info.synthetic)throw Error("Synthetic gateway required");
const base=`http://127.0.0.1:${info.admin_port}/admin-ui/#/`;
const page=(await taskSpace(spaceId)).page("p1");
if(!String(await page.url()).startsWith(base))throw Error("Unexpected browser origin");
const result=[];
const settled=async()=>{
 await page.waitForFunction(()=>!Array.from(document.querySelectorAll('main.canvas [role="status"]')).some(e=>e.getBoundingClientRect().height&&/读取|加载/.test(e.innerText)));
 await page.evaluate(async()=>{await document.fonts.ready;await Promise.all(document.getAnimations().filter(a=>a.effect?.getComputedTiming().iterations!==Infinity).map(a=>a.finished.catch(()=>{})));await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));});
};
for(const theme of ["light","dark"]){
 await page.goto(base+"settings");await settled();
 await page.click(`loc=role:radio[name="${theme==="light"?"浅色":"深色"}"]`);
 for(const [width,height] of [[1440,900],[1280,720],[390,844]]){
  await page.cdp("Emulation.setDeviceMetricsOverride",{width,height,deviceScaleFactor:1,mobile:false});
  for(const route of ["","accounts","upstreams","models","access","monitoring","usage","settings"]){
   await page.goto(base+route);await settled();
   const state=await page.evaluate(()=>({hash:location.hash,theme:document.documentElement.dataset.theme,width:innerWidth,height:innerHeight,canvas:!!document.querySelector("main.canvas"),overflow:document.documentElement.scrollWidth>innerWidth+1,errors:[...document.querySelectorAll('main.canvas [role="alert"]')].filter(e=>e.getBoundingClientRect().height).map(e=>e.innerText),headings:[...document.querySelectorAll("main.canvas h2")].map(e=>e.textContent)}));
   if(!state.canvas||state.overflow||state.errors.length)throw Error(JSON.stringify(state));
   const name=`${route||"overview"}-${theme}-${width}`;
   await page.screenshot({path:path.join(receipts,name+".png")});
   result.push({...state,screenshot:name+".png"});
  }
  console.log({theme,width,completed:result.length});
 }
}
await fs.writeFile(path.join(receipts,"workspaces.json"),JSON.stringify({browser:"EgoLite",realGateway:true,passed:true,states:result},null,2));
console.log({passed:true,states:result.length});
