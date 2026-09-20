import {test,expect} from "@playwright/test";
import {unlock,selectDraft,navigate} from "./helpers";

test("policy inline editor saves a pending receipt and reloads its stored fields",async({page})=>{
 await unlock(page);await selectDraft(page);await navigate(page,"出口策略");
 await page.locator("tr",{hasText:"仅中转站"}).getByRole("button",{name:"编辑",exact:true}).click();
 const editor=page.locator(".inline-workspace");await expect(page.getByRole("dialog")).toHaveCount(0);await editor.getByLabel("名称",{exact:true}).fill("Managed egress policy");await editor.getByRole("button",{name:"保存",exact:true}).click();
 await expect(page.getByRole("region",{name:"出口策略修改结果"})).toContainText("尚未应用");await editor.getByRole("button",{name:"完成",exact:true}).click();
 await expect(page.locator("tr",{hasText:"Managed egress policy"})).toBeVisible();await expect(page.locator(".dock")).toContainText("待应用草稿");
});

test("lost node creation clears the secret and cannot replay from the editor",async({page})=>{
 await unlock(page);await selectDraft(page);await navigate(page,"出口策略");
 await page.evaluate(async()=>{const {ManagementApi}=await import("/src/generated/management-client.ts");const original=ManagementApi.prototype.request;Reflect.set(globalThis,"__nodeWriteCount",0);ManagementApi.prototype.request=async function(this:unknown,operation:string,request:unknown){const response=await original.call(this,operation,request);if(operation==="createCompatibleProxyNode"){Reflect.set(globalThis,"__nodeWriteCount",Number(Reflect.get(globalThis,"__nodeWriteCount"))+1);throw new Error("synthetic node response lost");}return response;};});
 await page.locator(".cp-section",{hasText:"代理节点"}).getByRole("button",{name:"新建",exact:true}).click();
 const editor=page.locator(".inline-workspace");await editor.getByLabel("资源",{exact:true}).fill("node-uncertain");await editor.getByLabel("upstream").selectOption("relay-a");await editor.getByLabel("名称",{exact:true}).fill("Uncertain node");await editor.getByLabel("proxy_endpoint").fill("socks5://127.0.0.1:1080");await editor.getByRole("button",{name:"创建",exact:true}).click();
 await expect(page.locator(".compatible-proxy")).toContainText("synthetic node response lost");await expect(editor.getByLabel("proxy_endpoint")).toHaveValue("");await expect(editor.getByRole("button",{name:"创建",exact:true})).toBeDisabled();expect(await page.evaluate(()=>Reflect.get(globalThis,"__nodeWriteCount"))).toBe(1);
});
