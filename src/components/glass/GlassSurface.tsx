// The one glass primitive. Dev-mode pane counter enforces the ≤3 budget
// (docs/07 §8.8) mechanically: exceeding it logs an error during development.
import { useEffect, type CSSProperties, type ReactNode } from "react";

let mountedPanes = 0;

type GlassSurfaceProps = Readonly<{
  as?: "header" | "nav" | "aside" | "div" | "footer";
  material?: "draft" | "active" | "archived";
  className?: string;
  style?: CSSProperties;
  children: ReactNode;
  /** modal-layer panes (sheet/popover/toast) are exempt from the 3-pane budget */
  layer?: "chrome" | "modal";
  /** identifies the pane for its dedicated SVG lens filter (see PrismLens) */
  pane?: "topbar" | "rail" | "dock";
}>;

export function GlassSurface({
  as: Tag = "div",
  material,
  className,
  style,
  children,
  layer = "chrome",
  pane,
}: GlassSurfaceProps) {
  useEffect(() => {
    if (layer !== "chrome") {
      return;
    }
    mountedPanes += 1;
    if (import.meta.env.DEV && mountedPanes > 3) {
      console.error(
        `glass budget exceeded: ${mountedPanes} chrome panes mounted (max 3 — docs/07 §8.8)`,
      );
    }
    return () => {
      mountedPanes -= 1;
    };
  }, [layer]);

  return (
    <Tag
      className={className === undefined ? "glass" : `glass ${className}`}
      data-material={material}
      data-pane={pane}
      style={style}
    >
      {children}
    </Tag>
  );
}
