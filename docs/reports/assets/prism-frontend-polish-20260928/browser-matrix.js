const {readFileSync,writeFileSync} = await import('node:fs');
const directory = '/Users/huangrui/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/docs/reports/assets/prism-frontend-polish-20260928';
// Prepend a literal MATRIX_CONTEXT tuple when passing this script to ego-browser nodejs.
const [width,height,theme,only] = MATRIX_CONTEXT;
const task = await taskSpace(23);
const page = task.page('p2');
await page.cdp('Emulation.setDeviceMetricsOverride', {width,height,deviceScaleFactor:1,mobile:false});
await page.cdp('Emulation.setEmulatedMedia', {features:[
  {name:'prefers-color-scheme',value:theme},
  {name:'prefers-reduced-motion',value:'no-preference'},
  {name:'forced-colors',value:'none'},
  {name:'prefers-contrast',value:'no-preference'},
  {name:'prefers-reduced-transparency',value:'no-preference'},
]});
const rows = [];
for (const [name,route] of [['overview','/'],['accounts','/accounts'],['upstreams','/upstreams'],['models','/models'],['access','/access'],['monitoring','/monitoring'],['usage','/usage'],['settings','/settings']]) {
  if(only && !only.includes(name))continue;
  if (await page.evaluate(() => getComputedStyle(document.querySelector('#main-navigation')).display === 'none')) await page.click('css:#nav-toggle');
  await page.click(`css:#main-navigation a[href$="#${route}"]`);
  await page.waitForFunction(route => location.hash.split('?')[0] === `#${route}` && Boolean(document.querySelector('.page-head h2')), route, {timeout:10000});
  await page.waitForFunction(() => !/读取中[.…]|正在读取.*…|读取(连接|接口|来源|账号|费用来源)…|加载中/.test(document.querySelector('.canvas').innerText), undefined, {timeout:10000});
  await page.waitForFunction(() => {
    const host = document.querySelector('#main-navigation');
    if (getComputedStyle(host).display === 'none') return true;
    const active = host.querySelector("a[aria-current='page']")?.getBoundingClientRect();
    const svg = host.querySelector('.selection-indicator');
    const rect = svg?.querySelector('rect');
    if (!active || !rect) return false;
    const origin = svg.getBoundingClientRect();
    return Math.abs(Number(rect.getAttribute('y')) - active.top + origin.top) < 1;
  }, undefined, {timeout:10000});
  const metrics = await page.evaluate(() => {
    const canvas = document.querySelector('.canvas');
    const limit = canvas.getBoundingClientRect().right;
    const clipped = [];
    for (const element of canvas.querySelectorAll('*')) {
      const box = element.getBoundingClientRect();
      if (!element.checkVisibility({contentVisibilityAuto:true,visibilityProperty:true}) || !box.width || !box.height || box.right <= limit + 1) continue;
      let reachable = false;
      for (let parent = element.parentElement; parent && parent !== document.body; parent = parent.parentElement) {
        if (['auto','scroll'].includes(getComputedStyle(parent).overflowX)) { reachable = true; break; }
      }
      if (!reachable) clipped.push(`${element.tagName}.${String(element.className?.baseVal ?? element.className).split(' ').slice(0,2).join('.')} +${Math.round(box.right-limit)}px`);
    }
    const visible = element => {
      const box=element.getBoundingClientRect();
      return box.width>0 && box.height>0 && element.checkVisibility({contentVisibilityAuto:true,visibilityProperty:true});
    };
    const dock=document.querySelector('.dock');
    const box=dock?.getBoundingClientRect();
    return {
      viewport:[innerWidth,innerHeight],theme:document.documentElement.dataset.theme ?? (matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light'),
      route:location.hash,heading:canvas.querySelector('.page-head h2')?.textContent,
      horizontalDocumentOverflow:document.documentElement.scrollWidth>innerWidth,
      clipped:[...new Set(clipped)].slice(0,12),
      primaryActions:[...canvas.querySelectorAll('button.primary')].filter(visible).map(b=>b.textContent.trim()),
      dock:box?{x:box.x,y:box.y,width:box.width,height:box.height,buttons:[...dock.querySelectorAll('button')].map(b=>({text:b.textContent.trim(),variant:b.className,disabled:b.disabled}))}:null,
      badgeRadius:[...canvas.querySelectorAll('.badge')].filter(visible).slice(0,3).map(b=>getComputedStyle(b).borderRadius),
      errors:window.__polishErrors ?? [],
    };
  });
  const screenshot=`${directory}/matrix-${name}-${width}-${theme}.png`;
  await page.screenshot({path:screenshot});
  rows.push({name,route,...metrics,screenshot});
  console.log(JSON.stringify({name,viewport:metrics.viewport,theme:metrics.theme,clipped:metrics.clipped,overflow:metrics.horizontalDocumentOverflow,primary:metrics.primaryActions,dock:metrics.dock,errors:metrics.errors.length}));
}
const resultPath=`${directory}/matrix-${width}-${theme}.json`;
const merged=only?JSON.parse(readFileSync(resultPath,'utf8')).map(row=>rows.find(updated=>updated.name===row.name)??row):rows;
writeFileSync(resultPath,JSON.stringify(merged,null,2)+'\n');
