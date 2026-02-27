# Coding Guidelines

Project-specific patterns and clippy fixes. General style (naming, formatting) is enforced by `rustfmt` and covered in CLAUDE.md.

---

## Clippy — Common Failures

The project runs `cargo clippy --workspace --all-targets --all-features -- -D warnings`. Any lint is a CI failure.

### `collapsible_if`

```rust
// Bad
if condition_a {
    if condition_b {
        do_thing();
    }
}
// Good
if condition_a && condition_b {
    do_thing();
}
```

### `collapsible_else_if`

```rust
// Bad
} else {
    if condition {
        do_thing();
    }
}
// Good
} else if condition {
    do_thing();
}
```

### `uninlined_format_args`

Simple identifiers go inline; expressions with `.` or method calls stay outside.

```rust
// Bad
eprintln!("error: {}", e);
format!("name: {}", name);
// Good
eprintln!("error: {e}");
format!("name: {name}");
format!("path: {}", path.display())  // method call stays out
```

### `redundant_closure`

```rust
// Bad
.map(|x| foo(x))
// Good
.map(foo)
```

### `or_fun_call`

```rust
// Bad
.unwrap_or(Vec::new())
.unwrap_or(String::new())
// Good
.unwrap_or_default()
```

### `map_unwrap_or`

```rust
// Bad
option.map(|x| f(x)).unwrap_or(default)
// Good
option.map_or(default, |x| f(x))
```

### `single_match`

```rust
// Bad
match value {
    Some(x) => do_thing(x),
    _ => {}
}
// Good
if let Some(x) = value {
    do_thing(x);
}
```

---

## When `#[allow(clippy::...)]` Is Justified

The entire codebase has exactly **2** suppression attributes. Use `#[allow]` only for genuine false positives where refactoring would make the code worse. Always add a comment explaining why.

```rust
// OK: dispatch function with many optional CLI parameters; restructuring would obscure intent
#[allow(clippy::too_many_arguments)]
fn resolve_add_target(...) { ... }
```

---

## Async Patterns

### `drop_in_background` — large heap allocations at async tail

Dropping a large `HashMap` or generated file map synchronously at the end of an async function blocks the executor. Move it to a blocking thread:

```rust
/// Drop a large value on a blocking thread to avoid holding the async executor.
fn drop_in_background<T: Send + 'static>(value: T) {
    drop(tokio::task::spawn_blocking(move || drop(value)));
}
```

Applied in `husako-core/src/lib.rs` after type generation pipelines:

```rust
drop_in_background(result);        // GenerateResult (large generated TS file map)
drop_in_background(chart_schemas); // Helm schema map
```

When to apply: any large allocation dropped at the tail of an async function, especially after sequential I/O.

### `spawn_blocking` — CPU-bound / FFI work

QuickJS execution is synchronous. Wrap it so it doesn't starve the tokio runtime:

```rust
tokio::task::spawn_blocking(move || execute_sync(&js_source, &options))
    .await
    .map_err(|e| RuntimeError::Init(e.to_string()))?
```

Rule: CPU-bound or FFI-bound code → `spawn_blocking`. File/network I/O → `tokio::fs` / `tokio::process`.
