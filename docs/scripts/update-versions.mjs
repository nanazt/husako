#!/usr/bin/env node
// Usage: node docs/scripts/update-versions.mjs v0.2.0
//
// Updates docs/versions.json with the new release tag.
// Patch bumps (v0.1.0 → v0.1.1): replace latest in-place, no archive.
// Minor/major bumps (v0.1.x → v0.2.0): archive current latest, set new latest.
// Archive is capped at 4 entries (oldest dropped on overflow).

import { readFileSync, writeFileSync } from 'node:fs'
import { resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const VERSIONS_PATH = resolve(__dirname, '../versions.json')

function parseSemver(v) {
  const [major, minor, patch] = v.replace(/^v/, '').split('.').map(Number)
  return { major, minor, patch }
}

function compareSemverDesc(a, b) {
  const pa = parseSemver(a)
  const pb = parseSemver(b)
  return pb.major - pa.major || pb.minor - pa.minor || pb.patch - pa.patch
}

function isPatchBump(oldTag, newTag) {
  const o = parseSemver(oldTag)
  const n = parseSemver(newTag)
  return o.major === n.major && o.minor === n.minor
}

const newTag = process.argv[2]
if (!newTag || !/^v\d+\.\d+\.\d+/.test(newTag)) {
  console.error('Usage: node update-versions.mjs <tag>  (e.g. v0.2.0)')
  process.exit(1)
}

const data = JSON.parse(readFileSync(VERSIONS_PATH, 'utf-8'))
const { latest, archived } = data

if (latest === null) {
  // First release ever
  data.latest = newTag
} else if (isPatchBump(latest, newTag)) {
  // Patch bump: replace latest, keep archived unchanged
  data.latest = newTag
} else {
  // Minor/major bump: archive the old latest, set new latest
  const newArchived = [latest, ...archived]
    .sort(compareSemverDesc)
    .slice(0, 4)
  data.latest = newTag
  data.archived = newArchived
}

writeFileSync(VERSIONS_PATH, JSON.stringify(data, null, 2) + '\n')
console.log(`versions.json updated: latest=${data.latest}, archived=[${data.archived.join(', ')}]`)
