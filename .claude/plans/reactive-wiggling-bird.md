# Plan: Replace "real language" copy across docs and README

## Context

The phrase "A Real Language" / "real language features" / "real programming language"
appears in three places. The tone is defensive and invites comparison. Replacing with
"Resources as Code" framing (concrete, fits the IaC ecosystem vocabulary).

---

## Changes

### 1. `docs/index.md` — feature card title

**Branch: `feature/docs-site`**

```diff
-  - title: A Real Language
+  - title: Resources as Code
     details: Use functions, variables, loops, and imports to compose resources. Share
              common metadata, parameterize environments, reuse pod templates across
              deployments.
```

`details` text stays as-is — it already explains what "Resources as Code" means.

---

### 2. `docs/index.md` — hero tagline

```diff
-  tagline: Type safety, autocomplete, and real language features — instead of
-           templating hacks on top of YAML.
+  tagline: Type safety, autocomplete, and the full TypeScript language — instead of
+           templating hacks on top of YAML.
```

---

### 3. `README.md` — opening paragraph

```diff
-  husako compiles TypeScript to Kubernetes YAML. You get type safety, autocomplete,
-  and the full expressiveness of a real programming language — functions, variables,
-  loops, imports — instead of templating hacks on top of YAML.
+  husako compiles TypeScript to Kubernetes YAML. You get type safety, autocomplete,
+  and the full expressiveness of TypeScript — functions, variables, loops, imports —
+  instead of templating hacks on top of YAML.
```

---

## Target branch

All three files live in (or are accessible from) `feature/docs-site`. Commit there so
the change is included when that branch merges to master.

## Verification

After edit, confirm:
- `docs/index.md` hero tagline: contains `full TypeScript language`
- `docs/index.md` feature card: title is `Resources as Code`
- `README.md`: no remaining `real programming language` or `real language features`
