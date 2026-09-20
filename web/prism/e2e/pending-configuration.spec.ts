import {expect,test} from "@playwright/test";
import {unlock,navigate,selectDraft} from "./helpers";

test("pending review is inline and validation does not apply without confirmation",async({page})=>{
 await unlock(page);await selectDraft(page);
 await page.locator(".dock").getByRole("button",{name:"查看变更",exact:true}).click();
 const review=page.getByRole("region",{name:"待应用变更",exact:true});
 await expect(review).toBeVisible();await expect(page.getByRole("dialog")).toHaveCount(0);
 await expect(review).toContainText("完整净变化");
 await review.getByRole("button",{name:"校验并应用",exact:true}).click();
 const confirm=page.getByRole("dialog",{name:"确认应用配置"});
 await expect(confirm).toContainText("校验通过");
 await expect(page.locator("main")).toHaveAttribute("data-context-status","draft");
 await confirm.getByRole("button",{name:"取消",exact:true}).click();
 await expect(review).toBeVisible();
});

test("global catalog evidence is separate from net configuration changes",async({page})=>{
 await unlock(page);await selectDraft(page);await navigate(page,"计费与价格");
 await page.locator('[data-resource-id="cat-2026-08"]').getByRole("button",{name:"复制编辑"}).click();
 const editor=page.locator(".inline-workspace");await editor.getByLabel("生效时间（本地时区）").fill("2026-09-19T08:00");
 await editor.getByRole("button",{name:"预览差异"}).click();await editor.getByRole("button",{name:"确认导入"}).click();
 await page.getByRole("region",{name:"目录导入结果"}).getByRole("button",{name:"完成",exact:true}).click();
 await page.locator(".dock").getByRole("button",{name:"查看变更",exact:true}).click();
 await expect(page.getByRole("region",{name:"已独立保存的全局操作"})).toContainText("价格目录已导入");
 await expect(page.getByRole("region",{name:"待应用变更",exact:true})).toContainText("不会删除这些目录");
});

test("replacing the local pending selection requires an explicit decision",async({page})=>{
 await unlock(page);await selectDraft(page);await navigate(page,"配置版本");
 await page.getByRole("button",{name:"创建空草稿"}).click();
 const create=page.getByRole("dialog",{name:"创建空草稿"});await create.getByLabel("描述").fill("Second batch");
 await create.getByRole("button",{name:"创建草稿",exact:true}).click();
 await page.getByRole("dialog",{name:"草稿创建结果"}).getByRole("button",{name:"关闭",exact:true}).click();
 const row=page.locator("tr[data-version-id]",{hasText:"Second batch"});await row.getByRole("button",{name:"编辑草稿"}).click();
 const adopt=page.getByRole("dialog",{name:"接续配置草稿"});await expect(adopt).toContainText("原草稿仍保存在服务端");
 await adopt.getByRole("button",{name:"取消",exact:true}).click();
 await expect(page.locator("main")).toHaveAttribute("data-context-version","draft-2026-08");
 await row.getByRole("button",{name:"编辑草稿"}).click();await adopt.getByRole("button",{name:"切换待应用选择"}).click();
 await expect(page.locator("main")).not.toHaveAttribute("data-context-version","draft-2026-08");
 await expect(page.locator('[data-version-id="draft-2026-08"]')).toBeVisible();
});
