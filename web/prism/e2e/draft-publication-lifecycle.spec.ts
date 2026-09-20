import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

test("pending validation owns one modal through a successful result",async({page})=>{
  await unlock(page);
  await selectDraft(page);
  await navigate(page,"访问控制");
  await page.evaluate(async()=>{
    const {useVersionStore}=await import("/src/features/config-versions/versionStore.ts");
    const {holdFixtureOperationForTest}=await import("/src/dev/fixtures.ts");
    const id=useVersionStore.getState().context?.configVersionId;
    if(!id)throw new Error("missing selected draft");
    holdFixtureOperationForTest(`POST /admin/config-versions/${id}/validate`);
    Reflect.set(globalThis,"__heldValidationId",id);
  });
  await page.locator(".dock").getByRole("button",{name:"验证"}).click();
  const loading=page.getByRole("dialog",{name:"正在验证草稿"});
  await expect(loading).toBeVisible();
  await expect(page.getByRole("dialog")).toHaveCount(1);
  await expect(page.getByRole("button",{name:"创建客户端密钥"}).click({timeout:800})).rejects.toThrow();
  await expect(page.getByRole("dialog",{name:"创建 API 密钥"})).toHaveCount(0);
  await page.evaluate(async()=>{
    const {releaseFixtureOperationForTest}=await import("/src/dev/fixtures.ts");
    releaseFixtureOperationForTest(`POST /admin/config-versions/${Reflect.get(globalThis,"__heldValidationId")}/validate`);
  });
  const result=page.getByRole("dialog",{name:"验证结果"});
  await expect(result).toBeVisible();
  await expect(page.getByRole("dialog")).toHaveCount(1);
  await result.getByRole("button",{name:"关闭",exact:true}).click();
  await page.getByRole("button",{name:"创建客户端密钥"}).click();
  await expect(page.getByRole("dialog",{name:"创建 API 密钥"})).toBeVisible();
});

test("failed validation retires its loading modal and leaves the workspace usable",async({page})=>{
  await unlock(page);
  await selectDraft(page);
  await navigate(page,"访问控制");
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation!=="validateConfigVersion")return original.call(this,operation,request);
      await new Promise<void>(resolve=>Reflect.set(globalThis,"__releaseValidationFailure",resolve));
      return new Response(JSON.stringify({error:{code:"validation_unavailable",message:"synthetic validation unavailable"}}),{status:503,headers:{"Content-Type":"application/json"}});
    };
  });
  await page.locator(".dock").getByRole("button",{name:"验证"}).click();
  await expect(page.getByRole("dialog",{name:"正在验证草稿"})).toBeVisible();
  await expect.poll(()=>page.evaluate(()=>typeof Reflect.get(globalThis,"__releaseValidationFailure"))).toBe("function");
  await page.evaluate(()=>{const release=Reflect.get(globalThis,"__releaseValidationFailure") as (()=>void)|undefined;if(!release)throw new Error("validation did not start");release();});
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.locator(".dock-error")).toContainText("synthetic validation unavailable");
  await page.getByRole("button",{name:"创建客户端密钥"}).click();
  await expect(page.getByRole("dialog",{name:"创建 API 密钥"})).toBeVisible();
});

test("a draft forked from an older active version cannot be submitted as current",async({page})=>{
  await unlock(page);
  await selectDraft(page);
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__staleParentPublishCalls",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation==="publishConfigVersion")Reflect.set(globalThis,"__staleParentPublishCalls",Number(Reflect.get(globalThis,"__staleParentPublishCalls"))+1);
      const response=await original.call(this,operation,request);
      if(operation!=="listConfigVersions"||!response.ok)return response;
      const versions=await response.clone().json() as {id:string;status:string;parent_id?:string|null}[];
      return new Response(JSON.stringify(versions.map(version=>version.status==="draft"?{...version,parent_id:"older-active-version"}:version)),{status:response.status,headers:response.headers});
    };
  });
  await page.locator(".dock").getByRole("button",{name:"发布",exact:true}).click();
  const confirm=page.getByRole("dialog",{name:"确认发布"});
  await expect(confirm.getByRole("alert")).toContainText("较早的已发布配置");
  await expect(confirm.getByRole("button",{name:"确认发布"})).toBeDisabled();
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__staleParentPublishCalls")))).toBe(0);
});

test("an acknowledged publication survives a failed reread and is not replayed",async({page})=>{
  await unlock(page);
  await selectDraft(page);
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__publishAcks",0);
    Reflect.set(globalThis,"__failPublishedRead",true);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation==="publishConfigVersion"){
        Reflect.set(globalThis,"__publishAcks",Number(Reflect.get(globalThis,"__publishAcks"))+1);
        return original.call(this,operation,request);
      }
      if(operation==="listConfigVersions"&&Number(Reflect.get(globalThis,"__publishAcks"))>0&&Reflect.get(globalThis,"__failPublishedRead"))return new Response(JSON.stringify({error:{code:"read_unavailable",message:"synthetic version read unavailable"}}),{status:503,headers:{"Content-Type":"application/json"}});
      return original.call(this,operation,request);
    };
  });
  await page.locator(".dock").getByRole("button",{name:"发布",exact:true}).click();
  const confirm=page.getByRole("dialog",{name:"确认发布"});
  await expect(confirm.locator(".sheet-footer").getByRole("button",{name:"确认发布"})).toBeVisible();
  await confirm.getByRole("button",{name:"确认发布",exact:true}).click();
  const receipt=page.getByRole("dialog",{name:"配置已发布"});
  await expect(receipt).toContainText("发布已确认，但重读失败");
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__publishAcks")))).toBe(1);
  await page.evaluate(()=>Reflect.set(globalThis,"__failPublishedRead",false));
  await receipt.getByRole("button",{name:"重新读取活动配置"}).click();
  await expect(receipt.getByRole("button",{name:"完成"})).toBeVisible();
  await receipt.getByRole("button",{name:"完成"}).click();
  await expect(page.locator("main.canvas")).toHaveAttribute("data-context-status","active");
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__publishAcks")))).toBe(1);
});

test("a lost publication response becomes a review-only result",async({page})=>{
  await unlock(page);
  await selectDraft(page);
  await page.evaluate(async()=>{
    const {ManagementApi}=await import("/src/generated/management-client.ts");
    const original=ManagementApi.prototype.request;
    Reflect.set(globalThis,"__lostPublishCalls",0);
    ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){
      if(operation!=="publishConfigVersion")return original.call(this,operation,request);
      Reflect.set(globalThis,"__lostPublishCalls",Number(Reflect.get(globalThis,"__lostPublishCalls"))+1);
      await original.call(this,operation,request);
      throw new Error("synthetic publication response lost");
    };
  });
  await page.locator(".dock").getByRole("button",{name:"发布",exact:true}).click();
  await page.getByRole("dialog",{name:"确认发布"}).getByRole("button",{name:"确认发布",exact:true}).click();
  const receipt=page.getByRole("dialog",{name:"发布结果待核对"});
  await expect(receipt).toContainText("不会自动重放发布");
  await expect(receipt.getByRole("button",{name:"确认发布"})).toHaveCount(0);
  expect(await page.evaluate(()=>Number(Reflect.get(globalThis,"__lostPublishCalls")))).toBe(1);
});
