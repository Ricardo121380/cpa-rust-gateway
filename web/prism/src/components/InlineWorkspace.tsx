import { useCallback, useEffect, useId, useRef, useState, type ReactNode } from "react";
import { flushSync } from "react-dom";
import { useBlocker, useLocation } from "react-router-dom";
import { Sheet } from "./Sheet";
import { useOperationBoundary } from "./OperationBoundary";

type Leave = {kind:"route"} | {kind:"local"; next:()=>void};
export function InlineWorkspace({title,description,children,footer,busy,dirty,onClose}:Readonly<{
  title:string; description?:ReactNode; children:ReactNode; footer:ReactNode;
  busy:boolean; dirty:boolean; onClose:()=>void;
}>) {
  const boundary=useOperationBoundary();
  const titleId=useId();
  const heading=useRef<HTMLHeadingElement>(null);
  const previousFocus=useRef<HTMLElement | null>(null);
  const [leave,setLeave]=useState<Leave>();
  const location=useLocation();
  const acceptedRoute=useRef<string | undefined>(undefined);
  const blocker=useBlocker(({currentLocation,nextLocation})=>(busy||dirty)&&
    (currentLocation.pathname!==nextLocation.pathname||currentLocation.search!==nextLocation.search||currentLocation.hash!==nextLocation.hash));
  const accept=useCallback((transition:Leave)=>{
    if(busy)return;
    if(transition.kind==="route"){
      if(blocker.state==="blocked"){acceptedRoute.current=location.key;setLeave(undefined);blocker.proceed();}
    }else{
      flushSync(()=>{setLeave(undefined);onClose();});
      transition.next();
    }
  },[busy,blocker,onClose,location.key]);
  const request=useCallback((transition:Leave)=>{
    if(busy||leave)return;
    if(dirty){previousFocus.current ??= document.activeElement instanceof HTMLElement?document.activeElement:null;setLeave(transition);}
    else accept(transition);
  },[busy,leave,dirty,accept]);
  useEffect(()=>boundary.register(next=>request({kind:"local",next})),[boundary,request]);
  useEffect(()=>{
    if(blocker.state!=="blocked"||acceptedRoute.current!==undefined)return;
    if(busy){blocker.reset();return;}
    request({kind:"route"});
  },[blocker,busy,request]);
  useEffect(()=>{
    if(acceptedRoute.current!==undefined&&location.key!==acceptedRoute.current){
      acceptedRoute.current=undefined;
      onClose();
    }
  },[location.key,onClose]);
  useEffect(()=>{
    if(busy&&leave){if(blocker.state==="blocked")blocker.reset();setLeave(undefined);}
  },[busy,leave,blocker]);
  useEffect(()=>{heading.current?.focus();heading.current?.scrollIntoView({block:"start"});},[title]);
  useEffect(()=>{
    const handler=(event:BeforeUnloadEvent)=>{if(dirty||busy){event.preventDefault();event.returnValue="";}};
    window.addEventListener("beforeunload",handler);
    return ()=>window.removeEventListener("beforeunload",handler);
  },[dirty,busy]);
  const keep=()=>{if(blocker.state==="blocked")blocker.reset();setLeave(undefined);};
  return <section className="inline-workspace" role="region" aria-labelledby={titleId} aria-busy={busy}
    onFocusCapture={event=>{if(event.target instanceof HTMLInputElement||event.target instanceof HTMLTextAreaElement||event.target instanceof HTMLSelectElement)previousFocus.current=event.target;}}
    onKeyDown={event=>{if(event.key==="Escape"&&!event.defaultPrevented&&!leave&&!(event.target instanceof HTMLSelectElement)&&!(event.target instanceof Element&&event.target.closest('[role="menu"],[role="listbox"],[aria-expanded="true"]'))){event.preventDefault();request({kind:"local",next:()=>{}});}}}>

    <header className="page-head"><div><h3 id={titleId} ref={heading} tabIndex={-1}>{title}</h3>{description?<p className="stat-sub">{description}</p>:null}</div><button type="button" className="secondary" disabled={busy} onClick={()=>request({kind:"local",next:()=>{}})}>关闭编辑</button></header>
    <div className="inline-workspace-body">{children}</div>
    <footer className="inline-workspace-footer">{footer}</footer>
    {leave?<Sheet navigationOwned returnFocus={()=>previousFocus.current} title="放弃未保存的修改？" layout="confirm" guardUnsaved={false} onEscape={keep} footer={<><button type="button" className="secondary" onClick={keep}>继续编辑</button><button type="button" onClick={()=>accept(leave)}>放弃修改</button></>}><p>继续操作将放弃本次输入。</p></Sheet>:null}
  </section>;
}
