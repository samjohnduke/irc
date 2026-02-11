export interface NavItem {
  label: string;
  slug: string;
}

export interface NavSection {
  title: string;
  items: NavItem[];
}

export const guideSections: NavSection[] = [
  {
    title: "Getting Started",
    items: [
      { label: "Introduction", slug: "getting-started" },
      { label: "Installation", slug: "installation" },
    ],
  },
  {
    title: "Server",
    items: [
      { label: "Server Setup", slug: "server-setup" },
      { label: "Configuration", slug: "configuration" },
    ],
  },
  {
    title: "Clients",
    items: [
      { label: "CLI Usage", slug: "cli-usage" },
      { label: "GUI Usage", slug: "gui-usage" },
    ],
  },
];
