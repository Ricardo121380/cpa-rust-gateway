// Rare UI components import next/link; outside Next.js a plain anchor is equivalent here.
import { forwardRef, type AnchorHTMLAttributes } from "react";
const Link = forwardRef<HTMLAnchorElement, AnchorHTMLAttributes<HTMLAnchorElement> & { href: string; prefetch?: boolean; replace?: boolean; scroll?: boolean }>(({ prefetch: _p, replace: _r, scroll: _s, ...props }, ref) => <a ref={ref} {...props} />);
export default Link;
