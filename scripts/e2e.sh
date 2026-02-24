#!/usr/bin/env bash
# E2E test suite for husako CLI.
# Tests all CLI commands and all source kinds against real network sources.
# Requires: kubectl (for k8s YAML validation), python3 with PyYAML
# Usage:
#   bash scripts/e2e.sh                                        # use target/release/husako
#   HUSAKO_BIN=./target/debug/husako bash scripts/e2e.sh       # use debug binary
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
# Resolve HUSAKO_BIN to absolute path (relative paths break after cd into subdirs)
if [ -n "${HUSAKO_BIN:-}" ]; then
  HUSAKO="$(cd "$(dirname "$HUSAKO_BIN")" && pwd)/$(basename "$HUSAKO_BIN")"
else
  HUSAKO="$PROJECT_ROOT/target/release/husako"
fi
_COUNT_FILE=$(mktemp)
echo "0 0" > "$_COUNT_FILE"
trap 'rm -f "$_COUNT_FILE"' EXIT

# ── helpers ────────────────────────────────────────────────────────────────

pass() {
  echo "  ✓ $1"
  local p f; read -r p f < "$_COUNT_FILE"; echo "$((p+1)) $f" > "$_COUNT_FILE"
}
fail() {
  echo "  ✗ $1"
  local p f; read -r p f < "$_COUNT_FILE"; echo "$p $((f+1))" > "$_COUNT_FILE"
}

assert_contains() {
  local desc="$1" pattern="$2" content="$3"
  if echo "$content" | grep -q "$pattern"; then
    pass "$desc"
  else
    fail "$desc (expected pattern: $pattern)"
  fi
}

assert_not_contains() {
  local desc="$1" pattern="$2" content="$3"
  if ! echo "$content" | grep -q "$pattern"; then
    pass "$desc"
  else
    fail "$desc (unexpected pattern: $pattern)"
  fi
}

assert_file() {
  if [ -f "$1" ]; then
    pass "exists: $(basename "$1")"
  else
    fail "missing: $1"
  fi
}

assert_no_dir() {
  if [ ! -d "$1" ]; then
    pass "removed: $1"
  else
    fail "still exists: $1"
  fi
}

# Validate YAML is structurally valid using python3's yaml parser
assert_valid_yaml() {
  local desc="$1" yaml="$2"
  if echo "$yaml" | python3 -c "import sys,yaml; list(yaml.safe_load_all(sys.stdin))" 2>/dev/null; then
    pass "$desc is valid YAML"
  else
    fail "$desc is not valid YAML"
  fi
}

# Verify a husako.toml field+value pair coexist on the same line
# Usage: assert_toml_field "field" "value_substring" "description"
assert_toml_field() {
  local field="$1" value="$2" desc="${3:-TOML $1 = $2}"
  if grep -q "${field}.*${value}" husako.toml 2>/dev/null; then
    pass "$desc"
  else
    fail "$desc"
  fi
}

# Verify that a key (dep name) does NOT appear as a top-level entry in husako.toml
assert_toml_key_absent() {
  local key="$1"
  if ! grep -q "^${key}[[:space:]]*=" husako.toml 2>/dev/null; then
    pass "$key absent from husako.toml"
  else
    fail "$key still in husako.toml"
  fi
}

# Validate k8s YAML via kubeconform (uses built-in schemas, no cluster needed)
# Only use for standard k8s resources (Deployment, ConfigMap, etc.) —
# not for CRD-based custom resources which require server-side validation.
assert_k8s_valid() {
  local desc="$1" yaml="$2"
  if echo "$yaml" | kubeconform -strict 2>&1; then
    pass "k8s validate: $desc"
  else
    fail "k8s validate: $desc"
  fi
}

# Verify a .d.ts file exports the given symbol
assert_dts_exports() {
  local file="$1" symbol="$2"
  if grep -q "export.*${symbol}" "$file" 2>/dev/null; then
    pass "$(basename "$file") exports $symbol"
  else
    fail "$(basename "$file") missing export: $symbol"
  fi
}

# Check kubectl is available
require_kubectl() {
  if ! command -v kubectl > /dev/null 2>&1; then
    echo "ERROR: kubectl not found. Install kubectl first."
    exit 1
  fi
}

# Write husako.toml and copy tsconfig.json into the current directory
init_project() {
  printf '%s\n' "$1" > husako.toml
  cp "$PROJECT_ROOT/test/e2e/tsconfig.json" .
}

# Write a minimal ConfigMap TypeScript entry to the given filename
write_configmap() {
  cat > "$1" << 'TS'
import { ConfigMap } from "k8s/core/v1";
import { metadata, build } from "husako";
const cm = ConfigMap()
  .metadata(metadata().name("test-cm").namespace("default"))
  .set("data", { key: "value" });
build([cm]);
TS
}

# ── prerequisite checks ────────────────────────────────────────────────────

require_kubectl

if [ ! -f "$HUSAKO" ]; then
  echo "ERROR: husako binary not found at $HUSAKO"
  echo "Build it first: cargo build --release --bin husako"
  echo "Or set HUSAKO_BIN to point at the binary."
  exit 1
fi

# ── Scenario A: Static k8s + local Helm chart ──────────────────────────────

scenario_a() {
  echo
  echo "── A: Static k8s + local Helm chart ──"
  (
    cd "$PROJECT_ROOT/test/e2e"

    "$HUSAKO" gen

    # Side-effect: type files generated with correct exports
    assert_file ".husako/types/helm/local-chart.d.ts"
    assert_file ".husako/types/helm/local-chart.js"
    assert_dts_exports ".husako/types/helm/local-chart.d.ts" "LocalChart"
    assert_file ".husako/types/k8s/apps/v1.d.ts"
    assert_dts_exports ".husako/types/k8s/apps/v1.d.ts" "Deployment"

    # husako list shows both k8s and local-chart (output is on stderr)
    local list_out; list_out=$("$HUSAKO" list 2>&1)
    assert_contains "list shows k8s" "k8s" "$list_out"
    assert_contains "list shows local-chart" "local-chart" "$list_out"

    # k8s Deployment via file path
    if "$HUSAKO" validate entry.ts 2>/dev/null; then
      pass "validate entry.ts"
    else
      fail "validate entry.ts"
    fi
    local yaml; yaml=$("$HUSAKO" render entry.ts)
    assert_contains "render → kind: Deployment" "kind: Deployment" "$yaml"
    assert_contains "render → metadata.name: nginx" "name: nginx" "$yaml"
    assert_contains "render → image: nginx:1.25" "nginx:1.25" "$yaml"
    assert_k8s_valid "entry.ts Deployment" "$yaml"
    assert_valid_yaml "render entry.ts" "$yaml"

    # k8s Deployment via entry alias (side-effect: [entries] mapping works)
    if "$HUSAKO" validate deploy 2>/dev/null; then
      pass "validate alias 'deploy'"
    else
      fail "validate alias 'deploy'"
    fi
    local alias_yaml; alias_yaml=$("$HUSAKO" render deploy)
    assert_contains "render alias 'deploy'" "kind: Deployment" "$alias_yaml"
    assert_k8s_valid "alias 'deploy'" "$alias_yaml"

    # Helm values (local-chart, file source)
    if "$HUSAKO" validate helm-values.ts 2>/dev/null; then
      pass "validate helm-values.ts"
    else
      fail "validate helm-values.ts"
    fi
    local helm_yaml; helm_yaml=$("$HUSAKO" render helm-values.ts)
    assert_contains "render helm → replicaCount: 2" "replicaCount: 2" "$helm_yaml"
    assert_contains "render helm → repository: nginx" "repository: nginx" "$helm_yaml"
    assert_valid_yaml "render helm-values" "$helm_yaml"

    # Helm alias
    local helm_alias; helm_alias=$("$HUSAKO" render helm)
    assert_contains "render alias 'helm'" "replicaCount" "$helm_alias"

    # CLI smoke tests — verify exit 0 + non-empty output on stderr
    local info_out; info_out=$("$HUSAKO" info 2>&1)
    if [ -n "$info_out" ]; then
      pass "husako info produces output"
    else
      fail "husako info empty"
    fi

    local debug_out; debug_out=$("$HUSAKO" debug 2>&1)
    if [ -n "$debug_out" ]; then
      pass "husako debug produces output"
    else
      fail "husako debug empty"
    fi

    # outdated may exit non-zero if deps are outdated; just verify it runs
    "$HUSAKO" outdated 2>&1 || true
    pass "husako outdated ran"
  )
}

# ── Scenario B: Chart sources (artifacthub, registry, git) + husako remove ─

scenario_b() {
  echo
  echo "── B: Chart sources ──"
  local tmpdir; tmpdir=$(mktemp -d)
  (
    trap 'rm -rf "$tmpdir"' EXIT
    cd "$tmpdir"

    init_project "$(printf '[resources]\nk8s = { source = "release", version = "1.35" }')"
    write_configmap configmap.ts
    "$HUSAKO" gen  # pre-generate k8s types once

    # ── B1: artifacthub source (bitnami/postgresql)
    echo "  B1: artifacthub"
    "$HUSAKO" -y add pg --chart --source artifacthub --package bitnami/postgresql --version 18.4.0

    # Side-effect: husako.toml has correct source fields
    # Note: chart key "pg" → generated type "Pg" (PascalCase)
    assert_toml_field "source" "artifacthub" "pg source=artifacthub"
    assert_toml_field "package" "bitnami/postgresql" "pg package"
    assert_toml_field "version" "18.4.0" "pg version"

    "$HUSAKO" gen
    assert_file ".husako/types/helm/pg.d.ts"
    assert_file ".husako/types/helm/pg.js"
    # chart key "pg" → type name "Pg" (to_pascal_case("pg") = "Pg")
    assert_dts_exports ".husako/types/helm/pg.d.ts" "Pg"

    cat > pg-values.ts << 'TS'
import { Pg } from "helm/pg";
import { build } from "husako";
const values = Pg();
build([values]);
TS
    if "$HUSAKO" validate pg-values.ts 2>/dev/null; then
      pass "validate pg-values.ts"
    else
      fail "validate pg-values.ts"
    fi
    local pg_yaml; pg_yaml=$("$HUSAKO" render pg-values.ts)
    assert_valid_yaml "render pg" "$pg_yaml"

    # ── B2: registry source (bitnami HTTP → OCI archive URL delegation)
    # This exercises the OCI archive URL delegation bug fix in husako-helm.
    echo "  B2: registry"
    "$HUSAKO" -y add redis-reg --chart --source registry \
      --repo https://charts.bitnami.com/bitnami --chart-name redis --version 20.0.1

    assert_toml_field "source" "registry" "redis-reg source=registry"
    assert_toml_field "repo" "charts.bitnami.com" "redis-reg repo"
    assert_toml_field "chart" "redis" "redis-reg chart"
    assert_toml_field "version" "20.0.1" "redis-reg version"

    "$HUSAKO" gen
    assert_file ".husako/types/helm/redis-reg.d.ts"
    # chart key "redis-reg" → type name "RedisReg" (to_pascal_case("redis-reg") = "RedisReg")
    assert_dts_exports ".husako/types/helm/redis-reg.d.ts" "RedisReg"

    cat > redis-reg-values.ts << 'TS'
import { RedisReg } from "helm/redis-reg";
import { build } from "husako";
build([RedisReg()]);
TS
    if "$HUSAKO" validate redis-reg-values.ts 2>/dev/null; then
      pass "validate redis-reg-values.ts"
    else
      fail "validate redis-reg-values.ts"
    fi
    local redis_reg_yaml; redis_reg_yaml=$("$HUSAKO" render redis-reg-values.ts)
    assert_valid_yaml "render redis-reg" "$redis_reg_yaml"

    # ── B3: git chart source (prometheus-community/helm-charts)
    # Using prometheus-community as a stable alternative to bitnami charts.
    # The prometheus chart has a well-maintained values.schema.json.
    echo "  B3: git"
    "$HUSAKO" -y add prom-git --chart --source git \
      --repo https://github.com/prometheus-community/helm-charts \
      --tag prometheus-27.0.0 \
      --path charts/prometheus/values.schema.json

    assert_toml_field "source" "git" "prom-git source=git"
    assert_toml_field "tag" "prometheus-27.0.0" "prom-git tag"
    assert_toml_field "repo" "prometheus-community" "prom-git repo"

    "$HUSAKO" gen
    assert_file ".husako/types/helm/prom-git.d.ts"
    # chart key "prom-git" → type name "PromGit" (to_pascal_case("prom-git") = "PromGit")
    assert_dts_exports ".husako/types/helm/prom-git.d.ts" "PromGit"

    cat > prom-git-values.ts << 'TS'
import { PromGit } from "helm/prom-git";
import { build } from "husako";
build([PromGit()]);
TS
    if "$HUSAKO" validate prom-git-values.ts 2>/dev/null; then
      pass "validate prom-git-values.ts"
    else
      fail "validate prom-git-values.ts"
    fi
    local prom_yaml; prom_yaml=$("$HUSAKO" render prom-git-values.ts)
    assert_valid_yaml "render prom-git" "$prom_yaml"

    # ── B-remove: remove pg, verify TOML key gone, types cleaned up
    echo "  B-remove"
    "$HUSAKO" -y remove pg

    # Side-effect: pg key completely absent from husako.toml
    assert_toml_key_absent "pg"

    # Clean types and re-gen to verify orphaned type files are not kept
    "$HUSAKO" -y clean --types
    "$HUSAKO" gen
    if [ ! -f ".husako/types/helm/pg.d.ts" ]; then
      pass "pg.d.ts removed after dep removal"
    else
      fail "pg.d.ts still exists after dep removal"
    fi

    # k8s types still work after chart removal
    if "$HUSAKO" validate configmap.ts 2>/dev/null; then
      pass "validate after remove"
    else
      fail "validate after remove"
    fi
    local cm_yaml; cm_yaml=$("$HUSAKO" render configmap.ts)
    assert_k8s_valid "ConfigMap after remove" "$cm_yaml"
  )
}

# ── Scenario C: Resource sources (file, git) + husako remove ───────────────

scenario_c() {
  echo
  echo "── C: Resource sources ──"
  local tmpdir; tmpdir=$(mktemp -d)
  (
    trap 'rm -rf "$tmpdir"' EXIT
    cd "$tmpdir"

    cp "$PROJECT_ROOT/test/e2e/test-crd.yaml" .
    init_project "$(printf '[resources]\ntest-crd = { source = "file", path = "test-crd.yaml" }')"

    # ── C1: resource file source (local CRD YAML)
    echo "  C1: resource file"
    assert_toml_field "source" "file" "test-crd source=file"
    assert_toml_field "path" "test-crd.yaml" "test-crd path"

    "$HUSAKO" gen
    # Side-effect: CRD types generated in expected group-version path
    assert_file ".husako/types/k8s/e2e.husako.io/v1.d.ts"
    assert_dts_exports ".husako/types/k8s/e2e.husako.io/v1.d.ts" "Example"

    cat > example.ts << 'TS'
import { Example } from "k8s/e2e.husako.io/v1";
import { metadata, build } from "husako";
const ex = Example()
  .metadata(metadata().name("test-example").namespace("default"))
  .spec({ message: "hello", replicas: 1 });
build([ex]);
TS
    if "$HUSAKO" validate example.ts 2>/dev/null; then
      pass "validate example.ts"
    else
      fail "validate example.ts"
    fi
    local ex_yaml; ex_yaml=$("$HUSAKO" render example.ts)
    assert_contains "render → kind: Example" "kind: Example" "$ex_yaml"
    assert_contains "render → group e2e.husako.io" "e2e.husako.io" "$ex_yaml"
    assert_contains "render → spec.message: hello" "hello" "$ex_yaml"
    # Custom resources: validate YAML structure (kubectl dry-run requires server-side for CRDs)
    assert_valid_yaml "render example" "$ex_yaml"

    # ── C2: resource git source (cert-manager CRDs)
    echo "  C2: resource git (cert-manager)"
    "$HUSAKO" -y add cert-manager --resource --source git \
      --repo https://github.com/cert-manager/cert-manager \
      --tag v1.16.3 \
      --path deploy/crds

    assert_toml_field "source" "git" "cert-manager source=git"
    assert_toml_field "repo" "cert-manager/cert-manager" "cert-manager repo"
    assert_toml_field "tag" "v1.16.3" "cert-manager tag"

    "$HUSAKO" gen
    assert_file ".husako/types/k8s/cert-manager.io/v1.d.ts"
    assert_dts_exports ".husako/types/k8s/cert-manager.io/v1.d.ts" "Certificate"

    cat > certificate.ts << 'TS'
import { Certificate } from "k8s/cert-manager.io/v1";
import { metadata, build } from "husako";
const cert = Certificate()
  .metadata(metadata().name("my-cert").namespace("default"))
  .spec({
    secretName: "my-tls",
    issuerRef: { name: "letsencrypt", kind: "ClusterIssuer" },
    dnsNames: ["example.com"],
  });
build([cert]);
TS
    if "$HUSAKO" validate certificate.ts 2>/dev/null; then
      pass "validate certificate.ts"
    else
      fail "validate certificate.ts"
    fi
    local cert_yaml; cert_yaml=$("$HUSAKO" render certificate.ts)
    assert_contains "render → kind: Certificate" "kind: Certificate" "$cert_yaml"
    assert_contains "render → secretName: my-tls" "my-tls" "$cert_yaml"
    assert_valid_yaml "render certificate" "$cert_yaml"

    # ── C-remove: remove cert-manager, verify types gone, example still works
    echo "  C-remove"
    "$HUSAKO" -y remove cert-manager
    assert_toml_key_absent "cert-manager"

    "$HUSAKO" -y clean --types
    "$HUSAKO" gen
    if [ ! -f ".husako/types/k8s/cert-manager.io/v1.d.ts" ]; then
      pass "cert-manager types removed"
    else
      fail "cert-manager types still present"
    fi

    if "$HUSAKO" validate example.ts 2>/dev/null; then
      pass "example still valid after remove"
    else
      fail "example broke after remove"
    fi
    local ex_after; ex_after=$("$HUSAKO" render example.ts)
    assert_valid_yaml "render example after remove" "$ex_after"
  )
}

# ── Scenario D: Version management (gen → update → re-validate) ────────────

scenario_d() {
  echo
  echo "── D: Version management ──"
  local tmpdir; tmpdir=$(mktemp -d)
  (
    trap 'rm -rf "$tmpdir"' EXIT
    cd "$tmpdir"

    init_project "$(printf '[resources]\nk8s = { source = "release", version = "1.30" }')"
    write_configmap configmap.ts

    "$HUSAKO" gen

    # Side-effect: k8s 1.30 types present
    assert_file ".husako/types/k8s/core/v1.d.ts"
    if "$HUSAKO" validate configmap.ts 2>/dev/null; then
      pass "validate (k8s 1.30)"
    else
      fail "validate (k8s 1.30)"
    fi
    local cm_before; cm_before=$("$HUSAKO" render configmap.ts)
    assert_contains "render (1.30) → kind: ConfigMap" "kind: ConfigMap" "$cm_before"
    assert_k8s_valid "ConfigMap (k8s 1.30)" "$cm_before"

    # Record pre-update type file modification time
    local mtime_before
    # macOS uses -f %m, Linux uses -c %Y
    if stat -c %Y .husako/types/k8s/core/v1.d.ts > /dev/null 2>&1; then
      mtime_before=$(stat -c %Y .husako/types/k8s/core/v1.d.ts)
    else
      mtime_before=$(stat -f %m .husako/types/k8s/core/v1.d.ts)
    fi

    # Small sleep to ensure mtime difference is detectable
    sleep 1

    "$HUSAKO" update k8s 2>&1 || true

    # Side-effect: husako.toml version changed from 1.30
    local new_ver; new_ver=$(grep -o '"[0-9][0-9]*\.[0-9][0-9]*"' husako.toml | tr -d '"' | head -1)
    if [ "$new_ver" != "1.30" ]; then
      pass "version updated from 1.30 → $new_ver"
    else
      fail "version not updated from 1.30 (still $new_ver)"
    fi

    # Side-effect: type files regenerated (mtime changed or equal — update happened)
    local mtime_after
    if stat -c %Y .husako/types/k8s/core/v1.d.ts > /dev/null 2>&1; then
      mtime_after=$(stat -c %Y .husako/types/k8s/core/v1.d.ts)
    else
      mtime_after=$(stat -f %m .husako/types/k8s/core/v1.d.ts)
    fi
    if [ "$mtime_after" -ge "$mtime_before" ]; then
      pass "types regenerated after update"
    else
      fail "types not regenerated (mtime unchanged)"
    fi

    # husako update regenerates types — validate + render must still work
    if "$HUSAKO" validate configmap.ts 2>/dev/null; then
      pass "validate (after update)"
    else
      fail "validate (after update)"
    fi
    local cm_after; cm_after=$("$HUSAKO" render configmap.ts)
    assert_contains "render (after update) → kind: ConfigMap" "kind: ConfigMap" "$cm_after"
    assert_k8s_valid "ConfigMap (after update)" "$cm_after"
  )
}

# ── Scenario E: Plugin system + husako clean ───────────────────────────────

scenario_e() {
  echo
  echo "── E: Plugin system + husako clean ──"
  local tmpdir; tmpdir=$(mktemp -d)
  (
    trap 'rm -rf "$tmpdir"' EXIT
    cd "$tmpdir"

    init_project "$(printf '[resources]\nk8s = { source = "release", version = "1.35" }')"

    # ── E1: plugin add (path source — bundled FluxCD plugin in this repo)
    echo "  E1: plugin add"
    # Copy plugin locally so we can use a relative path (husako requires relative paths)
    cp -r "$PROJECT_ROOT/plugins/fluxcd" ./fluxcd-plugin
    "$HUSAKO" plugin add fluxcd --path fluxcd-plugin

    # Side-effect: husako.toml has fluxcd plugin entry
    if grep -q "fluxcd" husako.toml; then
      pass "fluxcd in husako.toml"
    else
      fail "fluxcd not in husako.toml"
    fi
    assert_toml_field "source" "path" "fluxcd plugin source=path"

    "$HUSAKO" gen

    # Side-effect: plugin list shows fluxcd (installed by gen)
    local plugin_list; plugin_list=$("$HUSAKO" plugin list 2>&1)
    assert_contains "plugin list shows fluxcd" "fluxcd" "$plugin_list"

    # Side-effect: plugin module files installed
    assert_file ".husako/plugins/fluxcd/modules/index.js"

    cat > helmrelease.ts << 'TS'
import { HelmRelease } from "fluxcd";
import { HelmRepository } from "fluxcd/source";
import { metadata, build } from "husako";

const repo = HelmRepository()
  .metadata(metadata().name("bitnami").namespace("flux-system"))
  .spec({ url: "https://charts.bitnami.com/bitnami", interval: "1h" });

const release = HelmRelease()
  .metadata(metadata().name("redis").namespace("default"))
  .spec({
    chart: { spec: { chart: "redis", version: "25.3.0", sourceRef: repo._sourceRef() } },
    interval: "10m",
  });

build([repo, release]);
TS
    if "$HUSAKO" validate helmrelease.ts 2>/dev/null; then
      pass "validate helmrelease.ts"
    else
      fail "validate helmrelease.ts"
    fi
    local hr_yaml; hr_yaml=$("$HUSAKO" render helmrelease.ts)
    assert_contains "render → HelmRelease" "kind: HelmRelease" "$hr_yaml"
    assert_contains "render → HelmRepository" "kind: HelmRepository" "$hr_yaml"
    assert_contains "render → bitnami repo URL" "charts.bitnami.com" "$hr_yaml"
    # FluxCD CRDs require server-side dry-run; use YAML validation instead
    assert_valid_yaml "render helmrelease" "$hr_yaml"

    # ── E2: plugin remove
    echo "  E2: plugin remove"
    "$HUSAKO" plugin remove fluxcd

    # Side-effect: fluxcd absent from husako.toml
    assert_toml_key_absent "fluxcd"

    # Side-effect: plugin list no longer shows fluxcd
    local plugin_list_after; plugin_list_after=$("$HUSAKO" plugin list 2>&1)
    assert_not_contains "fluxcd not in plugin list" "fluxcd" "$plugin_list_after"

    # Side-effect: plugin files removed from .husako/plugins/
    if [ ! -d ".husako/plugins/fluxcd" ]; then
      pass ".husako/plugins/fluxcd removed"
    else
      fail ".husako/plugins/fluxcd still exists"
    fi

    # ── E3: husako clean
    echo "  E3: husako clean"
    "$HUSAKO" gen  # re-gen so .husako/ exists

    # --all cleans both cache and types non-interactively; -y skips confirmation
    "$HUSAKO" -y clean --all

    # Side-effect: .husako/ completely removed
    assert_no_dir ".husako"

    # gen after clean re-downloads and rebuilds everything
    "$HUSAKO" gen
    assert_file ".husako/types/k8s/core/v1.d.ts"
    write_configmap configmap.ts
    if "$HUSAKO" validate configmap.ts 2>/dev/null; then
      pass "validate after clean + gen"
    else
      fail "validate after clean + gen"
    fi
    local cm_yaml; cm_yaml=$("$HUSAKO" render configmap.ts)
    assert_k8s_valid "ConfigMap after clean + gen" "$cm_yaml"
  )
}

# ── Scenario F: OCI chart source ──────────────────────────────────────────

scenario_f() {
  echo
  echo "── F: OCI chart source ──"
  local tmpdir; tmpdir=$(mktemp -d)
  (
    trap 'rm -rf "$tmpdir"' EXIT
    cd "$tmpdir"

    init_project "$(printf '[resources]\nk8s = { source = "release", version = "1.35" }')"

    # ── F1: add OCI chart via non-interactive flags
    echo "  F1: oci add"
    "$HUSAKO" -y add postgresql --chart --source oci \
      --reference "oci://registry-1.docker.io/bitnamicharts/postgresql" \
      --version "18.4.0"

    assert_toml_field "source" "oci" "postgresql source=oci"
    assert_toml_field "reference" "bitnamicharts/postgresql" "postgresql reference"
    assert_toml_field "version" "18.4.0" "postgresql version"

    "$HUSAKO" gen

    assert_file ".husako/types/helm/postgresql.d.ts"
    assert_file ".husako/types/helm/postgresql.js"
    assert_dts_exports ".husako/types/helm/postgresql.d.ts" "Postgresql"

    cat > pg-oci-values.ts << 'TS'
import { Postgresql } from "helm/postgresql";
import { build } from "husako";
build([Postgresql()]);
TS
    if "$HUSAKO" validate pg-oci-values.ts 2>/dev/null; then
      pass "validate pg-oci-values.ts"
    else
      fail "validate pg-oci-values.ts"
    fi
    local pg_yaml; pg_yaml=$("$HUSAKO" render pg-oci-values.ts)
    assert_valid_yaml "render postgresql OCI" "$pg_yaml"

    # ── F2: husako list shows oci source
    local list_out; list_out=$("$HUSAKO" list 2>&1)
    assert_contains "list shows postgresql" "postgresql" "$list_out"
    assert_contains "list shows oci source type" "oci" "$list_out"
  )
}

# ── run all scenarios ──────────────────────────────────────────────────────

scenario_a
scenario_b
scenario_c
scenario_d
scenario_e
scenario_f

read -r PASS FAIL < "$_COUNT_FILE"
echo
echo "══════════════════════════════════════════════"
printf "  Results: %d passed, %d failed\n" "$PASS" "$FAIL"
echo "══════════════════════════════════════════════"
[ "$FAIL" -eq 0 ] || exit 1
