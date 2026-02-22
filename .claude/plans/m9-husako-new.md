# Milestone 9: `husako new <directory>` command

**Status**: Implemented

## Context

`husako init` initializes the current directory (generates `.husako/types/`, `tsconfig.json`). There is no way to scaffold a new project from scratch. `husako new <directory>` creates a directory and populates it with a ready-to-use project from a template, so users can get started quickly.

## Design

- `husako new my-app` creates `my-app/` with template files
- `--template` / `-t` flag selects template (default: `simple`)
- Three templates: **simple**, **project**, **multi-env**
- Does NOT run `husako init` — prints a hint to run it next
- Rejects non-empty existing directories

## Templates

All templates use canonical style (builders + fragments from `husako` and `k8s/*`).

### simple (default) — single-file project
```
my-app/
├── .gitignore
└── entry.ts
```

### project — multi-file structure
```
my-app/
├── .gitignore
├── env/
│   └── dev.ts
├── deployments/
│   └── nginx.ts
└── lib/
    ├── index.ts
    └── metadata.ts
```

### multi-env — multi-environment with parameterized shared base
```
my-app/
├── .gitignore
├── base/
│   ├── nginx.ts        # parameterized Deployment builder factory
│   └── service.ts      # parameterized Service builder factory
├── dev/
│   └── main.ts         # entry point for dev (replicas: 1, nginx:latest)
├── staging/
│   └── main.ts         # entry point for staging (replicas: 2, nginx:1.25)
└── release/
    └── main.ts         # entry point for release (replicas: 3, nginx:1.25)
```

Base resources are functions that return builders, parameterized by env config.

## Files modified

1. **`crates/husako-sdk/src/templates/`** — 11 new template files
2. **`crates/husako-sdk/src/lib.rs`** — 11 `include_str!` constants for templates
3. **`crates/husako-core/src/lib.rs`** — `TemplateName` enum, `ScaffoldOptions` struct, `scaffold()` function, 7 unit tests
4. **`crates/husako-cli/src/main.rs`** — `Commands::New` subcommand with `--template` / `-t` flag
5. **`crates/husako-cli/tests/integration.rs`** — 7 integration tests (scaffold + end-to-end render)

## Error handling

| Scenario | Error | Exit code |
|---|---|---|
| Non-empty directory | `HusakoError::InitIo` | 1 |
| Cannot create dir/write file | `HusakoError::InitIo` | 1 |
| Invalid template name | clap validation | 2 |

## Test results

197 tests total (was 183), all passing. Zero clippy warnings.
