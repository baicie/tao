import { defineConfig } from "vitepress";

export default defineConfig({
  title: "Futao",
  description: "The Rust bootstrap compiler and architecture for the Futao language",
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
      { text: "Decisions", link: "/adr/" },
      { text: "Language Spec", link: "/spec/" },
      {
        text: "More",
        items: [
          { text: "Language 1.0 Spec", link: "/spec/language-1.0" },
          { text: "Delivery Archive", link: "/project/archive/language-1.0-delivery" },
          { text: "Compatibility", link: "/compatibility" },
          { text: "Diagnostics", link: "/reference/diagnostics" },
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
            { text: "Decision Log", link: "/adr/" },
            { text: "ADR-000 Bootstrap", link: "/adr/000-bootstrap-compiler" },
            { text: "ADR-001 Language 2.0", link: "/adr/001-language-architecture-ownership-runtime" },
            { text: "ADR-002 Ownership", link: "/adr/002-ownership-borrowing-move-drop" },
            { text: "ADR-003 NIR and ABI", link: "/adr/003-nir-llvm-backend-stable-abi" },
            { text: "ADR-004 Memory Layout", link: "/adr/004-runtime-memory-layout" },
            { text: "ADR-005 Host ABI", link: "/adr/005-host-abi-capabilities-handles" },
            { text: "ADR-006 Failure Model", link: "/adr/006-errors-panic-defer-abi" },
            { text: "ADR-007 Async", link: "/adr/007-async-structured-concurrency" },
            { text: "ADR-008 Wasm and UI", link: "/adr/008-wasm-js-ffi-ui-host" },
            { text: "ADR-009 Packages", link: "/adr/009-package-lockfile-artifacts-signing" },
            { text: "ADR-010 Futao Name", link: "/adr/010-futao-language-name" },
            { text: "ADR-011 Self-host Gate", link: "/adr/011-toolchain-versioning-and-self-hosting-gate" },
            { text: "0.1.0 Implementation Plan", link: "/implementation/self-hosting-0.1.0" },
            { text: "0.0.4 Storage Kernel", link: "/implementation/storage-kernel-0.0.4" },
            { text: "0.0.5 Typed NIR", link: "/implementation/nir-artifact-0.0.5" },
            { text: "0.0.6 Bootstrap Profile", link: "/implementation/bootstrap-profile-stdlib-0.0.6" },
            { text: "0.0.7 Lexer Differential", link: "/implementation/futao-lexer-differential-0.0.7" },
            { text: "0.0.8 Parser Differential", link: "/implementation/futao-parser-differential-0.0.8" },
            { text: "0.0.8 Parser Self-Graph", link: "/implementation/futao-parser-self-graph-scalability-0.0.8" },
            { text: "0.0.9 Resolver Differential", link: "/implementation/futao-resolver-differential-0.0.9" },
            { text: "0.0.10 Type Checker Kernel", link: "/implementation/futao-type-checker-0.0.10" },
            { text: "ADR-004 Shared and Weak", link: "/implementation/adr-004-shared-weak" },
            { text: "ADR-004 Owned Box", link: "/implementation/adr-004-owned-box" },
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
          text: "Language History",
          collapsed: true,
          items: [
            { text: "Language 1.0 (Delivered)", link: "/spec/language-1.0" },
            { text: "Language Core v0.9 (Delivered)", link: "/spec/language-core-v0.9" },
            { text: "Language Core v0.8 (Delivered)", link: "/spec/language-core-v0.8" },
            { text: "1.0 Delivery Archive", link: "/project/archive/language-1.0-delivery" },
          ],
        },
      ],
    },

    socialLinks: [
      { icon: "github", link: "https://github.com/baicie/tao" },
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
