const {readFileSync,writeFileSync}=await import('node:fs');
const directory='/Users/huangrui/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/docs/reports/assets/prism-frontend-polish-20260928';
const task=await taskSpace(23),page=task.page('p2');
await page.fetch('http://127.0.0.1:5200/__fixture/reset',{method:'POST'});
// Install before the generated client captures fetch. Record paths/methods only.
await page.cdp('Page.addScriptToEvaluateOnNewDocument',{source:`
window.__polishErrors=[];window.__polishNetwork=[];window.__polishRecording=true;
addEventListener('error',e=>window.__polishErrors.push({type:'error',message:e.message}));
addEventListener('unhandledrejection',e=>window.__polishErrors.push({type:'rejection',message:String(e.reason)}));
addEventListener('securitypolicyviolation',e=>window.__polishErrors.push({type:'csp',directive:e.violatedDirective,blocked:e.blockedURI}));
window.__polishFetchBase=globalThis.fetch;
const originalFetch=globalThis.fetch;
globalThis.fetch=function(input,options){
 if(window.__polishRecording)window.__polishNetwork.push({method:options?.method??input?.method??'GET',path:new URL(typeof input==='string'?input:input.url,location.href).pathname});
 return originalFetch.apply(this,arguments);
};`});
await page.reload();
await page.cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:1,mobile:false});
await page.fill('css:input[name="password"]','Prism-demo-2026');await page.click("loc=role:button[name='登录']");
await page.waitForFunction(()=>Boolean(document.querySelector('#main-navigation')));
await page.click('css:#main-navigation a[href$="#/settings"]');await page.click("loc=role:link[name='配置版本']");
await page.waitForFunction(()=>Boolean(document.querySelector('[data-version-id="draft-2026-08"]')));
await page.click('css:[data-version-id="draft-2026-08"] button:nth-of-type(2)');await page.click("loc=role:button[name='接续草稿']");
await page.waitForFunction(()=>document.querySelector('main.canvas')?.dataset.contextStatus==='draft');
await page.cdp('Emulation.setEmulatedMedia',{features:[{name:'forced-colors',value:'active'},{name:'prefers-color-scheme',value:'light'},{name:'prefers-contrast',value:'more'},{name:'prefers-reduced-motion',value:'reduce'}]});
await page.click('css:#main-navigation a[href$="#/"]');await page.waitForFunction(()=>Boolean(document.querySelector('.trend-modes')));
await page.click('css:.overview-range button:nth-of-type(3)');await page.waitForFunction(()=>Boolean(document.querySelector('.trend-modes')));
await page.click('css:.trend-modes button:nth-of-type(2)');
await page.waitForFunction(()=>document.querySelectorAll('.selection-indicator').length===3&&[...document.querySelectorAll('.selection-indicator')].every(svg=>getComputedStyle(svg).display==='none'&&!svg.parentElement.dataset.selectionReady));
const forced=await page.evaluate(()=>({active:matchMedia('(forced-colors: active)').matches,indicators:[...document.querySelectorAll('.selection-indicator')].map(svg=>({display:getComputedStyle(svg).display,ready:svg.parentElement.dataset.selectionReady??null})),selection:[...document.querySelectorAll('.rail a[aria-current="page"],.overview-range button[aria-pressed="true"],.trend-modes button[aria-pressed="true"]')].map(e=>({text:e.textContent.trim(),border:getComputedStyle(e).border,borderColor:getComputedStyle(e).borderColor})),errors:window.__polishErrors}));
if(forced.selection.length!==3||new Set(forced.selection.map(e=>e.borderColor)).size!==1||forced.errors.length)throw new Error('Forced-colors or strict-CSP failure');
await page.screenshot({path:`${directory}/forced-colors.png`});
const accessibility=JSON.parse(readFileSync(`${directory}/accessibility.json`,'utf8'));accessibility.forced=forced;
writeFileSync(`${directory}/accessibility.json`,JSON.stringify(accessibility,null,2)+'\n');
await page.cdp('Emulation.setEmulatedMedia',{features:[{name:'forced-colors',value:'none'},{name:'prefers-color-scheme',value:'light'},{name:'prefers-contrast',value:'no-preference'},{name:'prefers-reduced-motion',value:'no-preference'}]});
await page.waitForFunction(()=>document.querySelectorAll('.selection-indicator').length===3&&[...document.querySelectorAll('.selection-indicator')].every(svg=>svg.dataset.visible==='true'));
await page.waitForFunction(()=>Number(document.querySelector('#prism-lens-rail feGaussianBlur')?.getAttribute('stdDeviation'))===20);
const draftLens=await page.evaluate(()=>['topbar','rail','dock'].map(pane=>{const f=document.getElementById(`prism-lens-${pane}`),image=f.querySelector('feImage');return {pane,map:image.getAttribute('href').startsWith('data:image/png'),width:Number(image.getAttribute('width')),frost:Number(f.querySelector('feGaussianBlur').getAttribute('stdDeviation')),saturation:Number(f.querySelector('feColorMatrix[type="saturate"]').getAttribute('values'))};}));
await page.evaluate(()=>{window.__polishNetwork=[];window.__polishPhases=[];let previous='';window.__polishPhaseObserver=new MutationObserver(()=>{const dialog=document.querySelector('[role="dialog"]');const row={title:dialog?.querySelector('h2')?.textContent??null,buttons:[...document.querySelectorAll('.sheet-footer button,.dock button')].map(b=>({text:b.textContent.trim(),class:b.className,disabled:b.disabled}))};const next=JSON.stringify(row);if(next!==previous){window.__polishPhases.push(row);previous=next;}});window.__polishPhaseObserver.observe(document.body,{childList:true,subtree:true,characterData:true,attributes:true,attributeFilter:['disabled','aria-busy']});});
try {
 await page.click('css:.dock button');
 await page.waitForFunction(()=>document.querySelector('.inline-workspace')?.textContent.includes('完整净变化')&&document.querySelector('.inline-workspace-footer button.primary')?.disabled===false);
 const review=await page.evaluate(()=>({route:location.hash,text:document.querySelector('.inline-workspace').textContent,dock:[...document.querySelectorAll('.dock button')].map(b=>({text:b.textContent.trim(),class:b.className,disabled:b.disabled})),requests:window.__polishNetwork.slice()}));
 if(review.requests.some(r=>r.method!=='GET'))throw new Error('Dock review issued a write');
 await page.click('css:.inline-workspace-footer button.primary');
 await page.waitForFunction(()=>document.querySelector('[role="dialog"] h2')?.textContent==='确认应用配置');
 const confirm=await page.evaluate(()=>({title:document.querySelector('[role="dialog"] h2').textContent,button:document.querySelector('.sheet-footer button.primary')?.textContent,requests:window.__polishNetwork.slice(),dockDisabled:document.querySelector('.dock button').disabled}));
 if(confirm.requests.some(r=>r.method==='POST'&&r.path.endsWith('/publish')))throw new Error('Publish preceded explicit confirmation');
 await page.focus('css:.sheet-footer button.primary');
 await page.screenshot({path:`${directory}/confirm-apply.png`});
 await page.click('css:.sheet-footer button.primary');
 await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.textContent.includes('已核对：目标仍为当前活动配置。'));
 const receipt=await page.evaluate(()=>({text:document.querySelector('[role="dialog"]').textContent,dockDisabled:document.querySelector('.dock button').disabled}));
 await page.click("loc=role:button[name='完成']");
 await page.waitForFunction(()=>document.querySelector('main.canvas')?.dataset.contextStatus==='active'&&!document.querySelector('.dock'));
 await page.waitForFunction(()=>Number(document.querySelector('#prism-lens-rail feGaussianBlur')?.getAttribute('stdDeviation'))===9);
 const completed=await page.evaluate(()=>({context:document.querySelector('main.canvas').dataset.contextStatus,dock:!!document.querySelector('.dock'),dialogs:document.querySelectorAll('[role="dialog"]').length,lens:{frost:Number(document.querySelector('#prism-lens-rail feGaussianBlur').getAttribute('stdDeviation')),saturation:Number(document.querySelector('#prism-lens-rail feColorMatrix[type="saturate"]').getAttribute('values'))},requests:window.__polishNetwork.slice(),phases:window.__polishPhases,errors:window.__polishErrors}));
 writeFileSync(`${directory}/configuration-flow.json`,JSON.stringify({fixtureOnly:true,draftLens,review,confirm,receipt,completed},null,2)+'\n');
 console.log(JSON.stringify({forced,draftLens,reviewWrites:review.requests.filter(r=>r.method!=='GET'),confirm,completed}));
 if(!completed.requests.some(r=>r.method==='POST'&&r.path.endsWith('/publish'))||completed.errors.length)throw new Error('Fixture publication or strict-CSP failure');
} finally {
 await page.evaluate(()=>{window.__polishPhaseObserver?.disconnect();window.__polishRecording=false;if(window.__polishFetchBase)globalThis.fetch=window.__polishFetchBase;});
}
