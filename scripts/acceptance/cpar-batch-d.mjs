import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";

// Run with ego-browser nodejs. One existing TaskSpace, local synthetic data,
// management HTTP responses, and actual controls; no production/provider calls.
const {spaceId,origin,scenario,outputDir,commit}=globalThis.CPAR_D;
assert.match(origin,/^http:\/\/127\.0\.0\.1:\d+$/u);
const task=await taskSpace(spaceId);
const page=task.page("p1");
await mkdir(outputDir,{recursive:true});
const checks=[];
const record=(name,details={})=>checks.push({name,result:"PASS",...details});
const routeNames={"/":"仪表盘","/overview":"仪表盘","/accounts":"账号管理","/oauth":"OAuth 授权","/catalog":"模型目录","/usage":"用量与费用","/monitoring":"请求日志","/billing":"计费与价格","/versions":"配置版本","/upstreams":"AI 提供商","/models":"模型管理","/access":"API 密钥","/egress":"出口策略","/runtime":"运行诊断","/audit":"审计与备份","/settings":"设置"};
const route=async path=>{
  await page.evaluate(path=>{location.hash=`#${path}`;},path);
  await page.waitForFunction(({path,name})=>location.hash===`#${path}`&&document.querySelector("main.canvas")?.getAttribute("aria-label")===name&&document.querySelector("main.canvas .workspace > section"),{path,name:routeNames[path.split("?")[0]]});
};
const text=()=>page.evaluate(()=>document.querySelector("main")?.textContent??document.body.textContent??"");
const waitText=value=>page.waitForFunction(value=>document.body.textContent?.includes(value),value);
const mode=(operation,value)=>page.evaluate(({operation,value})=>{window.CPAR_D_HTTP.modes[operation]=value;},{operation,value});
const calls=operation=>page.evaluate(operation=>window.CPAR_D_HTTP.calls.filter(call=>call.operation===operation),operation);
const hook=()=>page.evaluate(async()=>{
  const url=performance.getEntriesByType("resource").map(row=>row.name).find(url=>/\/src\/generated\/management-client\.ts\?t=/u.test(url))??"/src/generated/management-client.ts";
  const {ManagementApi}=await import(url);
  const original=ManagementApi.prototype.request;
  window.CPAR_D_HTTP={modes:{},calls:[],cache:{},release:{},applied:[]};
  const reply=(status,body)=>new Response(JSON.stringify(body),{status,headers:{"Content-Type":"application/json"}});
  ManagementApi.prototype.request=async function(operation,request){
    const state=window.CPAR_D_HTTP,mode=state.modes[operation];
    state.calls.push({operation,cursor:request.query?.cursor??request.query?.before_id??null});
    if(mode==="error")return reply(500,{error:{code:"synthetic_read_failed",message:"D controlled source unavailable"}});
    if(mode==="denied")return reply(403,{error:{code:"synthetic_permission_denied",message:"D controlled permission denied"}});
    if(mode==="locked")return reply(404,{error:{code:"management_access_denied",message:"D controlled expired session"}});
    if(mode==="lost")throw new TypeError("D controlled response lost");
    if(mode==="invalid")return reply(400,{error:{code:"invalid_request",message:"D controlled write rejected"}});
    if(mode==="next-error"&&request.query?.cursor)return reply(500,{error:{code:"synthetic_cursor_failed",message:"D controlled next page unavailable"}});
    if(mode==="pool-record")return reply(200,[{id:"d-pool",name:"D controlled pool",upstream_id:"relay-a",enabled:true}]);
    if(mode==="no-availability")return reply(200,[]);
    if(mode==="pin-receipt")return reply(200,{...request.body,request_id:"d-pin-receipt",config_version_id:request.headers["X-Config-Version"],config_revision:1,credential_revision:1,runtime_credential_revision:1,connection_revision:1,runtime_build:"development:development:development",server_instance:"d-fixture",permission_basis:"management_key_projection",outcome:"failed",upstream_sent:false,attempt_count:1,response_started:false,observed_at_ms:Date.now(),stage:"egress_admission"});
    if(mode==="recovery-receipt")return reply(200,{state:"probe_scheduled"});
    if(mode==="resource-page")return reply(200,{items:request.query?.before_id?[{id:"2",action:"upstream_updated",actor:"admin",occurred_at_ms:Date.now(),config_version_id:request.headers["X-Config-Version"],resource_kind:"upstream",resource_id:"relay-a"}]:[],next_before_id:request.query?.before_id?null:"2"});
    if(mode==="resource-next-error"&&request.query?.before_id)return reply(500,{error:{code:"synthetic_cursor_failed",message:"D controlled audit page unavailable"}});
    if(mode==="resource-next-error")return reply(200,{items:[],next_before_id:"2"});
    if(mode==="hold")await new Promise(resolve=>{state.release[operation]=resolve;});
    const response=await original.call(this,operation,request);
    if(mode==="lost-after-write"&&response.ok){state.applied.push({operation,status:response.status});throw new TypeError("D controlled response lost after fixture mutation");}
    if(!response.ok||!mode)return response;
    const body=await response.clone().json();
    if(mode==="empty-items") {body.items=[];body.next_cursor=null;}
    if(mode==="failure-pages") {state.cache[operation]??=body;const first=state.cache[operation];return reply(200,{...first,items:request.query?.cursor?first.items.slice(1,2):first.items.slice(0,1),next_cursor:request.query?.cursor?null:"D-failure-page2"});}
    if(mode==="mandatory") {body.password_change_required=true;delete state.modes[operation];}
    if(mode==="paused")body.accepting_requests=false;
    if(mode==="recovered")for(const row of body)row.availability="available";
    if(mode==="estimate")for(const row of body.items)for(const family of ["input_tokens","output_tokens","reasoning_tokens","cache_read_tokens","cache_creation_tokens","cached_tokens"])row[family].provenance="estimated";
    if(mode==="request-evidence")for(const row of body.items??[])if(row.usage){row.usage.provenance="estimated";row.usage.input_accounting="inclusive";}
    if(mode==="ledger-evidence")for(const row of body.items){row.usage_provenance="estimated";row.input_accounting="inclusive";}
    if(mode==="catalog-evidence"||mode==="catalog-expired") {body.items=[{model:"d-source-model",present_in_last_success:true}];body.current_model_count=1;body.total_count=1;body.next_cursor=null;body.target.stale_at_ms=Date.now()-1000;body.target.expires_at_ms=Date.now()+(mode==="catalog-expired"?-1000:60000);}
    if(mode==="pool-pages") {state.cache[operation]??=body;const first=state.cache[operation];return reply(200,{...first,items:request.query?.cursor?first.items.slice(1):[],next_cursor:request.query?.cursor?null:"D-pool-page2"});}
    if(mode==="cost-pages"||mode==="next-error") {state.cache[operation]??=body;const first=state.cache[operation];return reply(200,{...first,items:request.query?.cursor?first.items.slice(1,2):first.items.slice(0,1),next_cursor:request.query?.cursor?null:"D-cost-page2"});}
    return reply(200,body);
  };
});
const login=async(password="Prism-demo-2026")=>{
  await page.fill('input[name="password"]',password);
  await page.click('loc=role:button[name="登录"]');
};
const reset=async()=>{
  await page.goto(`${origin}/#/unlock`);
  await page.reload();
  await page.waitForSelector('input[name="password"]');
  await login();await page.waitForSelector("main.canvas");
  await hook();
};
const prepareTarget=()=>page.evaluate(async()=>{
  const url=performance.getEntriesByType("resource").map(row=>row.name).find(url=>/\/src\/dev\/fixtures\.ts\?t=/u.test(url))??"/src/dev/fixtures.ts";
  const {prepareAccountPinForTest}=await import(url);prepareAccountPinForTest(true);
});
const desktop=()=>page.cdp("Emulation.setDeviceMetricsOverride",{width:1440,height:1000,deviceScaleFactor:1,mobile:false});
const screenshot=name=>page.screenshot({path:`${outputDir}/${name}.png`});
const readyButton=name=>page.waitForFunction(name=>[...document.querySelectorAll("button")].some(button=>button.textContent===name&&!button.disabled&&!button.closest('[inert]')&&button.getClientRects().length>0),name);
const appearance=async(width,theme)=>{
  await page.cdp("Emulation.setDeviceMetricsOverride",{width,height:width===375?812:1000,deviceScaleFactor:1,mobile:false});
  if(await page.evaluate(()=>document.documentElement.dataset.theme)!==theme)await page.click('loc=css:button[aria-label="切换深浅外观"]');
  // System appearance has no data-theme; the first toggle may select the other
  // explicit theme. Observe that result before selecting the requested one.
  if(await page.evaluate(()=>document.documentElement.dataset.theme)!==theme)await page.click('loc=css:button[aria-label="切换深浅外观"]');
  await page.waitForFunction(theme=>document.documentElement.dataset.theme===theme,theme);
};
const assertDialogFits=async()=>{
  await page.waitForFunction(()=>document.querySelector('[role="dialog"]')?.contains(document.activeElement));
  assert.ok(await page.evaluate(()=>{const dialog=document.querySelector('[role="dialog"]'),box=dialog.getBoundingClientRect();return box.left>=-1&&box.right<=innerWidth+1&&document.documentElement.scrollWidth<=innerWidth+1;}));
};
try {
  await desktop();
  if(scenario==="inventory") {
    await reset();
    const paths=["/","/overview","/accounts","/oauth","/catalog","/usage","/monitoring","/monitoring?tab=failures","/billing","/versions","/upstreams","/models","/access","/egress","/runtime","/audit","/settings"];
    await page.evaluate(()=>{window.CPAR_D_ERRORS=[];window.addEventListener("error",event=>window.CPAR_D_ERRORS.push(event.message));});
    for(const width of [1440,375])for(const theme of ["light","dark"]) {
      await appearance(width,theme);
      for(const path of paths) {
        await route(path);
        await page.waitForFunction(()=>!document.querySelector("vite-error-overlay")&&document.querySelector("main")?.textContent.length>60);
        const layout=await page.evaluate(()=>{
          const main=document.querySelector("main");
          const outside=[...main.querySelectorAll("*")].filter(node=>{
            const box=node.getBoundingClientRect();
            if((node.checkVisibility&&!node.checkVisibility())||box.width===0||box.right<=innerWidth+1||node.closest('[aria-hidden="true"]'))return false;
            for(let parent=node.parentElement;parent&&parent!==main;parent=parent.parentElement){
              const css=getComputedStyle(parent);
              if(css.clipPath!=="none"||css.clip!=="auto")return false;
              if(parent.tagName==="DETAILS"&&!parent.open&&!parent.querySelector(":scope > summary")?.contains(node))return false;
              if(["auto","scroll"].includes(css.overflowX)&&parent.scrollWidth>parent.clientWidth)return false;
            }
            return true;
          }).map(node=>node.tagName+"."+String(node.className)).slice(0,5);
          return {width:innerWidth,documentWidth:document.documentElement.scrollWidth,canvasWidth:main.clientWidth,canvasScroll:main.scrollWidth,outside,theme:document.documentElement.dataset.theme,title:main.querySelector("h2")?.textContent};
        });
        assert.ok(layout.documentWidth<=width+1,`${path} ${width} document overflow: ${JSON.stringify(layout)}`);
        assert.deepEqual(layout.outside,[],`${path} ${width} content outside scroll containers: ${JSON.stringify(layout)}`);
        assert.equal(layout.theme,theme);assert.ok(layout.title);
        record("reachable page and contained layout",{path,width,theme,canvasScroll:layout.canvasScroll});
      }
      await route("/usage");await waitText("费用来源");await screenshot(`usage-${width}-${theme}`);
    }
    await page.cdp("Emulation.setDeviceMetricsOverride",{width:812,height:375,deviceScaleFactor:1,mobile:false});
    await page.cdp("Emulation.setEmulatedMedia",{features:[{name:"prefers-reduced-motion",value:"reduce"}]});
    await route("/settings");
    assert.equal(await page.evaluate(()=>matchMedia("(prefers-reduced-motion: reduce)").matches),true);
    record("landscape with reduced motion",{width:812,height:375});
    assert.deepEqual(await page.evaluate(()=>window.CPAR_D_ERRORS),[]);
  } else if(scenario==="shell") {
    await page.goto(`${origin}/#/monitoring?tab=requests&model=return-check`);await page.reload();
    await page.waitForSelector('input[name="password"]');
    assert.match(await text(),/登录后返回原页面/u);
    await login("wrong-synthetic-password");await waitText("账号或密码不正确");
    await login();await page.waitForURL(/#\/monitoring\?tab=requests&model=return-check/u);
    record("failed login preserves original destination and filters");
    await route("/overview");
    assert.equal(await page.evaluate(()=>document.querySelector('#main-navigation a[aria-current="page"]')?.hash),"#/");
    assert.match(await page.evaluate(()=>document.querySelector(".top-context")?.textContent??""),/仪表盘/u);
    record("overview alias retains page identity");
    await page.cdp("Emulation.setDeviceMetricsOverride",{width:375,height:812,deviceScaleFactor:1,mobile:false});
    await page.click('loc=css:#nav-toggle');await page.click('loc=css:#main-navigation a[href="#/usage"]');
    await page.waitForFunction(()=>document.activeElement===document.querySelector("main.canvas"));
    record("narrow navigation moves focus to new workspace");
    await page.click('loc=css:#nav-toggle');await page.keyboard.press("Escape");
    assert.equal(await page.evaluate(()=>document.activeElement?.id),"nav-toggle");
    record("Escape returns focus to navigation opener");
    await desktop();await hook();await mode("summarizeRequests","locked");await page.evaluate(()=>{location.hash="#/monitoring?tab=requests&model=expired-return";});
    await page.waitForSelector('input[name="password"]');await mode("summarizeRequests",undefined);await login();
    await page.waitForURL(/#\/monitoring\?tab=requests&model=expired-return/u);record("expired session reauthenticates without losing destination");
  } else if(scenario==="password") {
    await page.goto(`${origin}/#/unlock`);await page.reload();await hook();await mode("loginAdministrator","mandatory");await login();
    await waitText("首次登录");await page.fill('input[name="password"]',"Prism-demo-2026");
    await page.fill('loc=css:input[autocomplete="new-password"] >> nth=0',"D-password-2026-new");
    await page.fill('loc=css:input[autocomplete="new-password"] >> nth=1',"D-password-2026-new");
    await page.keyboard.press("Escape");await page.click('loc=css:.unlock-card h1');
    await page.click('loc=role:button[name="保存并重新登录"]');await waitText("密码已更新");
    await login("wrong-synthetic-password");await waitText("账号或密码不正确");
    assert.match(await text(),/密码已更新/u);record("mandatory password-change receipt survives failed login");
    await login("D-password-2026-new");await page.waitForSelector("main.canvas");
    await route("/settings");await page.click('loc=role:button[name="设置新密码"]');await waitText("当前密码");
    assert.doesNotMatch(await text(),/首次登录/u);record("voluntary password change remains reachable");await screenshot("password-change");
  } else if(scenario==="usage") {
    await reset();await mode("listOperationalUsage","estimate");await mode("listOperationalBilling","cost-pages");await route("/usage?range=all");
    await waitText("估算用量");await waitText("加载更多费用记录");
    const before=await page.evaluate(()=>document.querySelector(".usage-cost-table")?.textContent);
    await mode("listOperationalBilling","next-error");await page.click('loc=role:button[name="加载更多费用记录"]');await waitText("下一页读取失败");
    assert.equal(await page.evaluate(()=>document.querySelector(".usage-cost-table")?.textContent),before);
    record("cost cursor failure retains loaded sources");
    await mode("listOperationalBilling","cost-pages");await page.click('loc=role:button[name="重试下一页"]');await page.waitForFunction(()=>!document.body.textContent?.includes("下一页读取失败"));
    const cursors=(await calls("listOperationalBilling")).map(call=>call.cursor);
    assert.ok(cursors.filter(cursor=>cursor==="D-cost-page2").length>=2);record("cost recovery retries the failed opaque cursor");
    await mode("listOperationalUsage","error");await route("/billing");await route("/usage?range=all");await waitText("刷新失败");
    assert.match(await text(),/估算用量/u);assert.match(await text(),/上次成功读取/u);record("usage refresh retains values and evidence");
    await mode("listOperationalUsage","hold");await page.click('loc=css:.usage-page .read-status button');
    await page.waitForFunction(()=>Object.keys(window.CPAR_D_HTTP.release).includes("listOperationalUsage"));
    assert.equal(await page.evaluate(()=>document.querySelector(".usage-page .read-status button")?.disabled),true);
    await page.evaluate(()=>{delete window.CPAR_D_HTTP.modes.listOperationalUsage;window.CPAR_D_HTTP.release.listOperationalUsage();});
    await page.waitForFunction(()=>!document.querySelector(".usage-page .read-status"));record("retry shows busy state and prevents duplicate reads");
    await route("/usage?range=all&model=retained-filter&protocol=openai_responses");await waitText("清除(2)");
    await page.click('loc=role:button[name="清除(2)"]');
    await page.waitForFunction(()=>location.hash==="#/usage?range=all"&&document.querySelector('select[name="protocol"]')?.value===""&&document.querySelector('input[name="model"]')?.value==="");
    assert.equal(await page.evaluate(()=>document.querySelector('select[name="protocol"]')?.value),"");
    assert.equal(await page.evaluate(()=>document.querySelector('input[name="model"]')?.value),"");record("cleared filters update visible form and URL");
    await mode("summarizeRequests","request-evidence");await route("/monitoring?tab=requests");await page.waitForSelector(".request-table tbody button");
    await page.click('loc=css:.request-table tbody tr:first-child button');await page.waitForSelector('[role="dialog"]');
    await page.click('loc=css:[role="dialog"] summary:has-text("Token 用量")');await waitText("输入已包含缓存 token");record("request details separate usage source and pricing");
    await page.click('loc=role:button[name="关闭"]');await mode("listOperationalBilling","ledger-evidence");await route("/monitoring?tab=ledger");await waitText("输入已包含缓存 token");record("ledger displays usage provenance and input accounting");
  } else if(scenario==="runtime") {
    await reset();await mode("getRuntimeAvailability","error");await route("/runtime");await waitText("synthetic_read_failed");
    assert.doesNotMatch(await text(),/加载可用性投影/u);record("initial runtime failure has recoverable error state");
    await mode("getRuntimeAvailability",undefined);await page.click('loc=role:button[name="重新读取"]');await waitText("恢复探测");
    await mode("getRuntimeAvailability","error");await page.click('loc=role:button[name="立即刷新"]');await waitText("刷新失败");
    assert.match(await text(),/恢复探测/u);record("runtime refresh retains last availability matrix");
    await mode("getRuntimeAvailability",undefined);await mode("listProviderAccountPools","pool-pages");await route("/runtime?account_id=cred-relay-spare");await waitText("已载入页尚无匹配账号");
    await page.click('loc=role:button[name="加载更多账号"]');await page.waitForSelector('tr[data-account-id="cred-relay-spare"]');record("account lookup continues beyond an empty first page");
    await mode("applyProviderAccountPoolAction","lost");await page.click('loc=css:tr[data-account-id="cred-relay-spare"] button:has-text("冷却")');await page.click('loc=role:button[name="确认冷却"]');await waitText("操作结果未确认");
    assert.equal((await calls("applyProviderAccountPoolAction")).length,1);
    assert.equal(await page.evaluate(()=>document.querySelector('tr[data-account-id="cred-relay-spare"] button')?.disabled),true);record("lost pool write is not replayed and requires readback");
    await mode("listProviderAccountPools","error");await page.click('loc=role:button[name="读取账号状态后确认下一次操作"]');await waitText("synthetic_read_failed");
    assert.equal(await page.evaluate(()=>document.querySelector('tr[data-account-id="cred-relay-spare"] button')?.disabled),true);record("failed reconciliation keeps pool actions disabled");
  } else if(scenario==="maintenance") {
    await reset();await mode("listManagementResourceAuditEvents","resource-next-error");await route("/audit");await waitText("后续页仍待核对");
    await page.click('loc=role:button[name="加载更早记录"]');await waitText("下一页读取失败");
    await mode("listManagementResourceAuditEvents","resource-page");await page.click('loc=role:button[name="重试下一页"]');await waitText("upstream_updated");record("audit empty page retains continuation and cursor recovery");
    await mode("previewBackup","error");await page.click('loc=role:button[name="源库备份预检"]');await waitText("D controlled source unavailable");
    await mode("previewBackup",undefined);await page.click('loc=role:button[name="源库备份预检"]');await waitText("源库预检已确认");
    assert.doesNotMatch(await text(),/D controlled source unavailable/u);record("successful backup preflight clears prior attempt error");
    await mode("previewBackup","error");await page.click('loc=role:button[name="源库备份预检"]');await waitText("保留上次成功预检");assert.match(await text(),/预检不会生成备份工件/u);record("preflight failure retains dated prior receipt");
    await mode("getSystemInformation","paused");await route("/settings");await waitText("新请求已暂停");
    await mode("getSystemInformation","error");await page.click('loc=css:.system-information button:has-text("刷新")');await waitText("刷新失败");
    assert.match(await text(),/新请求已暂停/u);assert.equal(await page.evaluate(()=>[...document.querySelectorAll("button")].find(button=>button.textContent==="应用运行配置")?.disabled),true);record("failed system refresh retains observed admission state with action gated");
    await route("/settings?focus=search");await page.fill('input[aria-label="搜索栏目"]',"D-no-section-match");await waitText("没有匹配的栏目");await page.click('loc=role:button[name="清除搜索"]');
    assert.equal(await page.evaluate(()=>document.activeElement?.getAttribute("aria-label")),"搜索栏目");record("settings no-match recovery restores focus");
    await route("/versions");await page.waitForSelector('loc=role:button[name="查看历史差异"]');await page.click('loc=role:button[name="查看历史差异"]');
    await page.waitForFunction(()=>document.activeElement?.textContent==="历史配置差异");await page.keyboard.press("Escape");await page.waitForFunction(()=>document.activeElement?.textContent==="查看历史差异");record("historical difference view restores opener focus");
  } else if(scenario==="catalog") {
    await reset();await prepareTarget();await mode("listCatalogModels","catalog-evidence");await route("/catalog?endpoint_id=ep-relay-a-responses&credential_id=cred-relay-key");await waitText("目录观测已陈旧");
    await page.click('loc=css:input[aria-label="选择 d-source-model"]');
    const selected=()=>page.evaluate(()=>document.querySelector('input[aria-label="选择 d-source-model"]')?.checked);
    await mode("listCatalogModels","error");await page.click('loc=role:button[name="重读目录"]');await waitText("刷新失败");
    assert.equal(await selected(),true);assert.match(await text(),/d-source-model/u);record("failed catalog read retains selection and observed source");
    await mode("listCatalogModels","catalog-evidence");await page.click('loc=role:button[name="重读目录"]');await page.waitForFunction(()=>!document.querySelector(".upstream-model-browser .read-status"));
    assert.equal(await selected(),true);record("successful read-only catalog refresh preserves valid selection");
    await mode("refreshCatalogModels","lost");await page.click('loc=role:button[name="刷新上游模型"]');await waitText("结果未确认时请重读目录");
    const refreshDisabled=()=>page.evaluate(()=>[...document.querySelectorAll(".upstream-model-browser button")].find(button=>button.textContent==="刷新上游模型")?.disabled);
    assert.equal(await refreshDisabled(),true);assert.equal((await calls("refreshCatalogModels")).length,1);record("lost catalog refresh response blocks replay");
    await mode("listCatalogModels","error");await page.click('loc=role:button[name="重读目录"]');await waitText("刷新失败");assert.equal(await refreshDisabled(),true);record("failed catalog readback leaves write gate closed");
    await mode("listCatalogModels","catalog-expired");await page.click('loc=role:button[name="重读目录"]');await waitText("目录已过期");
    assert.equal(await refreshDisabled(),false);assert.equal(await page.evaluate(()=>document.querySelector('input[aria-label="选择 d-source-model"]')?.disabled),true);record("hard-expired directory blocks admission after successful readback");await screenshot("catalog-evidence");
  } else if(scenario==="pin-uncertain") {
    await reset();await prepareTarget();await mode("executeChannelPin","pin-receipt");await route("/runtime");
    for(const field of ["provider_id","channel_id","route_id","credential_id"]) {
      await page.waitForFunction(field=>[...document.querySelectorAll(`select[name="${field}"] option`)].some(option=>option.value),field);
      const value=await page.evaluate(field=>[...document.querySelectorAll(`select[name="${field}"] option`)].find(option=>option.value)?.value,field);
      await page.selectOption(`select[name="${field}"]`,value);
    }
    await page.fill('input[name="client_key_id"]',"d-key");await page.fill('input[name="requested_model"]',"d-model");
    await page.click('loc=role:button[name="发一次真实请求"]');await waitText("d-pin-receipt");
    const prior=await page.evaluate(()=>document.querySelector(".rt-pin-receipt")?.textContent);
    await mode("executeChannelPin","lost");await page.click('loc=role:button[name="发一次真实请求"]');await waitText("本次调用结果未确认");
    const blocked=()=>page.evaluate(()=>document.querySelector('input[name="client_key_id"]')?.form?.querySelector('button[type="submit"]')?.disabled);
    assert.equal(await blocked(),true);assert.equal((await calls("executeChannelPin")).length,2);assert.equal(await page.evaluate(()=>document.querySelector(".rt-pin-receipt")?.textContent),prior);record("lost pin response retains prior receipt and blocks automatic repeat");
    await mode("getRuntimeAvailability","error");await page.click('loc=role:button[name="读取当前目标后确认新的诊断"]');await waitText("目标读取失败");assert.equal(await blocked(),true);record("failed target readback leaves pin disabled");
    await mode("getRuntimeAvailability",undefined);await page.click('loc=role:button[name="读取当前目标后确认新的诊断"]');await page.waitForFunction(()=>document.querySelector('input[name="client_key_id"]')?.form?.querySelector('button[type="submit"]')?.disabled===false);
    assert.equal((await calls("executeChannelPin")).length,2);record("successful target readback permits a separate deliberate diagnostic without replay");
  } else if(scenario==="recovery-uncertain") {
    await reset();await mode("requestQuotaRecovery","recovery-receipt");await route("/runtime");await page.waitForSelector('loc=css:button:has-text("发起恢复") >> nth=0');
    await page.click('loc=css:button:has-text("发起恢复") >> nth=0');await page.waitForSelector('.rt-chip[data-state="probe_scheduled"]');
    await mode("requestQuotaRecovery","lost");await page.click('loc=css:button:has-text("发起恢复") >> nth=0');await waitText("本次结果未确认");
    const blocked=()=>page.evaluate(()=>[...document.querySelectorAll("button")].find(button=>button.textContent==="发起恢复")?.disabled);
    assert.equal(await blocked(),true);assert.equal((await calls("requestQuotaRecovery")).length,2);assert.match(await text(),/已排程探测/u);record("lost recovery response retains prior scheduling fact without claiming recovery");
    await mode("getRuntimeAvailability","error");await page.click('loc=role:button[name="读取状态后核对"]');await waitText("状态核对失败");assert.equal(await blocked(),true);record("failed recovery readback prevents repeated mutation");
    await mode("getRuntimeAvailability",undefined);await page.click('loc=role:button[name="读取状态后核对"]');await page.waitForFunction(()=>[...document.querySelectorAll("button")].find(button=>button.textContent==="发起恢复")?.disabled===false);
    assert.equal((await calls("requestQuotaRecovery")).length,2);record("successful state readback releases a deliberate next action without replay");
    await mode("getRuntimeAvailability","recovered");await page.click('loc=role:button[name="立即刷新"]');await waitText("当前没有可发起恢复的组合");
    assert.ok(await page.evaluate(()=>!!document.querySelector('.rt-chip[data-state="probe_scheduled"]')));assert.match(await text(),/本次结果未确认/u);record("recovery outcomes remain visible after targets stop qualifying");
    await mode("getRuntimeAvailability","no-availability");await page.click('loc=role:button[name="立即刷新"]');await waitText("本次未观测到，保留上次结果");record("recovery identity and receipt survive target disappearance");
    await mode("getRuntimeAvailability","error");await page.click('loc=role:button[name="立即刷新"]');await waitText("刷新失败");assert.ok(await page.evaluate(()=>!!document.querySelector('.rt-chip[data-state="probe_scheduled"]')));record("later failed reads retain recovery evidence for missing targets");
  } else if(scenario==="apply-uncertain") {
    await reset();await mode("getSystemInformation","paused");await route("/settings");await waitText("新请求已暂停");
    await mode("applyRuntimeConfiguration","lost");await page.click('loc=role:button[name="应用运行配置"]');await waitText("应用结果未确认");
    const blocked=()=>page.evaluate(()=>[...document.querySelectorAll("button")].find(button=>button.textContent==="应用运行配置")?.disabled);
    assert.equal(await blocked(),true);assert.equal((await calls("applyRuntimeConfiguration")).length,1);record("unknown runtime apply requires explicit readback before another write");
    await mode("getSystemInformation","error");await page.click('loc=role:button[name="读取服务状态后核对"]');await waitText("服务状态核对失败");assert.equal(await blocked(),true);record("failed application readback keeps write gate closed");
    await mode("getSystemInformation","paused");await page.click('loc=role:button[name="读取服务状态后核对"]');await page.waitForFunction(()=>[...document.querySelectorAll("button")].find(button=>button.textContent==="应用运行配置")?.disabled===false);
    assert.equal((await calls("applyRuntimeConfiguration")).length,1);assert.match(await text(),/先前应用请求的结果仍未确认/u);record("successful service observation permits a deliberate new apply without manufacturing a receipt");
  } else if(scenario==="i18n") {
    await reset();await mode("listOperationalUsage","estimate");await mode("summarizeRequests","request-evidence");await route("/settings");await page.click('loc=role:radio[name="English"]');
    await page.evaluate(()=>location.hash="#/usage?range=all");await waitText("Estimated usage");assert.doesNotMatch(await text(),/估算用量/u);record("aggregated provenance follows actual language selection");
    await page.cdp("Emulation.setDeviceMetricsOverride",{width:375,height:812,deviceScaleFactor:1,mobile:false});assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));record("English usage evidence remains contained on narrow layout");
    await page.evaluate(()=>location.hash="#/monitoring?tab=requests");await page.waitForSelector(".request-table tbody button");await page.click('loc=css:.request-table tbody tr:first-child button');await page.waitForSelector('[role="dialog"]');await page.click('loc=css:[role="dialog"] summary:has-text("Token 用量")');await waitText("Input includes cached tokens");
    assert.match(await page.evaluate(()=>document.querySelector('[role="dialog"]')?.textContent??""),/Estimated usage/u);record("request source and input accounting update to English");await page.keyboard.press("Escape");
    await mode("listProviderAccountFailures","empty-items");await page.evaluate(()=>location.hash="#/monitoring?tab=failures&account_id=missing-d-account");await waitText("No failure attempts match the applied filters.");assert.doesNotMatch(await text(),/该配置版本下没有归因|没有符合筛选条件/u);record("filtered failure no-match follows English selection");
    await page.evaluate(()=>location.hash="#/monitoring?tab=failures");await waitText("This configuration version has no failure attempts attributed to accounts.");record("unfiltered empty failure state remains distinct in English");
    await page.evaluate(()=>location.hash="#/settings");await page.waitForSelector('loc=role:radio[name="中文"]');await page.click('loc=role:radio[name="中文"]');
  } else if(scenario==="details") {
    await reset();await mode("summarizeRequests","request-evidence");
    for(const width of [1440,375])for(const theme of ["light","dark"]) {
      await appearance(width,theme);
      await route("/monitoring?tab=requests");await page.waitForSelector(".request-table tbody button");await page.click('loc=css:.request-table tbody tr:first-child button');await page.waitForSelector('[role="dialog"]');
      await page.click('loc=css:[role="dialog"] summary:has-text("Token 用量")');await waitText("输入已包含缓存 token");
      const box=await page.evaluate(()=>{const dialog=document.querySelector('[role="dialog"]'),box=dialog.getBoundingClientRect();return {left:box.left,right:box.right,documentWidth:document.documentElement.scrollWidth,focusWithin:dialog.contains(document.activeElement)};});
      assert.ok(box.left>=-1&&box.right<=width+1&&box.documentWidth<=width+1);assert.equal(box.focusWithin,true);await screenshot(`request-detail-${width}-${theme}`);await page.keyboard.press("Escape");await page.waitForFunction(()=>!document.querySelector('[role="dialog"]'));record("request detail fits viewport with keyboard dismissal",{width,theme});
      await route("/versions");await page.click('loc=role:button[name="查看历史差异"]');await page.waitForFunction(()=>document.activeElement?.textContent==="历史配置差异");
      assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));await screenshot(`historical-diff-${width}-${theme}`);await page.keyboard.press("Escape");await page.waitForFunction(()=>document.activeElement?.textContent==="查看历史差异");record("history workspace retains focus return in both layouts",{width,theme});
    }
  } else if(scenario==="models-routes") {
    await reset();await prepareTarget();await route("/versions");await readyButton("编辑当前配置");await page.click('loc=role:button[name="编辑当前配置"]');await page.click('loc=role:button[name="创建草稿"]');
    await page.waitForSelector('loc=role:button[name="接续草稿"]');await page.click('loc=role:button[name="接续草稿"]');await page.waitForFunction(()=>!document.querySelector('[role="dialog"]'));await route("/models");await page.waitForSelector('loc=role:button[name="管理连接"]');
    assert.match(await text(),/正在编辑草稿/u);record("model maintenance starts from a publicly created draft");
    await page.fill('input[aria-label="搜索已接入模型"]',"D-no-model");await waitText("没有匹配的模型");await page.click('loc=role:button[name="清除筛选"]');await page.waitForSelector('loc=role:button[name="管理连接"]');record("model no-match search recovers through visible controls");
    await page.fill('input[aria-label="搜索已接入模型"]',"minimax");await page.selectOption('select[aria-label="模型提供商"]',"relay-a");await route("/billing");await route("/models");
    assert.equal(await page.evaluate(()=>document.querySelector('input[aria-label="搜索已接入模型"]')?.value),"minimax");assert.equal(await page.evaluate(()=>document.querySelector('select[aria-label="模型提供商"]')?.value),"relay-a");record("model source and search filters survive navigation return");
    await page.click('loc=role:button[name="管理连接"]');await page.waitForSelector(".model-source-card");await page.click('loc=role:button[name="编辑路径"]');await page.fill('loc=role:spinbutton[name="权重"]',"2");
    await page.click('loc=role:button[name="返回来源"]');await page.waitForSelector('[role="alertdialog"]');await page.click('loc=role:button[name="继续编辑"]');
    assert.equal(await page.evaluate(()=>document.querySelector('[role="dialog"] input[max="10000"]')?.value),"2");await page.click('loc=role:button[name="返回来源"]');await page.click('loc=role:button[name="放弃修改"]');await page.waitForSelector(".model-source-card");
    assert.equal((await calls("updateRouteCandidate")).length,0);record("source cancel keeps edits until explicit discard without writes");
    await page.click('loc=role:button[name="编辑路径"]');await page.fill('loc=role:spinbutton[name="权重"]',"2");await page.click('loc=role:button[name="保存并应用"]');await page.waitForSelector('.operation-receipt');
    assert.match(await page.evaluate(()=>document.querySelector('.operation-receipt')?.textContent??""),/已保存到当前草稿，尚未应用/u);assert.equal((await calls("updateRouteCandidate")).length,1);
    await page.click('loc=role:button[name="核对配置"]');await page.waitForFunction(()=>!document.querySelector('[role="dialog"], [role="alertdialog"]'));
    await page.click('loc=role:button[name="管理连接"]');await page.waitForSelector(".model-source-card");assert.ok(await page.evaluate(()=>[...document.querySelectorAll('.source-scheduling > div')].some(node=>node.querySelector('dt')?.textContent==='权重'&&node.querySelector('dd')?.textContent==='2')));
    await page.click('loc=role:button[name="关闭"]');record("source save returns to the draft model with confirmed weight");
    await page.click('loc=css:.row-menu > summary');await page.click('loc=role:button[name="编辑模型"]');await page.fill('loc=role:textbox[name="管理端显示名"]',"D reviewed model");await page.click('loc=role:button[name="取消"]');await page.waitForSelector('[role="alertdialog"]');await page.click('loc=role:button[name="放弃修改"]');await page.waitForFunction(()=>!document.querySelector('[role="dialog"]'));
    assert.equal((await calls("updatePublicModel")).length,0);assert.doesNotMatch(await text(),/D reviewed model/u);record("public model cancel preserves the original display name");
    await page.click('loc=role:button[name="编辑模型"]');await page.fill('loc=role:textbox[name="管理端显示名"]',"D reviewed model");await page.click('loc=role:button[name="保存"]');await waitText("模型配置结果");await page.click('loc=role:button[name="完成"]');await page.waitForFunction(()=>!document.querySelector('[role="dialog"]'));await waitText("D reviewed model");assert.equal((await calls("updatePublicModel")).length,1);record("public model save confirms draft state before returning");
    await page.click('loc=css:.models-advanced > summary');await page.waitForSelector('loc=role:button[name="打开路由"]');await page.click('loc=role:button[name="打开路由"]');await page.waitForSelector('loc=role:button[name="编辑路由"]');await page.click('loc=role:button[name="编辑路由"]');
    await page.fill('loc=role:spinbutton[name="最大尝试次数"]',"2");await page.keyboard.press("Escape");await page.waitForSelector('[role="alertdialog"]');await page.click('loc=role:button[name="继续编辑"]');
    assert.equal(await page.evaluate(()=>document.querySelector('#route-action-form input')?.value),"2");await page.click('loc=role:button[name="取消"]');await page.click('loc=role:button[name="放弃修改"]');await page.waitForFunction(()=>!document.querySelector('[role="dialog"]'));assert.equal((await calls("updateRoute")).length,0);record("route Escape and cancel share the discard gate without writes");
    await page.click('loc=role:button[name="编辑路由"]');await page.fill('loc=role:spinbutton[name="最大尝试次数"]',"2");await mode("updateRoute","invalid");await page.click('loc=role:button[name="保存路由"]');await waitText("D controlled write rejected");
    assert.equal(await page.evaluate(()=>document.querySelector('#route-action-form input')?.value),"2");assert.equal((await calls("updateRoute")).length,1);record("known route rejection keeps the editable form and target");
    await mode("updateRoute",null);await page.click('loc=role:button[name="保存路由"]');await waitText("路由配置结果");assert.match(await page.evaluate(()=>document.querySelector('[role="dialog"]')?.textContent??""),/已保存到当前草稿，尚未发布/u);await page.click('loc=role:button[name="完成"]');await page.waitForFunction(()=>!document.querySelector('[role="dialog"]'));await waitText("路由参数已保存到草稿");record("explicit route retry confirms draft write and returns to workbench");
    for(const width of [1440,375])for(const theme of ["light","dark"]) {
      await appearance(width,theme);
      await page.click('loc=role:button[name="编辑路由"]');await page.waitForSelector('#route-action-form');assert.equal(await page.evaluate(()=>document.querySelector('#route-action-form input')?.value),"2");
      await assertDialogFits();
      await screenshot(`route-edit-${width}-${theme}`);await page.keyboard.press("Escape");await page.waitForFunction(()=>!document.querySelector('[role="dialog"]')&&document.activeElement?.textContent==='编辑路由');record("route editor fits both layouts and themes with focus return",{width,theme});
      await page.click('loc=role:button[name="管理连接"]');await page.waitForSelector(".model-source-card");await page.click('loc=role:button[name="编辑路径"]');
      await assertDialogFits();
      await screenshot(`model-source-edit-${width}-${theme}`);await page.keyboard.press("Escape");await page.waitForSelector('.model-source-card');await page.click('loc=role:button[name="关闭"]');await page.waitForFunction(()=>!document.querySelector('[role="dialog"]')&&document.activeElement?.textContent==='管理连接');record("model source editor fits and returns without changes",{width,theme});
    }
    await desktop();await page.click('loc=role:button[name="编辑路由"]');await page.fill('loc=role:spinbutton[name="最大尝试次数"]',"3");await mode("updateRoute","lost-after-write");await page.click('loc=role:button[name="保存路由"]');await waitText("修改结果未确认");
    assert.equal((await calls("updateRoute")).length,3);assert.equal(await page.evaluate(()=>window.CPAR_D_HTTP.applied.filter(row=>row.operation==='updateRoute').length),1);assert.equal(await page.evaluate(()=>!!document.querySelector('#route-action-form')),false);record("applied-then-lost fixture write is uncertain and cannot be replayed");
    await page.click('loc=role:button[name="核对草稿"]');await route("/versions");await page.waitForFunction(()=>!document.querySelector('[role="dialog"]')&&!document.querySelector('#app[inert]'));await readyButton("接续待应用修改");await page.click('loc=role:button[name="接续待应用修改"]');await page.waitForSelector('loc=role:button[name="接续草稿"]');await page.click('loc=role:button[name="接续草稿"]');await page.waitForFunction(()=>!document.querySelector('[role="dialog"]'));await route("/models");
    await page.click('loc=css:.models-advanced > summary');await page.click('loc=role:button[name="打开路由"]');await page.click('loc=role:button[name="编辑路由"]');await page.waitForSelector('#route-action-form');assert.equal(await page.evaluate(()=>document.querySelector('#route-action-form input')?.value),"3");assert.equal((await calls("updateRoute")).length,3);await page.keyboard.press("Escape");record("public draft readback observes the applied value without resubmitting");
  } else if(scenario==="model-connections") {
    await reset();await prepareTarget();await mode("listRoutes","error");await route("/models");
    await page.waitForSelector('loc=role:button[name="管理连接"]');await page.click('loc=role:button[name="管理连接"]');await waitText("synthetic_read_failed");
    const dialogText=()=>page.evaluate(()=>document.querySelector('[role="dialog"]')?.textContent??"");
    assert.doesNotMatch(await dialogText(),/尚未连接提供商/u);record("initial topology failure stays unknown rather than empty");
    await mode("listRoutes",null);await page.click('loc=css:[role="dialog"] .read-status button');await page.waitForSelector(".model-source-card");
    assert.match(await dialogText(),/Fixture API/u);assert.doesNotMatch(await dialogText(),/synthetic_read_failed|尚未连接提供商/u);record("successful retry restores observed model sources");
    await page.click('loc=role:button[name="关闭"]');await mode("listRoutes","error");await route("/billing");await route("/models");await waitText("synthetic_read_failed");await page.click('loc=role:button[name="管理连接"]');
    assert.match(await dialogText(),/Fixture API/u);assert.match(await dialogText(),/上次成功读取/u);assert.doesNotMatch(await dialogText(),/尚未连接提供商/u);record("failed topology refresh retains the dated source snapshot");
    await reset();await prepareTarget();await mode("listRouteCandidates","empty-items");await route("/models");await page.click('loc=role:button[name="管理连接"]');await waitText("尚未连接提供商");
    assert.doesNotMatch(await dialogText(),/synthetic_read_failed/u);record("successfully observed empty topology has a genuine empty state");
  } else if(scenario==="failures") {
    await reset();await mode("listProviderAccountFailures","hold");await route("/monitoring?tab=failures");
    await page.waitForFunction(()=>!!window.CPAR_D_HTTP.release.listProviderAccountFailures);
    assert.equal(await page.evaluate(()=>[...document.querySelectorAll("button")].find(button=>button.textContent==="重新读取失败记录")?.disabled),true);
    assert.equal(await page.evaluate(()=>!!document.querySelector(".mon-table")),false);record("failure workspace distinguishes loading and prevents duplicate reads");
    await page.evaluate(()=>{delete window.CPAR_D_HTTP.modes.listProviderAccountFailures;window.CPAR_D_HTTP.release.listProviderAccountFailures();});await page.waitForSelector(".mon-table tbody tr");
    assert.match(await text(),/已加载失败尝试/u);record("failure records are attributed attempts rather than request totals");
    await page.selectOption('select[name="account_id"]',{label:"指定历史资源…"});await page.fill('input[aria-label="历史资源引用"]',"acct-0");await page.click('loc=role:button[name="使用此引用"]');await page.click('loc=role:button[name="应用筛选"]');
    await page.waitForFunction(()=>location.hash.includes("account_id=acct-0")&&document.querySelector(".mon-table tbody tr"));
    await page.click('loc=css:.mon-table tbody tr:first-child .linklike');await page.waitForSelector('[role="dialog"] .request-attempts');
    assert.match(await page.evaluate(()=>document.querySelector('[role="dialog"]')?.textContent??""),/尝试 1/u);await page.keyboard.press("Escape");
    assert.equal(await page.evaluate(()=>document.querySelector('select[name="account_id"]')?.value),"acct-0");record("failure filters and attempt detail survive keyboard return");
    await page.click('loc=css:.mon-table tbody tr:first-child a');await page.waitForFunction(()=>location.hash.startsWith("#/runtime?")&&location.hash.includes("credential_id=acct-0"));await page.evaluate(()=>history.back());
    await page.waitForFunction(()=>location.hash.includes("tab=failures")&&location.hash.includes("account_id=acct-0"));await page.waitForSelector(".mon-table tbody tr");record("exact binding diagnosis returns to the filtered failure task");
    await page.click('loc=role:button[name="清除"]');await page.waitForFunction(()=>location.hash==="#/monitoring?tab=failures"&&document.querySelector('select[name="account_id"]')?.value==="");
    await page.selectOption('select[name="account_id"]',{label:"指定历史资源…"});await page.fill('input[aria-label="历史资源引用"]',"missing-d-account");await page.click('loc=role:button[name="使用此引用"]');await page.click('loc=role:button[name="应用筛选"]');
    await page.waitForFunction(()=>location.hash.includes("account_id=missing-d-account")&&!!document.querySelector('.monitoring-page [data-kind="empty"]'));
    assert.match(await text(),/没有符合筛选条件的失败尝试/u);
    assert.doesNotMatch(await text(),/该配置版本下没有归因到账号的失败尝试/u);record("failure no-match state does not claim an empty configuration");
    await page.click('loc=role:button[name="清除"]');await page.waitForSelector(".mon-table tbody tr");
    const prior=await page.evaluate(()=>document.querySelector(".mon-table tbody")?.textContent);
    await mode("listProviderAccountFailures","error");await page.click('loc=role:button[name="重新读取失败记录"]');await waitText("刷新失败");
    assert.equal(await page.evaluate(()=>document.querySelector(".mon-table tbody")?.textContent),prior);assert.match(await text(),/上次成功读取/u);record("failure refresh error preserves dated attribution rows");
    await mode("listProviderAccountFailures","hold");await page.evaluate(()=>{delete window.CPAR_D_HTTP.release.listProviderAccountFailures;});await page.click('loc=css:.monitoring-page .read-status button');
    await page.waitForFunction(()=>!!window.CPAR_D_HTTP.release.listProviderAccountFailures);
    assert.equal(await page.evaluate(()=>document.querySelector('.monitoring-page .read-status button')?.disabled),true);
    assert.equal(await page.evaluate(()=>document.querySelector(".mon-table tbody")?.textContent),prior);
    await page.evaluate(()=>{delete window.CPAR_D_HTTP.modes.listProviderAccountFailures;window.CPAR_D_HTTP.release.listProviderAccountFailures();});await page.waitForFunction(()=>!document.querySelector('.monitoring-page .read-status'));record("failure retry remains busy with retained rows and recovers");
    await reset();await mode("listProviderAccountFailures","denied");await route("/monitoring?tab=failures");await waitText("synthetic_permission_denied");
    assert.equal(await page.evaluate(()=>!!document.querySelector('.monitoring-page [data-kind="empty"], .mon-table')),false);record("failure permission denial does not become empty data");
    await mode("listProviderAccountFailures","error");await page.click('loc=css:.monitoring-page .read-status button');await waitText("synthetic_read_failed");
    assert.equal(await page.evaluate(()=>!!document.querySelector('.monitoring-page [data-kind="empty"], .mon-table')),false);record("initial failure read error stays unknown");
    await mode("listProviderAccountFailures","empty-items");await page.click('loc=css:.monitoring-page .read-status button');await waitText("该配置版本下没有归因到账号的失败尝试");
    assert.doesNotMatch(await text(),/没有符合筛选条件/u);record("successful unfiltered empty failure data is explicit");
    await reset();await mode("listProviderAccountFailures","failure-pages");await route("/monitoring?tab=failures");await page.waitForSelector(".mon-table tbody tr");
    assert.equal(await page.evaluate(()=>document.querySelectorAll('.mon-table tbody tr').length),1);await page.waitForSelector('loc=role:button[name="再读一页"]');
    await mode("listProviderAccountFailures","next-error");await page.click('loc=role:button[name="再读一页"]');await waitText("下一页读取失败");
    assert.equal(await page.evaluate(()=>document.querySelectorAll('.mon-table tbody tr').length),1);record("failure cursor error retains a partial loaded projection");
    await mode("listProviderAccountFailures","failure-pages");await page.click('loc=role:button[name="重试下一页"]');await page.waitForFunction(()=>document.querySelectorAll('.mon-table tbody tr').length===2&&!document.querySelector('.monitoring-page .read-status'));
    assert.ok((await calls("listProviderAccountFailures")).filter(call=>call.cursor==="D-failure-page2").length>=2);record("failure retry resumes the same opaque cursor");
    for(const width of [1440,375])for(const theme of ["light","dark"]) {
      await appearance(width,theme);
      assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));await screenshot(`failures-${width}-${theme}`);
      await page.click('loc=css:.mon-table tbody tr:first-child .linklike');await page.waitForSelector('[role="dialog"] .request-attempts');
      await assertDialogFits();
      await screenshot(`failure-attempts-${width}-${theme}`);await page.keyboard.press("Escape");await page.waitForFunction(()=>!document.querySelector('[role="dialog"]'));record("failure projection and attempts fit with keyboard return",{width,theme});
    }
  } else if(scenario==="permissions-egress") {
    await reset();await mode("listAccessGroupRoutes","denied");await route("/access");
    await page.click('loc=css:summary:has-text("高级访问组")');await page.click('loc=css:button:has-text("路由") >> nth=0');await waitText("synthetic_permission_denied");assert.doesNotMatch(await text(),/本次读取中该组未配置路由授权/u);record("permission denial is distinct from empty route grants");
    await mode("listCompatibleProxyPools","pool-record");await mode("listCompatibleProxyNodes","error");await mode("listUpstreams","error");await route("/egress");await waitText("未确认");
    assert.doesNotMatch(await text(),/这个池还没有节点/u);record("failed dependency reads remain unknown rather than zero");
  } else throw new Error(`Unknown scenario: ${scenario}`);
  const result={scenario,result:"PASS",commit,environment:"local-synthetic",browser:"EgoLite",spaceId,checks};
  await writeFile(`${outputDir}/${scenario}.json`,JSON.stringify(result,null,2)+"\n");
  console.log(JSON.stringify({scenario,result:"PASS",checks:checks.length,commit,spaceId}));
} catch(error) {
  await screenshot(`${scenario}-failure`);
  await writeFile(`${outputDir}/${scenario}.json`,JSON.stringify({scenario,result:"FAIL",commit,environment:"local-synthetic",spaceId,checks,error:error.message},null,2)+"\n");
  console.log((await page.snapshot()).slice(0,9000));
  throw error;
}
