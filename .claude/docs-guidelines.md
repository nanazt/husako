# Documentation Guidelines

## Project description

Open with an `husako is a...` sentence in README and getting-started pages. Example:

> husako is a CLI tool for writing Kubernetes resources in TypeScript.

## Feature messaging

Use generic, descriptive terms in user-facing copy — not internal implementation names.

| Use | Avoid |
|-----|-------|
| TypeScript compiler | oxc, tsc |
| JavaScript runtime | QuickJS, Node.js |
| bundles its own | embeds |

Exception: technical docs, architecture pages, and "How it works" sections may name specific tools (oxc, QuickJS) because readers there want implementation details.

## VitePress feature cards

- Always include an `icon:` field (emoji preferred)
- Keep `details` to 1–2 sentences
- "Self-Contained" is the preferred label for the bundled-runtime card (not "No Node.js Required")

## Plugin documentation

**Naming**: Use the upstream project's canonical name as the plugin identifier and import path. Example: `fluxcd`, not `flux` or `flux-cd`.

**Compatibility**: State version compatibility near the top of the plugin README. Example: "Compatible with FluxCD v2.x."

**Versions table**: Consolidate upstream release version and individual component versions into a single "Compatibility" table at the bottom. Do not keep separate "Bundled CRD Versions" and version statement sections.

**Related features**: Mention husako built-in features that complement the plugin. For FluxCD, mention Helm chart type generation alongside `HelmRelease.values()`.

## Stability notice

husako is unstable. Any README or getting-started page that describes behavior should note that if behavior differs from documentation, users should file an issue at https://github.com/nanazt/husako/issues.

## General

- Write in English (see CLAUDE.md)
- Avoid AI writing patterns (see https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing)
