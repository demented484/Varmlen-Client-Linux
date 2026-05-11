export interface NavItem {
  path: string;
  label: string;
  /** Material-symbols-like outline glyph rendered inline as SVG path data. */
  icon: string;
}

export const NAV: NavItem[] = [
  {
    path: "/",
    label: "Dashboard",
    icon: "M3 13h8V3H3v10zm0 8h8v-6H3v6zm10 0h8V11h-8v10zm0-18v6h8V3h-8z",
  },
  {
    path: "/servers",
    label: "Servers",
    icon: "M4 6a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v3a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6zm0 9a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v3a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-3zm4-7.5a1 1 0 1 0 0-2 1 1 0 0 0 0 2zm0 9a1 1 0 1 0 0-2 1 1 0 0 0 0 2z",
  },
  {
    path: "/rules",
    label: "Split rules",
    icon: "M6 3v2H4v14h6v2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h2zm12 0a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-2v-2h2V5h-2V3h2zm-8 4h4v2h-4V7zm-2 4h8v2H8v-2zm2 4h4v2h-4v-2z",
  },
  {
    path: "/settings",
    label: "Settings",
    icon: "M19.14 12.94a7.96 7.96 0 0 0 .06-1.88l2.03-1.58a.5.5 0 0 0 .12-.64l-1.92-3.32a.5.5 0 0 0-.61-.22l-2.39.96a8.06 8.06 0 0 0-1.63-.94l-.36-2.54A.5.5 0 0 0 13.94 2h-3.88a.5.5 0 0 0-.5.42l-.36 2.54c-.59.24-1.13.55-1.63.94l-2.39-.96a.5.5 0 0 0-.61.22L2.65 8.48a.5.5 0 0 0 .12.64l2.03 1.58a8.05 8.05 0 0 0 0 1.88l-2.03 1.58a.5.5 0 0 0-.12.64l1.92 3.32c.14.24.43.34.61.22l2.39-.96c.5.39 1.04.7 1.63.94l.36 2.54c.06.24.27.42.5.42h3.88c.23 0 .44-.18.5-.42l.36-2.54c.59-.24 1.13-.55 1.63-.94l2.39.96c.18.12.47.02.61-.22l1.92-3.32a.5.5 0 0 0-.12-.64l-2.03-1.58zM12 15.5a3.5 3.5 0 1 1 0-7 3.5 3.5 0 0 1 0 7z",
  },
];
