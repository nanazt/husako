import { defineConfig } from 'vitepress'

export default defineConfig({
  base: '/husako/',
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
