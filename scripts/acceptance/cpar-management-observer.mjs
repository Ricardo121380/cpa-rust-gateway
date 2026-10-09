export async function installManagementObserver(page){
  // Install before the generated client captures its transport. Never serialize session values.
  await page.cdp("Page.addScriptToEvaluateOnNewDocument",{source:String.raw`(()=>{
    if(window.CPAR_SEO_OBSERVER_INSTALLED)return;
    window.CPAR_SEO_OBSERVER_INSTALLED=true;
    const original=window.fetch.bind(window);window.CPAR_SEO_RECORDS=[];window.CPAR_SEO_HEADERS={};
    const scrub=value=>Array.isArray(value)?value.map(scrub):value&&typeof value==='object'?Object.fromEntries(Object.entries(value).filter(([key])=>!['actor','secret','key','password','token','access_token','refresh_token','csrf_token','management_key','device_code','deviceCode','clientSecret','client_secret','user_code','userCode','verification_uri','verificationUri','device_id'].includes(key)).map(([key,value])=>[key,scrub(value)])):value;
    window.fetch=async(input,init)=>{
      const url=new URL(typeof input==='string'?input:input.url,location.href),headers=new Headers(init?.headers??(typeof input==='string'?undefined:input.headers));
      for(const name of ['x-management-key','x-management-csrf-token'])if(headers.has(name))window.CPAR_SEO_HEADERS[name]=headers.get(name);
      const method=init?.method??'GET';let body={};try{body=JSON.parse(init?.body??'{}')}catch{}
      if(window.CPAR_SEO_FAIL_CONFIG&&method==='GET'&&/^\/admin\/config-versions\/[^/]+$/.test(url.pathname)){
        window.CPAR_SEO_FAIL_CONFIG=false;window.CPAR_SEO_RECORDS.push({method,path:url.pathname,injected:'client follow-up read failure'});
        return new Response(JSON.stringify({error:{code:'management_runtime_unavailable',message:'Synthetic follow-up read failure'}}),{status:503,headers:{'content-type':'application/json'}});
      }
      const response=await original(input,init);
      if(url.pathname.startsWith('/admin/')&&!url.pathname.includes('/auth/')){
        const data=await response.clone().json().catch(()=>null);
        window.CPAR_SEO_RECORDS.push({method,path:url.pathname,config:headers.get('x-config-version'),ifMatch:headers.get('if-match'),authenticated:headers.has('x-management-key'),csrf:headers.has('x-management-csrf-token'),sameOrigin:url.origin===location.origin,body:scrub(body),status:response.status,response:scrub(data)});
        if(window.CPAR_SEO_LOSE_IMPORT&&method==='POST'&&url.pathname.endsWith('/account-import')&&response.status===201){window.CPAR_SEO_LOSE_IMPORT=false;throw new TypeError('Synthetic client response loss after genuine server commit');}
        if(window.CPAR_SEO_FAIL_AFTER_RECEIPT&&method==='GET'&&url.pathname.includes('/account-imports/')&&response.status===200){window.CPAR_SEO_FAIL_AFTER_RECEIPT=false;window.CPAR_SEO_FAIL_CONFIG=true;}
      }
      return response;
    };
  })()`});
}
