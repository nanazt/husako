import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vitepress'

const __dirname = dirname(fileURLToPath(import.meta.url))
const versions = JSON.parse(
  readFileSync(resolve(__dirname, '../versions.json'), 'utf-8')
)

const BASE = 'https://nanazt.github.io/husako/'
const RELEASES = 'https://github.com/nanazt/husako/releases/tag/'

function parseSemver(v: string) {
  const [major, minor, patch] = v.replace(/^v/, '').split('.').map(Number)
  return { major, minor, patch }
}

function compareSemverDesc(a: string, b: string) {
  const pa = parseSemver(a)
  const pb = parseSemver(b)
  return pb.major - pa.major || pb.minor - pa.minor || pb.patch - pa.patch
}

const versionNav = versions.latest
  ? {
      text: versions.latest,
      items: [
        { text: `${versions.latest} (latest)`, link: BASE },
        { text: 'master (dev)', link: BASE },
        ...[...versions.archived]
          .sort(compareSemverDesc)
          .map((v: string) => ({ text: v, link: RELEASES + v })),
      ],
    }
  : undefined

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
      ...(versionNav ? [versionNav] : []),
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
