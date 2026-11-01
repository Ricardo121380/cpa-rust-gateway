// From 21st.dev Agent Elements (MIT, github.com/21st-dev/agent-elements), registry item "tool-group".
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
