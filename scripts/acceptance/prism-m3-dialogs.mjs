// EgoLite interaction/layout checks on the owned embedded synthetic gateway.
// Prepend PRISM_M3={spaceId,receipts}; authenticated p1, no dialog open.
const fs=await import("node:fs/promises");
const path=await import("node:path");
const {spaceId,receipts}=globalThis.PRISM_M3;
const info=JSON.parse(await fs.readFile(path.join(receipts,"local-preview.json"),"utf8"));
if(!info.synthetic)throw Error("Synthetic gateway required");
const base=`http://127.0.0.1:${info.admin_port}/admin-ui/#/`;
const page=(await taskSpace(spaceId)).page("p1");
if(!String(await page.url()).startsWith(base))throw Error("Unexpected origin");
const states=[];
const settle=()=>page.evaluate(async()=>{await document.fonts.ready;await Promise.all(document.getAnimations().filter(a=>a.effect?.getComputedTiming().iterations!==Infinity).map(a=>a.finished.catch(()=>{})));await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));});
for(const theme of ["light","dark"]){
 await page.goto(base+"settings");await page.click(`loc=role:radio[name="${theme==="light"?"浅色":"深色"}"]`);
 for(const [width,height] of [[1440,900],[1280,720],[390,844]]){
  await page.cdp("Emulation.setDeviceMetricsOverride",{width,height,deviceScaleFactor:1,mobile:false});
  for(const [route,action,title] of [["accounts","授权 / 导入账号","添加账号"],["models","接入模型","接入模型"],["access","创建客户端密钥","创建 API 密钥"]]){
   await page.goto(base+route);await page.click(`loc=role:button[name="${action}"]`);
   await page.waitForSelector(`loc=role:dialog[name="${title}"]`,{state:"visible"});await settle();
   // Probe wrapped, exact model IDs without saving them or inventing catalog evidence.
   if(route==="models")await page.fill('textarea[placeholder="每行一个完整模型 ID，最多 20 个"]',"Synthetic/"+"long-exact-model-".repeat(12));
   const layout=await page.evaluate(()=>{const dialog=document.querySelector('[role="dialog"]');const r=dialog.getBoundingClientRect();return {left:r.left,right:r.right,top:r.top,bottom:r.bottom,width:innerWidth,height:innerHeight,overflow:document.documentElement.scrollWidth>innerWidth+1,dialogOverflow:dialog.scrollWidth>dialog.clientWidth+1};});
   if(layout.overflow||layout.dialogOverflow||layout.left<11||layout.right>width-11||layout.top<0||layout.bottom>height+1)throw Error(JSON.stringify({route,theme,layout}));
   // Both tab directions must remain inside the active dialog.
   await page.focus('[role="dialog"] button[aria-label="关闭面板"]');
   await page.keyboard.press("Shift+Tab");
   if(!await page.evaluate(()=>!!document.activeElement.closest('[role="dialog"]')))throw Error("Reverse focus escaped dialog");
   await page.keyboard.press("Tab");
   if(!await page.evaluate(()=>!!document.activeElement.closest('[role="dialog"]')))throw Error("Focus escaped dialog");
   const screenshot=`dialog-${route}-${theme}-${width}.png`;
   await settle();await page.screenshot({path:path.join(receipts,screenshot)});
   await page.keyboard.press("Escape");
   if(route==="models")await page.click('loc=role:button[name="放弃修改"]');
   await page.waitForSelector('[role="dialog"]',{state:"hidden"});
   states.push({route,theme,width,height,layout,keyboard:true,screenshot});
  }
 }
}
await page.goto(base+"settings");
for(const name of ["减少透明度","增强对比度","减少动态效果"])await page.click(`loc=role:switch[name="${name}"]`);
const preferences=await page.evaluate(()=>({attributes:[...document.documentElement.attributes].filter(a=>a.name.startsWith("data-")).map(a=>[a.name,a.value]),switches:[...document.querySelectorAll('[role="switch"]')].map(e=>({label:e.getAttribute("aria-label")||e.textContent,checked:e.getAttribute("aria-checked")}))}));
await settle();await page.screenshot({path:path.join(receipts,"accessibility-preferences.png")});
await page.goto(base+"models");await page.fill('input[placeholder="搜索模型 ID"]',"no-such-synthetic-model");
await page.waitForFunction(()=>document.body.innerText.includes("没有匹配"));await settle();
await page.screenshot({path:path.join(receipts,"models-empty.png")});
await fs.writeFile(path.join(receipts,"dialogs.json"),JSON.stringify({browser:"EgoLite",realGateway:true,passed:true,states,preferences,emptyState:true},null,2));
console.log({passed:true,dialogStates:states.length,preferences});
