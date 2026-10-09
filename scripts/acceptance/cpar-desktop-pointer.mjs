import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
import {operationsHarness} from "./cpar-operations-support.mjs";

const options=globalThis.CPAR_OPERATIONS;
const h=await operationsHarness(options),{page,d,seed,config,facts,clear,record}=h;
const materials=JSON.parse(await readFile(d.api_material_file,"utf8"));
for(const viewport of [{width:1280,height:800},{width:1440,height:900}]){
  try{
    await page.cdp("Emulation.setDeviceMetricsOverride",{...viewport,deviceScaleFactor:1,mobile:false});
    const namespace=`pointer-${viewport.width}-${Date.now()}`;
    const prepared=await seed(async api=>{
      for(let index=0;index<18;index++){
        const id=`${namespace}-${index}`;
        await api("POST","/admin/access-groups",{id,name:`${namespace} prior draft ${index}`,status:"active",limits:{}},201);
      }
    },{publish:false});
    await h.login();await page.goto(`${d.origin}/admin-ui/#/versions`);
    await page.click(`tr[data-version-id="${prepared.scope}"] button:text-is("编辑草稿")`);
    await page.click('loc=role:button[name="接续草稿"]');
    await page.waitForFunction(()=>!document.querySelector('[role="dialog"]'));
    await page.goto(`${d.origin}/admin-ui/#/upstreams`);
    await page.click('loc=role:button[name="添加 AI 提供商"]');
    await page.selectOption('form#provider-setup-form select:not([required])',"responses");
    await page.fill('input[placeholder="例如：我的 OpenAI"]',`Pointer ${viewport.width}`);
    const endpoint=(await config()).endpoints.items.find(row=>row.base_url.endsWith("/operations-responses"));
    await page.fill('input[type="url"]',endpoint.base_url);
    await page.waitForFunction(()=>/将复用|找到 2 个匹配连接/u.test(document.querySelector("main")?.textContent??""));
    if(await page.evaluate(()=>!!document.querySelector('form#provider-setup-form select[required]')))await page.selectOption('form#provider-setup-form select[required]',endpoint.id);
    await page.fill('textarea[autocomplete="off"]',`${materials.responses}-pointer-${viewport.width}`);
    await clear();await page.click('loc=role:button[name="保存并接入"]');
    await page.waitForFunction(()=>document.querySelector("main")?.textContent.includes("服务与接口已保存到草稿，尚未应用"));
    const saved=await facts();assert.equal(saved.records.filter(row=>row.method==="POST"&&row.path.endsWith("/publish")).length,0);
    await page.click('loc=role:button[name="核对合并清单"]');
    await page.waitForSelector('section[aria-label="本次配置核对"] input[type="checkbox"]');
    const preview=await facts();assert(preview.text.includes(`${namespace}-17`));
    await page.click('section[aria-label="本次配置核对"] details summary');
    const beforeScroll=await page.evaluate(()=>({x:scrollX,y:scrollY}));
    await page.mouse.move(viewport.width-100,viewport.height/2);
    await page.mouse.wheel(0,700,{label:"scroll full configuration review"});
    const afterScroll=await page.evaluate(()=>({x:scrollX,y:scrollY}));
    await page.click('section[aria-label="本次配置核对"] input[type="checkbox"]');
    const image=await page.screenshot({path:`${options.out.replace(/\.json$/u,"")}-${viewport.width}-before-apply.png`});
    await page.click('loc=role:button[name="校验并应用这份清单"]');
    await page.waitForFunction(()=>document.querySelector("main")?.textContent.includes("接入修改已应用"));
    const applied=await facts(),active=await config();assert.equal(active.active.id,prepared.scope);
    assert.equal(applied.records.filter(row=>row.method==="POST"&&row.path.endsWith("/account-import")).length,1);
    assert.equal(applied.records.filter(row=>row.method==="POST"&&row.path.endsWith("/publish")&&row.status===200).length,1);
    await record({scenario:`desktop-${viewport.width}x${viewport.height}-prior-draft-pointer-review-apply`,result:"PASS",viewport,prepared,beforeScroll,afterScroll,saved,preview,applied,active:active.active,screenshot:image});
    await page.click('loc=role:button[name="关闭"]');
  }catch(error){
    const image=await page.screenshot({path:`${options.out.replace(/\.json$/u,"")}-${viewport.width}-failure.png`});
    await record({scenario:`desktop-${viewport.width}-pointer`,result:"FAIL",error:String(error),facts:await facts(),screenshot:image});throw error;
  }
}
console.log(await page.snapshot());
