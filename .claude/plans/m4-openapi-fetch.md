# Milestone 4: OpenAPI v3 Fetch and Cache

**Status**: Completed
**Commit**: `c3b613d`

## Goal

Fetch Kubernetes OpenAPI v3 specifications from a cluster API server or local directory, with disk caching and offline mode support.

## Deliverables

- OpenAPI v3 discovery endpoint parsing (`/openapi/v3`)
- Per-group-version spec fetching (`/openapi/v3/apis/apps/v1`, etc.)
- Disk cache under `.husako/cache/openapi/` with content-hash naming
- Offline mode: use cache only, no network requests
- Local directory source: read pre-fetched spec files from disk
- Bearer token authentication support

## Architecture Decisions

### Crate: `husako-openapi`

Single-purpose crate with clean API:

```rust
pub struct OpenApiClient { ... }
pub struct FetchOptions {
    pub source: OpenApiSource,
    pub cache_dir: PathBuf,
    pub offline: bool,
}
pub enum OpenApiSource {
    Url { base_url: String, bearer_token: Option<String> },
    Directory(PathBuf),
}
```

### Fetch Strategy

```
1. URL source:
   a. GET /openapi/v3 -> discover group-version paths
   b. For each path: check disk cache (by ETag/hash)
   c. If not cached or stale: fetch, write to cache
   d. Return HashMap<path, serde_json::Value>

2. Directory source:
   a. Walk directory for *.json files
   b. Parse relative path as group-version path
   c. Return HashMap<path, serde_json::Value>

3. Offline mode:
   a. Read only from cache_dir
   b. Never make network requests
   c. Error if cache is empty
```

### Cache Layout

```
.husako/cache/openapi/
  apis_apps_v1.json       # sanitized path as filename
  apis_batch_v1.json
  api_v1.json
```

### Error Types

```rust
pub enum OpenApiError {
    Network(String),       // HTTP/connection errors
    Parse(String),         // JSON parse errors
    Io(String),            // File I/O errors
    NoSpecs(String),       // No specs found
}
```

## Files Created

```
crates/husako-openapi/src/lib.rs     # OpenApiClient, FetchOptions, main logic
crates/husako-openapi/src/fetch.rs   # HTTP fetching logic
crates/husako-openapi/src/cache.rs   # Disk cache read/write
```

## Dependencies

- `reqwest` (blocking, json) — HTTP client
- `serde_json` — JSON parsing
- `thiserror` — error types

## Tests

### Unit Tests

- Discovery endpoint parsing
- Cache hit/miss behavior
- Directory source loading
- Offline mode with empty cache -> error

### Integration Tests (with mockito)

- Mock server serves discovery + spec endpoints
- First fetch populates cache
- Second fetch uses cache (no network)
- Offline mode works with populated cache

## Acceptance Criteria

- [x] Fetches OpenAPI specs from URL source
- [x] Caches specs to disk
- [x] Directory source reads local files
- [x] Offline mode uses cache only
- [x] Bearer token authentication supported
- [x] No external network in tests (mockito)
