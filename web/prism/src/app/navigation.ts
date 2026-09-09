import type { Pack } from "../i18n/messages";

export const NAV_GROUPS: ReadonlyArray<{
  label: keyof Pack["navigation"];
  items: ReadonlyArray<{ to: string; key: keyof Pack["nav"]; icon: string }>;
}> = [
  {
    label: "operations",
    items: [
      {
        to: "/",
        key: "overview",
        icon: "M3 3h7v7H3z M14 3h7v7h-7z M3 14h7v7H3z M14 14h7v7h-7z",
      },
      { to: "/monitoring", key: "monitoring", icon: "M3 12h4l3-8 4 16 3-8h4" },
      { to: "/usage", key: "usage", icon: "M4 20V10 M12 20V4 M20 20v-7" },
      {
        to: "/billing",
        key: "billing",
        icon: "M6 3h12v18l-3-2-3 2-3-2-3 2z M9 8h6 M9 12h6",
      },
    ],
  },
  {
    label: "resources",
    items: [
      {
        to: "/accounts",
        key: "accounts",
        icon: "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2 M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8 M20 8v6 M17 11h6",
      },
      {
        to: "/upstreams",
        key: "upstreams",
        icon: "M4 3h16v7H4z M4 14h16v7H4z M7 6h1 M7 17h1",
      },
      {
        to: "/catalog",
        key: "catalog",
        icon: "M3 4h6a3 3 0 0 1 3 3v14a3 3 0 0 0-3-3H3z M21 4h-6a3 3 0 0 0-3 3v14a3 3 0 0 1 3-3h6z",
      },
      {
        to: "/models",
        key: "models",
        icon: "M12 3v6 M5 15V9h14v6 M2 15h6v6H2z M16 15h6v6h-6z",
      },
      {
        to: "/access",
        key: "access",
        icon: "M12 3 3 7v6c0 5 9 9 9 9s9-4 9-9V7z M8 12l3 3 5-6",
      },
    ],
  },
  {
    label: "management",
    items: [
      {
        to: "/runtime",
        key: "runtime",
        icon: "M4 4h16v14H4z M8 22h8 M8 8l3 3-3 3 M13 14h4",
      },
      {
        to: "/egress",
        key: "egress",
        icon: "M3 7h16l-4-4 M19 7l-4 4 M21 17H5l4-4 M5 17l4 4",
      },
      {
        to: "/versions",
        key: "versions",
        icon: "M6 3v12a5 5 0 0 0 10 0V9 M3 3h6 M13 5h6v4h-6z",
      },
      {
        to: "/audit",
        key: "audit",
        icon: "M5 3h10l4 4v14H5z M14 3v5h5 M8 12h8 M8 16h8",
      },
      {
        to: "/settings",
        key: "settings",
        icon: "M4 6h16 M4 12h16 M4 18h16 M8 3v6 M16 9v6 M10 15v6",
      },
    ],
  },
];

export const NAV_ITEMS = NAV_GROUPS.flatMap((group) => group.items);
