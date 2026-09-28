const {writeFileSync} = await import('node:fs');
const directory = '/Users/huangrui/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/docs/reports/assets/prism-frontend-polish-20260928';
const task = await taskSpace(23), page = task.page('p2');
await page.cdp('Emulation.setDeviceMetricsOverride', {width:1440,height:900,deviceScaleFactor:1,mobile:false});
const observations = [];
for (const theme of ['light','dark']) {
  await page.cdp('Emulation.setEmulatedMedia',{features:[{name:'prefers-color-scheme',value:theme},{name:'forced-colors',value:'none'},{name:'prefers-contrast',value:'no-preference'}]});
  for (const route of ['/','/accounts','/upstreams','/models','/access','/monitoring','/usage','/settings']) {
    await page.click(`css:#main-navigation a[href$="#${route}"]`);
    await page.waitForFunction(route => location.hash.split('?')[0]===`#${route}` && !/读取中[.…]|正在读取.*…|读取(连接|接口|来源|账号|费用来源)…|加载中/.test(document.querySelector('.canvas').innerText),route,{timeout:10000});
    // Sample settled palette values after the existing navigation colour transition.
    await page.waitForFunction(() => {
      const canvas=document.createElement('canvas');canvas.width=canvas.height=1;
      const context=canvas.getContext('2d',{willReadFrequently:true});
      const parse=color=>{context.clearRect(0,0,1,1);context.fillStyle=color;context.fillRect(0,0,1,1);return [...context.getImageData(0,0,1,1).data];};
      return [...document.querySelectorAll('.rail a')].every(anchor=>{
        const style=getComputedStyle(anchor),expected=parse(style.getPropertyValue(anchor.matches('[aria-current="page"]')?'--tint':anchor.matches(':hover')?'--ink':'--ink-2'));
        return parse(style.color).every((value,index)=>Math.abs(value-expected[index])<2);
      });
    },undefined,{timeout:10000});
    const rows = await page.evaluate(() => {
      // Parse browser-resolved RGB/color(srgb) values without editing page styles.
      const canvas=document.createElement('canvas'); canvas.width=canvas.height=1;
      const context=canvas.getContext('2d',{willReadFrequently:true});
      const parse=color=>{context.clearRect(0,0,1,1);context.fillStyle=color;context.fillRect(0,0,1,1);const rgba=[...context.getImageData(0,0,1,1).data];return [rgba[0],rgba[1],rgba[2],rgba[3]/255];};
      const blend=(fg,bg)=>fg.slice(0,3).map((v,i)=>v*fg[3]+bg[i]*(1-fg[3]));
      const background=element=>{
        const ancestors=[];for(let node=element;node;node=node.parentElement)ancestors.push(node);
        let rgb=parse(getComputedStyle(document.documentElement).getPropertyValue('--canvas')).slice(0,3);
        for(const node of ancestors.reverse())rgb=blend(parse(getComputedStyle(node).backgroundColor),rgb);
        return rgb;
      };
      const linear=v=>{const s=v/255;return s<=0.04045?s/12.92:((s+0.055)/1.055)**2.4;};
      const luminance=rgb=>0.2126*linear(rgb[0])+0.7152*linear(rgb[1])+0.0722*linear(rgb[2]);
      const ratio=(fg,bg)=>{const pair=[luminance(fg),luminance(bg)].sort((a,b)=>b-a);return(pair[0]+0.05)/(pair[1]+0.05);};
      const rows=[];
      for(const element of document.querySelectorAll('.canvas button:not(:disabled), .canvas .badge, .dock button:not(:disabled), .canvas a, .rail a')) {
        if(!element.checkVisibility({contentVisibilityAuto:true,visibilityProperty:true}))continue;
        if(element.matches('a')&&!element.closest('.rail')&&![...element.childNodes].some(n=>n.nodeType===3&&n.textContent.trim()))continue;
        const style=getComputedStyle(element), own=background(element), parent=background(element.parentElement);
        let stops=style.backgroundImage.match(/rgba?\([^)]*\)|color\([^)]*\)|#[\da-f]+/gi)??[];
        const selection=element.closest('[data-selection-ready="true"]');
        if(selection && element.matches('[aria-current="page"], [aria-pressed="true"]'))stops=[...selection.querySelectorAll('.selection-indicator stop')].map(stop=>getComputedStyle(stop).stopColor);
        const colors=stops.length?stops.map(stop=>blend(parse(stop),parent)):[own];
        const minimum=Math.min(...colors.map(bg=>ratio(blend(parse(style.color),bg),bg)));
        const size=parseFloat(style.fontSize),floor=size>=24||(size>=18.66&&Number(style.fontWeight)>=700)?3:4.5;
        rows.push({element:element.tagName,class:element.className,text:element.textContent.trim().slice(0,60),ratio:Number(minimum.toFixed(3)),floor,pass:minimum>=floor});
      }
      return rows;
    });
    observations.push({theme,route,rows});
    console.log(JSON.stringify({theme,route,checked:rows.length,min:Math.min(...rows.map(r=>r.ratio)),failures:rows.filter(r=>!r.pass)}));
  }
}
writeFileSync(`${directory}/contrast.json`,JSON.stringify(observations,null,2)+'\n');
