export interface NavItem {
  label: string;
  slug: string;
}

export interface NavSection {
  title: string;
  items: NavItem[];
}

export const userGuideSections: NavSection[] = [
  {
    title: "Introduction",
    items: [
      { label: "What is IRC?", slug: "what-is-irc" },
    ],
  },
  {
    title: "Getting Started",
    items: [
      { label: "Installation", slug: "installation" },
      { label: "Quick Start", slug: "quickstart" },
    ],
  },
  {
    title: "CLI Client",
    items: [
      { label: "Connecting", slug: "cli-connecting" },
      { label: "The Interface", slug: "cli-interface" },
      { label: "Chatting", slug: "cli-chatting" },
      { label: "Command Reference", slug: "cli-commands" },
      { label: "CLI Configuration", slug: "cli-configuration" },
    ],
  },
  {
    title: "Concepts",
    items: [
      { label: "IRC Networks", slug: "irc-networks" },
      { label: "Modes Reference", slug: "modes-reference" },
    ],
  },
  {
    title: "GUI Client",
    items: [
      { label: "GUI Client", slug: "gui-usage" },
    ],
  },
];

export const serverGuideSections: NavSection[] = [
  {
    title: "Getting Started",
    items: [
      { label: "Running the Server", slug: "server-quickstart" },
    ],
  },
  {
    title: "Configuration",
    items: [
      { label: "Configuration Reference", slug: "server-configuration" },
      { label: "TLS Encryption", slug: "server-tls" },
    ],
  },
  {
    title: "Administration",
    items: [
      { label: "Server Operators", slug: "server-operators" },
      { label: "Channel Management", slug: "server-channels" },
    ],
  },
  {
    title: "Production",
    items: [
      { label: "Deployment", slug: "server-deployment" },
    ],
  },
];

export const devGuideSections: NavSection[] = [
  {
    title: "Overview",
    items: [
      { label: "Project Overview", slug: "project-overview" },
    ],
  },
  {
    title: "Development",
    items: [
      { label: "Contributing", slug: "contributing" },
    ],
  },
];
