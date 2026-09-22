import { expect, test } from "@playwright/test";
import { navigate, selectDraft, unlock } from "./helpers";

test("model workspace keeps independent read filters across tabs and clears on scope change", async ({page}) => {
  await unlock(page);
  await selectDraft(page);
  await navigate(page,"模型目录");
  const browser=page.getByRole("region",{name:"上游模型清单"});
  await browser.getByRole("combobox",{name:"接口",exact:true}).selectOption("ep-relay-a-responses");
  await browser.getByRole("combobox",{name:"目录账号",exact:true}).selectOption("cred-relay-key");
  await browser.getByRole("searchbox",{name:"搜索上游模型"}).fill("Exact/Model");
  const tabs=page.getByRole("navigation",{name:"工作区页面"});
  await tabs.getByRole("link",{name:"已开放模型",exact:true}).click();
  await page.getByRole("searchbox",{name:"搜索已接入模型"}).fill("minimax");
  await tabs.getByRole("link",{name:"上游目录",exact:true}).click();
  await expect(browser.getByRole("combobox",{name:"接口",exact:true})).toHaveValue("ep-relay-a-responses");
  await expect(browser.getByRole("combobox",{name:"目录账号",exact:true})).toHaveValue("cred-relay-key");
  await expect(browser.getByRole("searchbox",{name:"搜索上游模型"})).toHaveValue("Exact/Model");
  await tabs.getByRole("link",{name:"已开放模型",exact:true}).click();
  await expect(page.getByRole("searchbox",{name:"搜索已接入模型"})).toHaveValue("minimax");
  await page.evaluate(async()=>{const {useVersionStore}=await import("/src/features/config-versions/versionStore.ts");useVersionStore.getState().reset();});
  await expect(page.getByRole("searchbox",{name:"搜索已接入模型"})).toHaveValue("");
  await tabs.getByRole("link",{name:"上游目录",exact:true}).click();
  await expect(browser.getByRole("combobox",{name:"接口",exact:true})).toHaveValue("");
  await expect(browser.getByRole("searchbox",{name:"搜索上游模型"})).toHaveValue("");
});
