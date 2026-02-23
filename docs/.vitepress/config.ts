import { defineConfig } from 'vitepress'

// VITEPRESS_BASE is set by CI when building versioned archives (e.g. /husako/v0.1.0/).
// Defaults to /husako/ for the latest (master) build.
const base = (process.env.VITEPRESS_BASE ?? '/husako/') as `/${string}/`

export default defineConfig({
  base,
  title: 'husako',
  description: 'Type-safe Kubernetes resource authoring in TypeScript',
  lang: 'en',

  themeConfig: {
    socialLinks: [
      { icon: 'github', link: 'https://github.com/nanazt/husako' },
    ],
    search: { provider: 'local' },
    footer: {
      message: 'Released under the MIT License.',
      copyright: 'Copyright © 2025 husako contributors',
    },

    nav: [
      { text: 'Guide', link: '/guide/getting-started' },
      { text: 'Reference', link: '/reference/cli' },
      { text: 'Advanced', link: '/advanced/plugins' },
      // Version switcher — add a new entry here when cutting a release:
      // { text: 'vX.Y.Z', link: 'https://nanazt.github.io/husako/vX.Y.Z/' }
      {
        text: 'master',
        items: [
          { text: 'master (latest)', link: 'https://nanazt.github.io/husako/' },
          // v0.1.0: { text: 'v0.1.0', link: 'https://nanazt.github.io/husako/v0.1.0/' },
        ],
      },
    ],

    sidebar: [
      {
        text: 'Guide',
        items: [
          { text: 'Getting Started',   link: '/guide/getting-started' },
          { text: 'Writing Resources', link: '/guide/writing-resources' },
          { text: 'Configuration',     link: '/guide/configuration' },
          { text: 'Templates',         link: '/guide/templates' },
          { text: 'Helm Chart Values', link: '/guide/helm' },
          { text: 'Official Plugins',  link: '/guide/plugins/' },
          { text: 'Flux CD',           link: '/guide/plugins/flux' },
        ],
      },
      {
        text: 'Reference',
        items: [
          { text: 'CLI Reference', link: '/reference/cli' },
          { text: 'Import System', link: '/reference/import-system' },
          { text: 'Builder API',   link: '/reference/builder-api' },
        ],
      },
      {
        text: 'Advanced',
        items: [
          { text: 'Writing a Plugin', link: '/advanced/plugins' },
        ],
      },
    ],
  },
})
