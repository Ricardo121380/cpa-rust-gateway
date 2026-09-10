// Synthetic local gateway only. Input through stdin; never write credentials to artifacts.
import { chromium, expect } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
let input = '';
for await (const chunk of process.stdin) input += chunk;
const { base, initial, newPassword, output } = JSON.parse(input);
if (!/^http:\/\/127\.0\.0\.1:\d+$/u.test(base)) throw new Error('This acceptance script requires a synthetic loopback gateway');
await mkdir(output, { recursive: true });
const browser = await chromium.launch();
const evidence = { browser: browser.version(), checks: [], errors: [] };
try {
  const page = await browser.newPage();
  page.on('pageerror', () => evidence.errors.push('browser page error'));
  await page.goto(`${base}/admin-ui/#/unlock`);
  const capture = async name => {
    for (const [width, height] of [[1440,900],[1280,720],[390,844]]) {
      await page.setViewportSize({width,height});
      for (const colorScheme of ['light','dark']) {
        await page.emulateMedia({colorScheme});
        const overflow = await page.evaluate(() => document.documentElement.scrollWidth > innerWidth);
        if (overflow) throw new Error('Login page overflow');
        await page.screenshot({path:`${output}/${name}-${width}-${colorScheme}.png`});
      }
    }
  };
  await expect(page.getByRole('heading',{name:'管理员登录',exact:true})).toBeVisible();
  await expect(page.getByText('Management Key')).toHaveCount(0);
  await expect(page.getByText('CSRF Token')).toHaveCount(0);
  await capture('login');
  await page.getByLabel('账号',{exact:true}).fill('admin');
  await page.getByLabel('密码',{exact:true}).fill(initial);
  await page.getByRole('button',{name:'登录',exact:true}).click();
  await expect(page.getByRole('heading',{name:'设置新密码',exact:true})).toBeVisible();
  await expect(page.getByRole('navigation')).toHaveCount(0);
  await capture('first-password');
  await page.getByLabel('初始密码',{exact:true}).fill(initial);
  await page.getByLabel('新密码',{exact:true}).fill(newPassword);
  await page.getByLabel('确认新密码',{exact:true}).fill('mismatched-synthetic');
  await page.getByRole('button',{name:'保存并重新登录',exact:true}).click();
  await expect(page.getByRole('alert')).toHaveText('两次输入的新密码不一致');
  await page.getByLabel('确认新密码',{exact:true}).fill(newPassword);
  await page.getByRole('button',{name:'保存并重新登录',exact:true}).click();
  await expect(page.getByRole('heading',{name:'管理员登录',exact:true})).toBeVisible();
  await expect(page.getByRole('status')).toHaveText('密码已更新，请重新登录');
  await expect(page.getByLabel('密码',{exact:true})).toHaveValue('');
  await page.getByLabel('密码',{exact:true}).fill(newPassword);
  await page.getByRole('button',{name:'登录',exact:true}).click();
  await expect(page.getByRole('heading',{name:'总览',exact:true})).toBeVisible();
  await page.reload();
  await expect(page.getByRole('heading',{name:'管理员登录',exact:true})).toBeVisible();
  await expect(page.getByLabel('密码',{exact:true})).toHaveValue('');
  evidence.checks.push('initial restricted session → mismatch validation → durable change → fresh login → refresh clears session');
  await page.emulateMedia({colorScheme:'dark',reducedMotion:'reduce',contrast:'more'});
  await page.getByLabel('账号',{exact:true}).focus();
  await page.keyboard.press('Tab');
  await expect(page.getByLabel('密码',{exact:true})).toBeFocused();
  await page.screenshot({path:`${output}/mobile-contrast-keyboard.png`});
  const storage = await page.evaluate(() => localStorage.length + sessionStorage.length);
  if(storage !== 0) throw new Error('Unexpected browser persistence');
  evidence.checks.push('three viewport sizes, light/dark, reduced motion/contrast, keyboard focus, no browser storage');
} finally {
  await browser.close();
  await writeFile(`${output}/browser-login.json`,JSON.stringify(evidence,null,2));
}
if(evidence.errors.length) throw new Error('Browser errors during login');
console.log('Real gateway browser login and first-password change passed');
