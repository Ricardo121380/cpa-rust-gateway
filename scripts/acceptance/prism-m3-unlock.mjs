// Six unlock layouts; theme changes use Settings before a real logout.
const fs=await import("node:fs/promises");const path=await import("node:path");
const {spaceId,receipts}=globalThis.PRISM_M3;
const info=JSON.parse(await fs.readFile(path.join(receipts,"local-preview.json"),"utf8"));
if(!info.synthetic)throw Error("Synthetic gateway required");
const base=`http://127.0.0.1:${info.admin_port}/admin-ui/#/`;
const page=(await taskSpace(spaceId)).page("p1");
if(!String(await page.url()).startsWith(base))throw Error("Unexpected origin");
const states=[];
for(const theme of ["light","dark"]){
 await page.goto(base+"unlock");await page.fill('input[name="password"]',(await fs.readFile(path.join(info.root,"qa-password"),"utf8")).trim());
 await page.keyboard.press("Escape");await page.click('loc=role:heading[name="管理员登录"]');await page.click('loc=role:button[name="登录"]');
 await page.waitForSelector("main.canvas",{state:"visible"});await page.goto(base+"settings");
 await page.click(`loc=role:radio[name="${theme==="light"?"浅色":"深色"}"]`);
 await page.click('loc=role:button[name="退出登录"]');await page.waitForSelector('input[name="password"]',{state:"visible"});
 for(const [width,height] of [[1440,900],[1280,720],[390,844]]){
  await page.cdp("Emulation.setDeviceMetricsOverride",{width,height,deviceScaleFactor:1,mobile:false});
  await page.evaluate(async()=>{await document.fonts.ready;await Promise.all(document.getAnimations().filter(a=>a.effect?.getComputedTiming().iterations!==Infinity).map(a=>a.finished.catch(()=>{})));});
  const state=await page.evaluate(()=>({theme:document.documentElement.dataset.theme,width:innerWidth,height:innerHeight,overflow:document.documentElement.scrollWidth>innerWidth+1,emptyPassword:document.querySelector('input[name="password"]').value==="",protectedContent:!!document.querySelector("main.canvas")}));
  if(state.overflow||!state.emptyPassword||state.protectedContent||state.theme!==theme)throw Error(JSON.stringify(state));
  const screenshot=`unlock-${theme}-${width}.png`;await page.screenshot({path:path.join(receipts,screenshot)});states.push({...state,screenshot});
 }
}
await fs.writeFile(path.join(receipts,"unlock.json"),JSON.stringify({browser:"EgoLite",realGateway:true,passed:true,states},null,2));
console.log({passed:true,states:states.length});
