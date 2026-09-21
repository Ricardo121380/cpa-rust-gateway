import { expect, test } from "@playwright/test";
import { selectDraft, unlock } from "./helpers";

test("dashboard resource failures retain observed counts and offer keyboard retry", async ({ page }) => {
  await unlock(page);
  const resources = page.locator('.overview-resources');
  await expect(resources.locator('.count-value').first()).not.toHaveText('…');
  await page.evaluate(async () => {
    const { ManagementApi } = await import('/src/generated/management-client.ts');
    const { queryClient } = await import('/src/api/queryClient.ts');
    const original = ManagementApi.prototype.request;
    Reflect.set(globalThis, '__rejectResourceCounts', true);
    ManagementApi.prototype.request = async function(operation: string, request: unknown) {
      if (operation === 'listUpstreams' && Reflect.get(globalThis, '__rejectResourceCounts'))
        return new Response(JSON.stringify({error:{code:'synthetic_count_unavailable',message:'Synthetic resource read failure'}}),{status:400});
      return original.call(this, operation, request);
    };
    await queryClient.invalidateQueries({queryKey:['upstreams-count']});
  });
  await expect(resources.getByRole('alert')).toContainText('读取失败');
  await expect(resources.getByRole('alert')).toContainText('上次读取');
  await page.evaluate(() => Reflect.set(globalThis, '__rejectResourceCounts', false));
  await resources.getByRole('button', {name:'重试读取'}).focus();
  await page.keyboard.press('Enter');
  await expect(resources.getByRole('alert')).toHaveCount(0);
  await expect(resources.locator('.count-value').first()).not.toHaveText('…');
});

test("empty observed requests preserve unknown success and latency instead of invented zeros", async ({ page }) => {
  await unlock(page);
  await page.evaluate(async () => {
    const { ManagementApi } = await import('/src/generated/management-client.ts');
    const { queryClient } = await import('/src/api/queryClient.ts');
    const original = ManagementApi.prototype.request;
    ManagementApi.prototype.request = async function(operation: string, request: unknown) {
      const response=await original.call(this,operation,request);
      if(operation !== 'summarizeRequests')return response;
      const body=await response.json();
      return new Response(JSON.stringify({...body,items:[],series:[],next_cursor:null,summary:{requests:0,succeeded:0,failed:0,cancelled:0,unknown:0,attempts:0,success_rate:null,average_duration_ms:null,average_first_content_ms:null,p50_duration_ms:null,p95_duration_ms:null}}),{status:200});
    };
    await queryClient.invalidateQueries({queryKey:['request-summary']});
  });
  const overview=page.locator('.request-overview');
  await expect(overview).toContainText('此时间范围没有已观测的请求终态');
  await expect(overview.getByRole('link').filter({hasText:'成功率'})).toContainText('—');
  await expect(overview.getByRole('link').filter({hasText:'P50 / P95'})).toContainText('— / —');
  await overview.locator('summary').focus();await page.keyboard.press('Enter');
  await expect(overview.locator('details')).toHaveAttribute('open','');
  await expect(overview.locator('tbody tr')).toHaveCount(0);
});

test("long provider names remain readable without mobile document overflow", async ({ page }) => {
  await page.setViewportSize({width:390,height:844});
  await unlock(page);
  await selectDraft(page);
  const name='Provider-'+ 'LongName'.repeat(24);
  await page.evaluate(async name=>{
    const {ManagementApi}=await import('/src/generated/management-client.ts');
    const original=ManagementApi.prototype.request;
    ManagementApi.prototype.request=async function(operation:string,request:unknown){
      const response=await original.call(this,operation,request);
      if(operation!=='listUpstreams')return response;
      const rows=await response.json();
      return new Response(JSON.stringify(rows.map((row:Record<string,unknown>,index:number)=>index===0?{...row,name}:row)),{status:200});
    };
    const {router}=await import('/src/App.tsx');await router.navigate('/upstreams');
  },name);
  await expect(page.locator('.provider-list')).toContainText(name);
  expect(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth)).toBe(false);
});
