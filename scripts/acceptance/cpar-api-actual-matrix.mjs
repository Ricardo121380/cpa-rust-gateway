import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
import {operationsHarness} from "./cpar-operations-support.mjs";

const options=globalThis.CPAR_OPERATIONS;
const h=await operationsHarness(options),{page,d,config,seed,clear,facts,record}=h;
const materials=JSON.parse(await readFile(d.api_material_file,"utf8"));
try{
for(const preset of options.presets??["responses","chat","messages","kimi","grok-api"]){
  const definition=d.targets[preset];
  const base=preset==="grok-api"?definition.base:`${definition.base}/operations-${preset}`;
  let selected=(await config()).endpoints.items.find(row=>row.base_url===base&&row.adapter_id===definition.adapter);
  for(const count of options.counts??[0,1,2]){
    let prepared;
    if(count===1){
      const current=await config(),owner=current.upstreams.find(row=>row.id===selected.upstream_id);
      prepared=await seed(async api=>{
        if(!current.groups.some(row=>row.id==="ops-retained-group")){
          await api("POST","/admin/access-groups",{id:"ops-retained-group",name:"Existing model permission",status:"active",limits:{}},201);
          await api("POST","/admin/client-keys",{id:"ops-retained-key",access_group_id:"ops-retained-group",status:"active",expires_at_ms:null},201);
          await api("POST","/admin/access-groups/ops-retained-group/routes",{route_id:current.routes[0].id,enabled:true},201);
        }
        await api("PATCH",`/admin/upstreams/${selected.upstream_id}`,{id:owner.id,name:owner.name,kind:owner.kind,enabled:false,tags:owner.tags,egress_policy_id:owner.egress_policy_id});
        const {id,adapter_id,api_format,base_url,inference_path,models_path,transport}=selected;
        await api("PATCH",`/admin/endpoints/${selected.id}`,{id,adapter_id,api_format,base_url,inference_path,models_path,transport,enabled:false});
      });
    }else if(count===2){
      const owner=`ops-${preset}-second`,id=`${owner}-connection`;
      prepared=await seed(async api=>{
        await api("POST","/admin/upstreams",{id:owner,name:`Second ${preset}`,kind:definition.kind,enabled:false,tags:[],egress_policy_id:null},201);
        await api("POST",`/admin/upstreams/${owner}/endpoints`,{id,adapter_id:definition.adapter,api_format:definition.format,base_url:base,inference_path:definition.path,models_path:null,transport:"https",enabled:false},201);
      });
      selected={id,upstream_id:owner};
    }
    await page.goto(`${d.origin}/admin-ui/#/upstreams`);
    if(prepared){await h.login();await page.goto(`${d.origin}/admin-ui/#/upstreams`);}
    await page.click('loc=role:button[name="添加 AI 提供商"]');
    await page.selectOption('form#provider-setup-form select:not([required])',preset);
    await page.fill('input[placeholder="例如：我的 OpenAI"]',`Operations ${preset} ${count}`);
    if(preset!=="grok-api")await page.fill('input[type="url"]',base);
    await page.waitForFunction(count=>document.querySelector("main")?.textContent.includes(count===0?"没有匹配连接，将新建。":count===1?"将复用":"找到 2 个匹配连接"),count);
    await page.fill('textarea[autocomplete="off"]',`${materials[preset]}-${count}`);
    const model=`ops-model-${preset}-${count}`;
    await page.fill('textarea[placeholder="每行一个真实模型 ID"]',model);
    await page.click('#provider-setup-form input[type="checkbox"]');
    const before=await config();await clear();
    if(count===2){
      await page.click('loc=role:button[name="保存并接入"]');
      assert.equal((await facts()).records.filter(row=>row.method==="POST").length,0,"no write before choosing a matching connection");
      await page.selectOption('form#provider-setup-form select[required]',selected.id);
    }
    await page.click('loc=role:button[name="保存并接入"]');
    await page.waitForFunction(()=>document.querySelector("main")?.textContent.includes("接入修改已应用"),undefined,{timeout:15000});
    const result=await facts(),after=await config();
    const imports=result.records.filter(row=>row.method==="POST"&&row.path.endsWith("/account-import"));
    assert.equal(imports.length,1);assert.equal(imports[0].status,201);
    const bindings=result.records.filter(row=>row.method==="POST"&&row.path.endsWith("/credential-bindings"));
    assert.equal(bindings.length,1);assert.equal(bindings[0].body.credential_id,imports[0].response.id);
    const endpointId=bindings[0].path.split("/")[3];
    if(count)assert.equal(endpointId,selected.id);
    selected=after.endpoints.items.find(row=>row.id===endpointId);assert(selected);
    assert.equal(selected.adapter_id,definition.adapter);assert.equal(selected.api_format,definition.format);
    assert.equal(selected.transport,"https");assert.equal(selected.base_url,base);assert.equal(selected.inference_path,definition.path);
    const owner=after.upstreams.find(row=>row.id===selected.upstream_id);assert.equal(owner.kind,definition.kind);
    assert.equal(imports[0].path,`/admin/upstreams/${owner.id}/account-import`);
    assert.equal(imports[0].body.channel,preset==="grok-api"?"grok.official":preset==="kimi"?"kimi-api":preset==="messages"?"anthropic-compatible":"openai-compatible");
    assert.equal(after.accounts.length,before.accounts.length+1);
    assert.equal(result.records.filter(row=>row.method==="POST"&&row.path==="/admin/upstreams").length,count?0:1);
    assert.equal(result.records.filter(row=>row.method==="POST"&&/^\/admin\/upstreams\/[^/]+\/endpoints$/u.test(row.path)).length,count?0:1);
    assert.equal(result.records.filter(row=>row.method==="POST"&&row.path.endsWith("/publish")).length,1);
    assert.equal(after.upstreams.length,before.upstreams.length+(count?0:1));
    assert.equal(after.endpoints.items.length,before.endpoints.items.length+(count?0:1));
    if(count){assert.deepEqual(after.upstreams,before.upstreams);assert.deepEqual(after.endpoints.items,before.endpoints.items);assert.equal(owner.enabled,false);assert.equal(selected.enabled,false);}
    assert.deepEqual(after.groups,before.groups);assert.deepEqual(after.keys,before.keys);assert.deepEqual(after.grants,before.grants);
    assert.equal(result.records.filter(row=>row.method==="POST"&&row.path.includes("/access-groups/")).length,0);
    const newModel=after.models.find(row=>row.model_name===model);assert(newModel);
    const newRoutes=after.routes.filter(row=>row.public_model_id===newModel.id);assert.equal(newRoutes.length,1);
    assert(after.grants.every(group=>group.routes.every(grant=>!newRoutes.some(route=>route.id===grant.route_id))),"new model is not granted by onboarding");
    const image=await page.screenshot({path:`${options.out.replace(/\.json$/u,"")}-${preset}-${count}-applied.png`});
    await record({scenario:`api-${preset}-matching-${count}-save-bind-apply`,result:"PASS",matchingCount:count,prepared,definition,selected,owner,newModel:model,permissionsChanged:false,grantsCreated:0,providerInference:"NOT_RUN",before,after,facts:result,screenshot:image});
    await page.click('loc=role:button[name="关闭"]');
    await page.waitForFunction(()=>!document.querySelector(".inline-workspace"));
  }
}
console.log(await page.snapshot());
}catch(error){await record({scenario:"api-matrix-harness-failure",result:"FAIL",error:String(error),facts:await facts()});throw error;}
