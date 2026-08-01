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
          { text: "Language 1.0 Spec", link: "/spec/language-1.0" },
          { text: "Delivery Archive", link: "/project/archive/language-1.0-delivery" },
          { text: "Compatibility", link: "/compatibility" },
          { text: "Diagnostics", link: "/reference/diagnostics" },
          { text: "Language 1.0 Roadmap", link: "/project/roadmap/language-1.0" },
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
            { text: "Language Guide", link: "/guide/language" },
            { text: "CLI Guide", link: "/guide/cli" },
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
            { text: "Language Guide", link: "/guide/language" },
            { text: "CLI Guide", link: "/guide/cli" },
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
          text: "Reference",
          collapsed: false,
          items: [
            { text: "Compatibility", link: "/compatibility" },
            { text: "Diagnostics", link: "/reference/diagnostics" },
            { text: "Release Validation", link: "/release" },
          ],
        },
        {
          text: "Language Design",
          collapsed: false,
          items: [
            { text: "Language Core v0.9 (Delivered)", link: "/spec/language-core-v0.9" },
            { text: "v0.9 Roadmap (Delivered)", link: "/project/roadmap/language-core-v0.9" },
            { text: "Language Core v0.8 (Delivered)", link: "/spec/language-core-v0.8" },
            { text: "v0.8 Roadmap (Delivered)", link: "/project/roadmap/language-core-v0.8" },
            { text: "Language Core v0.7 (Delivered)", link: "/spec/language-core-v0.7" },
            { text: "v0.7 Roadmap (Delivered)", link: "/project/roadmap/language-core-v0.7" },
            { text: "Language Core v0.6 (Delivered)", link: "/spec/language-core-v0.6" },
            { text: "v0.6 Roadmap (Delivered)", link: "/project/roadmap/language-core-v0.6" },
            { text: "Language Core v0.5 (Delivered)", link: "/spec/language-core-v0.5" },
            { text: "v0.5 Roadmap (Delivered)", link: "/project/roadmap/language-core-v0.5" },
            { text: "Language Core v0.4 (Delivered)", link: "/spec/language-core-v0.4" },
            { text: "v0.4 Roadmap (Delivered)", link: "/project/roadmap/language-core-v0.4" },
            { text: "Language 1.0 (Delivered)", link: "/spec/language-1.0" },
            { text: "1.0 Roadmap (Delivered)", link: "/project/roadmap/language-1.0" },
            { text: "1.0 Delivery Archive", link: "/project/archive/language-1.0-delivery" },
            { text: "Language Core v0.3 (Delivered)", link: "/spec/language-core-v0.3" },
            { text: "Language Core v0.2", link: "/spec/language-core-v0.2" },
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
