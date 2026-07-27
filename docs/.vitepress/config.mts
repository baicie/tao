import { defineConfig } from "vitepress";

export default defineConfig({
  title: "Nexa",
  description: "A TS-shaped language with independent static semantics and a MIR interpreter",
  lang: "en-US",
  lastUpdated: true,
  cleanUrls: true,

  head: [
    ["link", { rel: "icon", href: "/favicon.svg", type: "image/svg+xml" }],
  ],

  themeConfig: {
    logo: "/logo.svg",

    nav: [
      { text: "Home", link: "/" },
      { text: "Guide", link: "/guide/getting-started" },
      { text: "Architecture", link: "/architecture" },
      { text: "Language Spec", link: "/spec/" },
      {
        text: "More",
        items: [
          { text: "Roadmap", link: "/project/roadmap/language-core-v0.1" },
          { text: "Changelog", link: "/changelog" },
          { text: "Contributing", link: "/contributing" },
        ],
      },
    ],

    sidebar: {
      "/guide/": [
        {
          text: "Guide",
          collapsed: false,
          items: [
            { text: "Getting Started", link: "/guide/getting-started" },
            { text: "Project Structure", link: "/guide/project-structure" },
            { text: "Development", link: "/guide/development" },
          ],
        },
      ],
      "/": [
        {
          text: "Introduction",
          items: [{ text: "Overview", link: "/" }],
        },
        {
          text: "Guide",
          collapsed: false,
          items: [
            { text: "Getting Started", link: "/guide/getting-started" },
            { text: "Project Structure", link: "/guide/project-structure" },
            { text: "Development", link: "/guide/development" },
          ],
        },
        {
          text: "Architecture",
          collapsed: false,
          items: [
            { text: "Overview", link: "/architecture" },
          ],
        },
        {
          text: "Language Design",
          collapsed: false,
          items: [
            { text: "Language Core v0.1", link: "/spec/" },
            { text: "Roadmap", link: "/project/roadmap/language-core-v0.1" },
          ],
        },
      ],
    },

    socialLinks: [
      { icon: "github", link: "https://github.com/baicie/nexa" },
    ],

    search: {
      provider: "local",
    },

    footer: {
      message: "Released under the MIT License.",
      copyright: "Copyright (c) 2024-present",
    },

    outline: {
      label: "On This Page",
    },

    docFooter: {
      prev: "Previous",
      next: "Next",
    },

    lastUpdatedText: "Last Updated",
  },
});
