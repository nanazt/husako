use std::collections::HashSet;
use std::fmt::Write;

use crate::schema::{PropertyInfo, SchemaInfo, TsType, has_complex_property};

/// Properties to skip when generating spec property methods on resource builders.
const RESOURCE_SPEC_SKIP: &[&str] = &["status", "apiVersion", "kind", "metadata"];

/// Properties to skip when generating top-level property methods on resource builders.
/// These fields have dedicated methods or must not be overridden.
const RESOURCE_TOP_LEVEL_SKIP: &[&str] = &["apiVersion", "kind", "metadata", "spec", "status"];

/// Resource factory function name matches the class name (PascalCase).
fn to_factory_name(class_name: &str) -> String {
    class_name.to_string()
}

/// Format a TsType into its TypeScript string representation.
pub fn format_ts_type(ty: &TsType) -> String {
    match ty {
        TsType::String => "string".to_string(),
        TsType::Number => "number".to_string(),
        TsType::Boolean => "boolean".to_string(),
        TsType::IntOrString => "number | string".to_string(),
        TsType::Array(inner) => format!("{}[]", format_ts_type(inner)),
        TsType::Map(val) => format!("Record<string, {}>", format_ts_type(val)),
        TsType::Ref(name) => name.clone(),
        TsType::Any => "any".to_string(),
    }
}

/// Emit a single TypeScript interface.
pub fn emit_interface(schema: &SchemaInfo) -> String {
    let mut out = String::new();

    if let Some(desc) = &schema.description {
        let _ = writeln!(out, "/** {desc} */");
    }
    let _ = writeln!(out, "export interface {} {{", schema.ts_name);

    for prop in &schema.properties {
        emit_property(&mut out, prop);
    }

    let _ = writeln!(out, "}}");
    out
}

fn emit_property(out: &mut String, prop: &PropertyInfo) {
    if let Some(desc) = &prop.description {
        let _ = writeln!(out, "  /** {desc} */");
    }
    let opt = if prop.required { "" } else { "?" };
    let _ = writeln!(
        out,
        "  {}{}: {};",
        prop.name,
        opt,
        format_ts_type(&prop.ts_type)
    );
}

/// Find the Spec type name for a resource schema by looking for a `spec` property with a Ref type.
fn find_spec_type(schema: &SchemaInfo) -> Option<String> {
    schema.properties.iter().find_map(|p| {
        if p.name == "spec"
            && let TsType::Ref(ref name) = p.ts_type
        {
            return Some(name.clone());
        }
        None
    })
}

/// Find the spec schema for a resource by looking up its spec type name.
fn find_spec_schema<'a>(
    resource: &SchemaInfo,
    all_schemas: &'a [&'a SchemaInfo],
) -> Option<&'a SchemaInfo> {
    let spec_type = find_spec_type(resource)?;
    all_schemas.iter().find(|s| s.ts_name == spec_type).copied()
}

/// Check if a spec schema has a `template` property referencing PodTemplateSpec.
fn has_pod_template(spec_schema: &SchemaInfo) -> bool {
    spec_schema.properties.iter().any(|p| {
        p.name == "template" && matches!(&p.ts_type, TsType::Ref(name) if name == "PodTemplateSpec")
    })
}

/// Check if a schema should get a `_SchemaBuilder` subclass.
/// A schema benefits from a builder when it has at least one property with a
/// `Ref` or `Array(Ref)` type, indicating deep nesting.
pub fn should_generate_builder(schema: &SchemaInfo) -> bool {
    // Only non-GVK schemas (intermediate types) get schema builders
    schema.gvk.is_none() && has_complex_property(schema)
}

/// Emit typed chainable method declarations for each property of a schema (.d.ts).
fn emit_property_methods_dts(out: &mut String, props: &[PropertyInfo], skip: &[&str]) {
    for prop in props {
        if skip.contains(&prop.name.as_str()) {
            continue;
        }
        if let Some(desc) = &prop.description {
            let _ = writeln!(out, "  /** {desc} */");
        }
        // resources: ResourceRequirements — also accept the ResourceRequirementsChain from k8s/_chains.
        let ts_type = if prop.name == "resources"
            && matches!(&prop.ts_type, TsType::Ref(name) if name == "ResourceRequirements")
        {
            format!(
                "{} | import(\"k8s/_chains\").ResourceRequirementsChain",
                format_ts_type(&prop.ts_type)
            )
        } else {
            format_ts_type(&prop.ts_type)
        };
        let _ = writeln!(out, "  {}(value: {}): this;", prop.name, ts_type);
    }
}

/// Emit JS chainable method implementations using `_set` for each property.
fn emit_property_methods_js_set(out: &mut String, props: &[PropertyInfo], skip: &[&str]) {
    for prop in props {
        if skip.contains(&prop.name.as_str()) {
            continue;
        }
        let _ = writeln!(
            out,
            "  {}(v) {{ return this._set(\"{}\", v); }}",
            prop.name, prop.name
        );
    }
}

/// Emit JS chainable method implementations using `set` for top-level resource fields.
fn emit_property_methods_js_top(out: &mut String, props: &[PropertyInfo], skip: &[&str]) {
    for prop in props {
        if skip.contains(&prop.name.as_str()) {
            continue;
        }
        let _ = writeln!(
            out,
            "  {}(v) {{ return this.set(\"{}\", v); }}",
            prop.name, prop.name
        );
    }
}

/// Emit JS chainable method implementations using `_setSpec` for each property.
fn emit_property_methods_js_spec(out: &mut String, props: &[PropertyInfo], skip: &[&str]) {
    for prop in props {
        if skip.contains(&prop.name.as_str()) {
            continue;
        }
        let _ = writeln!(
            out,
            "  {}(v) {{ return this._setSpec(\"{}\", v); }}",
            prop.name, prop.name
        );
    }
}

/// Emit a builder class for a kind with GVK, including typed .spec() override
/// and per-spec-property chainable methods if the spec schema is available.
pub fn emit_builder_class(
    schema: &SchemaInfo,
    api_version: &str,
    all_schemas: &[&SchemaInfo],
) -> String {
    let mut out = String::new();

    if let Some(desc) = &schema.description {
        let _ = writeln!(out, "/** {desc} */");
    }
    let _ = writeln!(
        out,
        "export interface {} extends _ResourceBuilder {{",
        schema.ts_name
    );

    // Typed metadata() override — accepts MetadataChain or SpecFragment from k8s/meta/v1.
    let _ = writeln!(
        out,
        "  /** Set metadata using a chain starter (name(), namespace(), label() from \"k8s/meta/v1\"). */"
    );
    let _ = writeln!(
        out,
        "  metadata(chain: import(\"k8s/_chains\").MetadataChain): this;"
    );

    // Emit typed .spec() override if there is a spec property with a Ref type
    if let Some(spec_type) = find_spec_type(schema) {
        let _ = writeln!(out, "  /** Set the resource specification. */");
        let _ = writeln!(out, "  spec(value: {spec_type}): this;");

        // Emit per-spec-property methods
        if let Some(spec_schema) = find_spec_schema(schema, all_schemas) {
            emit_property_methods_dts(&mut out, &spec_schema.properties, RESOURCE_SPEC_SKIP);

            // Convenience shortcuts for workload resources with PodTemplateSpec
            if has_pod_template(spec_schema) {
                let _ = writeln!(
                    out,
                    "  /** Set pod containers. Accepts ContainerChain items (use name(), image() from \"k8s/core/v1\"). */"
                );
                let _ = writeln!(
                    out,
                    "  containers(items: import(\"k8s/_chains\").ContainerChain[]): this;"
                );
                let _ = writeln!(
                    out,
                    "  /** Set pod init containers. Accepts ContainerChain items. */"
                );
                let _ = writeln!(
                    out,
                    "  initContainers(items: import(\"k8s/_chains\").ContainerChain[]): this;"
                );
            }
        }
    }

    // Emit per-top-level-property methods (data, rules, subjects, etc.)
    emit_property_methods_dts(&mut out, &schema.properties, RESOURCE_TOP_LEVEL_SKIP);

    let _ = writeln!(out, "}}");

    // Factory function declaration
    let factory = to_factory_name(&schema.ts_name);
    let _ = writeln!(out, "export function {factory}(): {};", schema.ts_name);

    // Suppress unused warning — api_version is available for future use
    let _ = api_version;

    out
}

/// Emit a `_SchemaBuilder` subclass declaration (.d.ts).
/// Uses `interface` (not `class`) so the factory function with the same PascalCase
/// name can coexist via TypeScript declaration merging.
fn emit_schema_builder_class(schema: &SchemaInfo) -> String {
    let mut out = String::new();

    if let Some(desc) = &schema.description {
        let _ = writeln!(out, "/** {desc} */");
    }
    let _ = writeln!(
        out,
        "export interface {} extends _SchemaBuilder {{",
        schema.ts_name
    );

    emit_property_methods_dts(&mut out, &schema.properties, &[]);

    let _ = writeln!(out, "}}");

    // Factory function declaration (PascalCase, same as class name)
    let factory = to_factory_name(&schema.ts_name);
    let _ = writeln!(out, "export function {factory}(): {};", schema.ts_name);

    out
}

/// Emit a `_SchemaBuilder` subclass implementation (.js).
/// The class is prefixed with `_` (not exported) to avoid name collision with
/// the PascalCase factory function.
fn emit_schema_builder_js(schema: &SchemaInfo) -> String {
    let mut out = String::new();

    let _ = writeln!(out, "class _{} extends _SchemaBuilder {{", schema.ts_name);

    emit_property_methods_js_set(&mut out, &schema.properties, &[]);

    let _ = writeln!(out, "}}");

    // Factory function (PascalCase, same as class name)
    let factory = to_factory_name(&schema.ts_name);
    let _ = writeln!(
        out,
        "export function {factory}() {{ return new _{}(); }}\n",
        schema.ts_name
    );
    out
}

/// Collect all `Ref` type names used by a set of schemas.
fn collect_refs(schemas: &[&SchemaInfo]) -> HashSet<String> {
    let mut refs = HashSet::new();
    for schema in schemas {
        for prop in &schema.properties {
            collect_type_refs(&prop.ts_type, &mut refs);
        }
    }
    refs
}

fn collect_type_refs(ty: &TsType, refs: &mut HashSet<String>) {
    match ty {
        TsType::Ref(name) => {
            refs.insert(name.clone());
        }
        TsType::Array(inner) | TsType::Map(inner) => collect_type_refs(inner, refs),
        _ => {}
    }
}

/// Emit `_common.d.ts` from common schemas.
pub fn emit_common(schemas: &[&SchemaInfo]) -> String {
    let mut out = String::new();

    let _ = writeln!(out, "// Auto-generated by husako. Do not edit.\n");

    let has_schema_builders = schemas.iter().any(|s| should_generate_builder(s));

    if has_schema_builders {
        let _ = writeln!(out, "import {{ _SchemaBuilder }} from \"husako/_base\";\n");
    }

    // Collect refs to determine which types from _common itself are referenced
    let refs = collect_refs(schemas);
    let local_names: HashSet<&str> = schemas.iter().map(|s| s.ts_name.as_str()).collect();

    // Find refs to types that aren't in common — these would be from other modules.
    // For common, we don't import anything external.
    let external_refs: Vec<&String> = refs
        .iter()
        .filter(|r| !local_names.contains(r.as_str()))
        .collect();
    if !external_refs.is_empty() {
        // These are unknown refs; emit them as `any` type alias.
        for r in &external_refs {
            let _ = writeln!(out, "type {r} = any;");
        }
        let _ = writeln!(out);
    }

    for schema in schemas {
        // Skip raw interface for schemas that also get a builder class — emitting both
        // causes TypeScript declaration merging which makes methods non-callable.
        if should_generate_builder(schema) {
            continue;
        }
        let _ = write!(out, "{}", emit_interface(schema));
        let _ = writeln!(out);
    }

    // Emit schema builder classes for complex common schemas
    for schema in schemas {
        if should_generate_builder(schema) {
            let _ = write!(out, "{}", emit_schema_builder_class(schema));
            let _ = writeln!(out);
        }
    }

    out
}

/// Emit `_common.js` runtime module for common schemas with builders.
pub fn emit_common_js(schemas: &[&SchemaInfo]) -> String {
    let mut out = String::new();

    let _ = writeln!(out, "// Auto-generated by husako. Do not edit.\n");
    let _ = writeln!(out, "import {{ _SchemaBuilder }} from \"husako/_base\";\n");

    for schema in schemas {
        if should_generate_builder(schema) {
            let _ = write!(out, "{}", emit_schema_builder_js(schema));
        }
    }

    out
}

/// Emit a per-group-version `.d.ts` file.
///
/// All schemas with GVK get builder classes (no registered_kinds filter).
/// Non-GVK schemas with complex properties get `_SchemaBuilder` subclasses.
/// `common_names` — set of type names available in `_common.d.ts`.
pub fn emit_group_version(schemas: &[&SchemaInfo], common_names: &HashSet<String>) -> String {
    let mut out = String::new();

    let _ = writeln!(out, "// Auto-generated by husako. Do not edit.\n");

    let has_resource_builders = schemas.iter().any(|s| s.gvk.is_some());
    let has_schema_builders = schemas.iter().any(|s| should_generate_builder(s));

    // Build import for base classes
    let mut base_imports = Vec::new();
    if has_resource_builders {
        base_imports.push("_ResourceBuilder");
    }
    if has_schema_builders {
        base_imports.push("_SchemaBuilder");
    }
    if !base_imports.is_empty() {
        let _ = writeln!(
            out,
            "import {{ {} }} from \"husako/_base\";\n",
            base_imports.join(", ")
        );
    }

    // Collect referenced types from non-GVK schemas only.
    // GVK schemas' raw interfaces are not emitted (see below), so their top-level
    // property types (e.g. Deployment.metadata: ObjectMeta) would create stale imports.
    // Non-GVK spec schemas (e.g. DeploymentSpec) still reference common types in their
    // builder method signatures, so those imports are preserved correctly.
    let non_gvk: Vec<&SchemaInfo> = schemas
        .iter()
        .filter(|s| s.gvk.is_none())
        .copied()
        .collect();
    let refs = collect_refs(&non_gvk);
    let local_names: HashSet<&str> = schemas.iter().map(|s| s.ts_name.as_str()).collect();

    // Import common types that are referenced but not defined locally
    let common_imports: Vec<&String> = refs
        .iter()
        .filter(|r| !local_names.contains(r.as_str()) && common_names.contains(r.as_str()))
        .collect();

    if !common_imports.is_empty() {
        let mut sorted: Vec<&&String> = common_imports.iter().collect();
        sorted.sort();
        let names: Vec<&str> = sorted.iter().map(|s| s.as_str()).collect();
        let _ = writeln!(
            out,
            "import {{ {} }} from \"k8s/_common\";\n",
            names.join(", ")
        );
    }

    // Emit interfaces — skip schemas that also get a builder class to avoid
    // TypeScript declaration merging which makes methods non-callable.
    for schema in schemas {
        if schema.gvk.is_some() || should_generate_builder(schema) {
            continue;
        }
        let _ = write!(out, "{}", emit_interface(schema));
        let _ = writeln!(out);
    }

    // Emit builder classes for all schemas with GVK
    for schema in schemas {
        if let Some(gvk) = &schema.gvk {
            let api_version = if gvk.group.is_empty() {
                gvk.version.clone()
            } else {
                format!("{}/{}", gvk.group, gvk.version)
            };
            let _ = write!(out, "{}", emit_builder_class(schema, &api_version, schemas));
            let _ = writeln!(out);
        }
    }

    // Emit schema builder classes for non-GVK schemas with complex properties
    for schema in schemas {
        if should_generate_builder(schema) {
            let _ = write!(out, "{}", emit_schema_builder_class(schema));
            let _ = writeln!(out);
        }
    }

    out
}

/// Emit a per-group-version `.js` runtime module.
///
/// Each schema with GVK gets a builder class extending `_ResourceBuilder`
/// with per-spec-property chainable methods.
/// Non-GVK schemas with complex properties get `_SchemaBuilder` subclasses.
pub fn emit_group_version_js(schemas: &[&SchemaInfo]) -> String {
    let mut out = String::new();

    let _ = writeln!(out, "// Auto-generated by husako. Do not edit.\n");

    let has_resource_builders = schemas.iter().any(|s| s.gvk.is_some());
    let has_schema_builders = schemas.iter().any(|s| should_generate_builder(s));

    // Build import for base classes
    let mut base_imports = Vec::new();
    if has_resource_builders {
        base_imports.push("_ResourceBuilder");
    }
    if has_schema_builders {
        base_imports.push("_SchemaBuilder");
    }
    if !base_imports.is_empty() {
        let _ = writeln!(
            out,
            "import {{ {} }} from \"husako/_base\";\n",
            base_imports.join(", ")
        );
    }

    // Emit resource builder classes with per-spec-property methods
    for schema in schemas {
        if let Some(gvk) = &schema.gvk {
            let api_version = if gvk.group.is_empty() {
                gvk.version.clone()
            } else {
                format!("{}/{}", gvk.group, gvk.version)
            };
            let _ = writeln!(out, "class _{} extends _ResourceBuilder {{", schema.ts_name);
            let _ = writeln!(
                out,
                "  constructor() {{ super(\"{api_version}\", \"{}\"); }}",
                schema.ts_name
            );

            // Emit per-spec-property methods
            if let Some(spec_schema) = find_spec_schema(schema, schemas) {
                emit_property_methods_js_spec(
                    &mut out,
                    &spec_schema.properties,
                    RESOURCE_SPEC_SKIP,
                );

                // Convenience shortcuts for workload resources
                if has_pod_template(spec_schema) {
                    let _ = writeln!(out, "  containers(items) {{");
                    let _ = writeln!(out, "    const resolved = items.map(function(item) {{");
                    let _ = writeln!(
                        out,
                        "      if (item && item._husakoTag === \"SpecFragment\") return item._toContainer();"
                    );
                    let _ = writeln!(
                        out,
                        "      throw new Error(\"containers() items must be ContainerChain — use name(), image() from \\\"k8s/core/v1\\\".\");"
                    );
                    let _ = writeln!(out, "    }});");
                    let _ = writeln!(
                        out,
                        "    return this._setDeep(\"template.spec.containers\", resolved);"
                    );
                    let _ = writeln!(out, "  }}");
                    let _ = writeln!(out, "  initContainers(items) {{");
                    let _ = writeln!(out, "    const resolved = items.map(function(item) {{");
                    let _ = writeln!(
                        out,
                        "      if (item && item._husakoTag === \"SpecFragment\") return item._toContainer();"
                    );
                    let _ = writeln!(
                        out,
                        "      throw new Error(\"initContainers() items must be ContainerChain — use name(), image() from \\\"k8s/core/v1\\\".\");"
                    );
                    let _ = writeln!(out, "    }});");
                    let _ = writeln!(
                        out,
                        "    return this._setDeep(\"template.spec.initContainers\", resolved);"
                    );
                    let _ = writeln!(out, "  }}");
                }
            }

            // Emit per-top-level-property methods
            emit_property_methods_js_top(&mut out, &schema.properties, RESOURCE_TOP_LEVEL_SKIP);

            let _ = writeln!(out, "}}");

            // Factory function (only export)
            let factory = to_factory_name(&schema.ts_name);
            let _ = writeln!(
                out,
                "export function {factory}() {{ return new _{}(); }}\n",
                schema.ts_name
            );
        }
    }

    // Emit schema builder classes for non-GVK schemas with complex properties
    for schema in schemas {
        if should_generate_builder(schema) {
            let _ = write!(out, "{}", emit_schema_builder_js(schema));
        }
    }

    out
}

/// Emit `k8s/_chains.d.ts` — chain interface definitions for the starter API.
///
/// Defines: MetadataChain, ContainerChain, ResourceRequirementsChain, SpecFragment.
/// Generated by `husako gen` and referenced by per-group-version .d.ts files.
pub fn emit_chains_dts() -> String {
    let mut out = String::new();
    let _ = writeln!(out, "// Auto-generated by husako. Do not edit.\n");
    let _ = writeln!(
        out,
        "/** Chain interface for metadata configuration. Returned by namespace(), label(), annotation(). */"
    );
    let _ = writeln!(out, "export interface MetadataChain {{");
    let _ = writeln!(out, "  name(v: string): MetadataChain;");
    let _ = writeln!(out, "  namespace(v: string): MetadataChain;");
    let _ = writeln!(out, "  label(k: string, v: string): MetadataChain;");
    let _ = writeln!(out, "  annotation(k: string, v: string): MetadataChain;");
    let _ = writeln!(out, "}}\n");

    let _ = writeln!(
        out,
        "/** Chain interface for container configuration. Returned by image(), imagePullPolicy(). */"
    );
    let _ = writeln!(out, "export interface ContainerChain {{");
    let _ = writeln!(out, "  name(v: string): ContainerChain;");
    let _ = writeln!(out, "  image(v: string): ContainerChain;");
    let _ = writeln!(
        out,
        "  imagePullPolicy(v: \"Always\" | \"IfNotPresent\" | \"Never\"): ContainerChain;"
    );
    let _ = writeln!(
        out,
        "  resources(r: ResourceRequirementsChain): ContainerChain;"
    );
    let _ = writeln!(out, "  command(v: string[]): ContainerChain;");
    let _ = writeln!(out, "  args(v: string[]): ContainerChain;");
    let _ = writeln!(out, "}}\n");

    let _ = writeln!(
        out,
        "/** Bare resource list chain (cpu, memory). Returned by cpu() and memory(). Pass to requests(). */"
    );
    let _ = writeln!(out, "export interface ResourceListChain {{");
    let _ = writeln!(out, "  cpu(v: string | number): ResourceListChain;");
    let _ = writeln!(out, "  memory(v: string | number): ResourceListChain;");
    let _ = writeln!(out, "}}\n");

    let _ = writeln!(
        out,
        "/** Full resource requirements chain. Returned by requests(). Accepted by .resources(). */"
    );
    let _ = writeln!(out, "export interface ResourceRequirementsChain {{");
    let _ = writeln!(
        out,
        "  limits(chain: ResourceListChain): ResourceRequirementsChain;"
    );
    let _ = writeln!(out, "}}\n");

    let _ = writeln!(
        out,
        "/** SpecFragment: returned by name() — compatible with both MetadataChain and ContainerChain. */"
    );
    let _ = writeln!(
        out,
        "export interface SpecFragment extends MetadataChain, ContainerChain {{"
    );
    let _ = writeln!(out, "  name(v: string): SpecFragment;");
    let _ = writeln!(out, "  namespace(v: string): SpecFragment;");
    let _ = writeln!(out, "  label(k: string, v: string): SpecFragment;");
    let _ = writeln!(out, "  annotation(k: string, v: string): SpecFragment;");
    let _ = writeln!(out, "  image(v: string): SpecFragment;");
    let _ = writeln!(
        out,
        "  imagePullPolicy(v: \"Always\" | \"IfNotPresent\" | \"Never\"): SpecFragment;"
    );
    let _ = writeln!(
        out,
        "  resources(r: ResourceRequirementsChain): SpecFragment;"
    );
    let _ = writeln!(out, "  command(v: string[]): SpecFragment;");
    let _ = writeln!(out, "  args(v: string[]): SpecFragment;");
    let _ = writeln!(out, "}}");
    out
}

/// Emit `k8s/meta/v1.d.ts` chain starter declarations for ObjectMeta fields.
pub fn emit_meta_v1_starters_dts() -> String {
    let mut out = String::new();
    let _ = writeln!(out, "// Auto-generated by husako. Do not edit.\n");
    let _ = writeln!(
        out,
        "import type {{ MetadataChain, SpecFragment }} from \"k8s/_chains\";\n"
    );
    let _ = writeln!(
        out,
        "/** Create a SpecFragment with the given name. Compatible with both .metadata() and .containers(). */"
    );
    let _ = writeln!(out, "export function name(v: string): SpecFragment;");
    let _ = writeln!(
        out,
        "/** Create a MetadataChain with the given namespace. */"
    );
    let _ = writeln!(out, "export function namespace(v: string): MetadataChain;");
    let _ = writeln!(out, "/** Create a MetadataChain with a single label. */");
    let _ = writeln!(
        out,
        "export function label(k: string, v: string): MetadataChain;"
    );
    let _ = writeln!(
        out,
        "/** Create a MetadataChain with a single annotation. */"
    );
    let _ = writeln!(
        out,
        "export function annotation(k: string, v: string): MetadataChain;"
    );
    out
}

/// Emit `k8s/meta/v1.js` chain starter implementations for ObjectMeta fields.
pub fn emit_meta_v1_starters_js() -> String {
    let mut out = String::new();
    let _ = writeln!(out, "// Auto-generated by husako. Do not edit.\n");
    let _ = writeln!(
        out,
        "import {{ _createSpecFragment }} from \"husako/_base\";\n"
    );
    let _ = writeln!(
        out,
        "export function name(v) {{ return _createSpecFragment({{ _name: v }}); }}"
    );
    let _ = writeln!(
        out,
        "export function namespace(v) {{ return _createSpecFragment({{ _namespace: v }}); }}"
    );
    let _ = writeln!(
        out,
        "export function label(k, v) {{ const l = {{}}; l[k] = v; return _createSpecFragment({{ _labels: l }}); }}"
    );
    let _ = writeln!(
        out,
        "export function annotation(k, v) {{ const a = {{}}; a[k] = v; return _createSpecFragment({{ _annotations: a }}); }}"
    );
    out
}

/// Emit chain starter declarations to prepend to `k8s/core/v1.d.ts`.
pub fn emit_core_v1_starters_dts() -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "import type {{ ContainerChain, SpecFragment, ResourceListChain, ResourceRequirementsChain }} from \"k8s/_chains\";"
    );
    let _ = writeln!(
        out,
        "/** Create a SpecFragment with the given name. Compatible with both .metadata() and .containers(). */"
    );
    let _ = writeln!(out, "export function name(v: string): SpecFragment;");
    let _ = writeln!(out, "/** Create a ContainerChain with the given image. */");
    let _ = writeln!(out, "export function image(v: string): ContainerChain;");
    let _ = writeln!(
        out,
        "/** Create a ContainerChain with the given imagePullPolicy. */"
    );
    let _ = writeln!(
        out,
        "export function imagePullPolicy(v: \"Always\" | \"IfNotPresent\" | \"Never\"): ContainerChain;"
    );
    let _ = writeln!(
        out,
        "/** Create a bare ResourceListChain with the given cpu quantity. Pass to requests(). */"
    );
    let _ = writeln!(
        out,
        "export function cpu(v: string | number): ResourceListChain;"
    );
    let _ = writeln!(
        out,
        "/** Create a bare ResourceListChain with the given memory quantity. Pass to requests(). */"
    );
    let _ = writeln!(
        out,
        "export function memory(v: string | number): ResourceListChain;"
    );
    let _ = writeln!(
        out,
        "/** Wrap a ResourceListChain into a ResourceRequirementsChain for use with .resources(). */"
    );
    let _ = writeln!(
        out,
        "export function requests(chain: ResourceListChain): ResourceRequirementsChain;"
    );
    out
}

/// Emit chain starter implementations to prepend to `k8s/core/v1.js`.
pub fn emit_core_v1_starters_js() -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "import {{ _createSpecFragment, _createResourceChain, _createResourceRequirementsChain }} from \"husako/_base\";"
    );
    let _ = writeln!(
        out,
        "export function name(v) {{ return _createSpecFragment({{ _name: v }}); }}"
    );
    let _ = writeln!(
        out,
        "export function image(v) {{ return _createSpecFragment({{ _image: v }}); }}"
    );
    let _ = writeln!(
        out,
        "export function imagePullPolicy(v) {{ return _createSpecFragment({{ _imagePullPolicy: v }}); }}"
    );
    let _ = writeln!(
        out,
        "export function cpu(v) {{ return _createResourceChain({{}}).cpu(v); }}"
    );
    let _ = writeln!(
        out,
        "export function memory(v) {{ return _createResourceChain({{}}).memory(v); }}"
    );
    let _ = writeln!(
        out,
        "export function requests(chain) {{ const list = chain && typeof chain._toJSON === \"function\" ? chain._toJSON() : chain; return _createResourceRequirementsChain(list); }}"
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{GroupVersionKind, SchemaLocation};

    fn make_schema(name: &str, props: Vec<PropertyInfo>) -> SchemaInfo {
        SchemaInfo {
            full_name: format!("io.k8s.api.apps.v1.{name}"),
            ts_name: name.to_string(),
            location: SchemaLocation::GroupVersion {
                group: "apps".to_string(),
                version: "v1".to_string(),
            },
            properties: props,
            gvk: None,
            description: None,
        }
    }

    #[test]
    fn format_basic_types() {
        assert_eq!(format_ts_type(&TsType::String), "string");
        assert_eq!(format_ts_type(&TsType::Number), "number");
        assert_eq!(format_ts_type(&TsType::Boolean), "boolean");
        assert_eq!(format_ts_type(&TsType::IntOrString), "number | string");
        assert_eq!(format_ts_type(&TsType::Any), "any");
    }

    #[test]
    fn format_composite_types() {
        assert_eq!(
            format_ts_type(&TsType::Array(Box::new(TsType::String))),
            "string[]"
        );
        assert_eq!(
            format_ts_type(&TsType::Map(Box::new(TsType::Number))),
            "Record<string, number>"
        );
        assert_eq!(
            format_ts_type(&TsType::Ref("ObjectMeta".to_string())),
            "ObjectMeta"
        );
    }

    #[test]
    fn emit_interface_snapshot() {
        let schema = make_schema(
            "DeploymentSpec",
            vec![
                PropertyInfo {
                    name: "replicas".to_string(),
                    ts_type: TsType::Number,
                    required: false,
                    description: Some("Number of desired pods.".to_string()),
                },
                PropertyInfo {
                    name: "selector".to_string(),
                    ts_type: TsType::Ref("LabelSelector".to_string()),
                    required: true,
                    description: None,
                },
            ],
        );

        let output = emit_interface(&schema);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn emit_builder_class_snapshot() {
        let spec = make_schema(
            "DeploymentSpec",
            vec![
                PropertyInfo {
                    name: "replicas".to_string(),
                    ts_type: TsType::Number,
                    required: false,
                    description: Some("Number of desired pods.".to_string()),
                },
                PropertyInfo {
                    name: "selector".to_string(),
                    ts_type: TsType::Ref("LabelSelector".to_string()),
                    required: true,
                    description: None,
                },
                PropertyInfo {
                    name: "template".to_string(),
                    ts_type: TsType::Ref("PodTemplateSpec".to_string()),
                    required: false,
                    description: None,
                },
            ],
        );

        let mut schema = make_schema(
            "Deployment",
            vec![PropertyInfo {
                name: "spec".to_string(),
                ts_type: TsType::Ref("DeploymentSpec".to_string()),
                required: false,
                description: None,
            }],
        );
        schema.gvk = Some(GroupVersionKind {
            group: "apps".to_string(),
            version: "v1".to_string(),
            kind: "Deployment".to_string(),
        });
        schema.description =
            Some("Deployment enables declarative updates for Pods and ReplicaSets.".to_string());

        let all_schemas: Vec<&SchemaInfo> = vec![&schema, &spec];
        let output = emit_builder_class(&schema, "apps/v1", &all_schemas);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn emit_builder_class_without_spec() {
        let mut schema = make_schema("Namespace", vec![]);
        schema.gvk = Some(GroupVersionKind {
            group: String::new(),
            version: "v1".to_string(),
            kind: "Namespace".to_string(),
        });
        schema.description = Some("Namespace provides a scope for Names.".to_string());

        let all_schemas: Vec<&SchemaInfo> = vec![&schema];
        let output = emit_builder_class(&schema, "v1", &all_schemas);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn emit_common_snapshot() {
        let s1 = SchemaInfo {
            full_name: "io.k8s.apimachinery.pkg.apis.meta.v1.ObjectMeta".to_string(),
            ts_name: "ObjectMeta".to_string(),
            location: SchemaLocation::Common,
            properties: vec![
                PropertyInfo {
                    name: "name".to_string(),
                    ts_type: TsType::String,
                    required: false,
                    description: None,
                },
                PropertyInfo {
                    name: "namespace".to_string(),
                    ts_type: TsType::String,
                    required: false,
                    description: None,
                },
                PropertyInfo {
                    name: "labels".to_string(),
                    ts_type: TsType::Map(Box::new(TsType::String)),
                    required: false,
                    description: None,
                },
            ],
            gvk: None,
            description: Some(
                "ObjectMeta is metadata attached to every Kubernetes object.".to_string(),
            ),
        };

        let s2 = SchemaInfo {
            full_name: "io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelector".to_string(),
            ts_name: "LabelSelector".to_string(),
            location: SchemaLocation::Common,
            properties: vec![PropertyInfo {
                name: "matchLabels".to_string(),
                ts_type: TsType::Map(Box::new(TsType::String)),
                required: false,
                description: None,
            }],
            gvk: None,
            description: None,
        };

        let schemas: Vec<&SchemaInfo> = vec![&s1, &s2];
        let output = emit_common(&schemas);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn emit_group_version_snapshot() {
        let deployment = SchemaInfo {
            full_name: "io.k8s.api.apps.v1.Deployment".to_string(),
            ts_name: "Deployment".to_string(),
            location: SchemaLocation::GroupVersion {
                group: "apps".to_string(),
                version: "v1".to_string(),
            },
            properties: vec![
                PropertyInfo {
                    name: "metadata".to_string(),
                    ts_type: TsType::Ref("ObjectMeta".to_string()),
                    required: false,
                    description: None,
                },
                PropertyInfo {
                    name: "spec".to_string(),
                    ts_type: TsType::Ref("DeploymentSpec".to_string()),
                    required: false,
                    description: None,
                },
            ],
            gvk: Some(GroupVersionKind {
                group: "apps".to_string(),
                version: "v1".to_string(),
                kind: "Deployment".to_string(),
            }),
            description: None,
        };

        let spec = SchemaInfo {
            full_name: "io.k8s.api.apps.v1.DeploymentSpec".to_string(),
            ts_name: "DeploymentSpec".to_string(),
            location: SchemaLocation::GroupVersion {
                group: "apps".to_string(),
                version: "v1".to_string(),
            },
            properties: vec![PropertyInfo {
                name: "replicas".to_string(),
                ts_type: TsType::Number,
                required: false,
                description: None,
            }],
            gvk: None,
            description: None,
        };

        let schemas: Vec<&SchemaInfo> = vec![&deployment, &spec];
        let common = HashSet::from(["ObjectMeta".to_string()]);

        let output = emit_group_version(&schemas, &common);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn emit_group_version_js_snapshot() {
        let deployment = SchemaInfo {
            full_name: "io.k8s.api.apps.v1.Deployment".to_string(),
            ts_name: "Deployment".to_string(),
            location: SchemaLocation::GroupVersion {
                group: "apps".to_string(),
                version: "v1".to_string(),
            },
            properties: vec![PropertyInfo {
                name: "spec".to_string(),
                ts_type: TsType::Ref("DeploymentSpec".to_string()),
                required: false,
                description: None,
            }],
            gvk: Some(GroupVersionKind {
                group: "apps".to_string(),
                version: "v1".to_string(),
                kind: "Deployment".to_string(),
            }),
            description: None,
        };

        let stateful_set = SchemaInfo {
            full_name: "io.k8s.api.apps.v1.StatefulSet".to_string(),
            ts_name: "StatefulSet".to_string(),
            location: SchemaLocation::GroupVersion {
                group: "apps".to_string(),
                version: "v1".to_string(),
            },
            properties: vec![PropertyInfo {
                name: "spec".to_string(),
                ts_type: TsType::Ref("StatefulSetSpec".to_string()),
                required: false,
                description: None,
            }],
            gvk: Some(GroupVersionKind {
                group: "apps".to_string(),
                version: "v1".to_string(),
                kind: "StatefulSet".to_string(),
            }),
            description: None,
        };

        let spec = SchemaInfo {
            full_name: "io.k8s.api.apps.v1.DeploymentSpec".to_string(),
            ts_name: "DeploymentSpec".to_string(),
            location: SchemaLocation::GroupVersion {
                group: "apps".to_string(),
                version: "v1".to_string(),
            },
            properties: vec![
                PropertyInfo {
                    name: "replicas".to_string(),
                    ts_type: TsType::Number,
                    required: false,
                    description: None,
                },
                PropertyInfo {
                    name: "template".to_string(),
                    ts_type: TsType::Ref("PodTemplateSpec".to_string()),
                    required: false,
                    description: None,
                },
            ],
            gvk: None,
            description: None,
        };

        let schemas: Vec<&SchemaInfo> = vec![&deployment, &stateful_set, &spec];
        let output = emit_group_version_js(&schemas);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn emit_schema_builder_snapshot() {
        let container = SchemaInfo {
            full_name: "io.k8s.api.core.v1.Container".to_string(),
            ts_name: "Container".to_string(),
            location: SchemaLocation::GroupVersion {
                group: "core".to_string(),
                version: "v1".to_string(),
            },
            properties: vec![
                PropertyInfo {
                    name: "name".to_string(),
                    ts_type: TsType::String,
                    required: true,
                    description: None,
                },
                PropertyInfo {
                    name: "image".to_string(),
                    ts_type: TsType::String,
                    required: false,
                    description: None,
                },
                PropertyInfo {
                    name: "resources".to_string(),
                    ts_type: TsType::Ref("ResourceRequirements".to_string()),
                    required: false,
                    description: None,
                },
            ],
            gvk: None,
            description: Some("A single container within a pod.".to_string()),
        };

        assert!(should_generate_builder(&container));
        let dts = emit_schema_builder_class(&container);
        let js = emit_schema_builder_js(&container);
        insta::assert_snapshot!("schema_builder_dts", dts);
        insta::assert_snapshot!("schema_builder_js", js);
    }

    #[test]
    fn should_not_generate_builder_for_simple_schema() {
        let label_selector = SchemaInfo {
            full_name: "io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelector".to_string(),
            ts_name: "LabelSelector".to_string(),
            location: SchemaLocation::Common,
            properties: vec![PropertyInfo {
                name: "matchLabels".to_string(),
                ts_type: TsType::Map(Box::new(TsType::String)),
                required: false,
                description: None,
            }],
            gvk: None,
            description: None,
        };
        assert!(!should_generate_builder(&label_selector));
    }

    #[test]
    fn should_not_generate_builder_for_gvk_schema() {
        let mut deployment = make_schema(
            "Deployment",
            vec![PropertyInfo {
                name: "spec".to_string(),
                ts_type: TsType::Ref("DeploymentSpec".to_string()),
                required: false,
                description: None,
            }],
        );
        deployment.gvk = Some(GroupVersionKind {
            group: "apps".to_string(),
            version: "v1".to_string(),
            kind: "Deployment".to_string(),
        });
        // GVK schemas get _ResourceBuilder, not _SchemaBuilder
        assert!(!should_generate_builder(&deployment));
    }

    #[test]
    fn emit_builder_class_with_top_level_fields() {
        let mut schema = make_schema(
            "ConfigMap",
            vec![
                PropertyInfo {
                    name: "apiVersion".to_string(),
                    ts_type: TsType::String,
                    required: false,
                    description: None,
                },
                PropertyInfo {
                    name: "kind".to_string(),
                    ts_type: TsType::String,
                    required: false,
                    description: None,
                },
                PropertyInfo {
                    name: "metadata".to_string(),
                    ts_type: TsType::Ref("ObjectMeta".to_string()),
                    required: false,
                    description: None,
                },
                PropertyInfo {
                    name: "data".to_string(),
                    ts_type: TsType::Map(Box::new(TsType::String)),
                    required: false,
                    description: Some("Data contains the configuration data.".to_string()),
                },
                PropertyInfo {
                    name: "immutable".to_string(),
                    ts_type: TsType::Boolean,
                    required: false,
                    description: None,
                },
            ],
        );
        schema.gvk = Some(GroupVersionKind {
            group: String::new(),
            version: "v1".to_string(),
            kind: "ConfigMap".to_string(),
        });

        let all_schemas: Vec<&SchemaInfo> = vec![&schema];
        let output = emit_builder_class(&schema, "v1", &all_schemas);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn emit_group_version_js_with_top_level_fields() {
        let mut schema = make_schema(
            "ConfigMap",
            vec![
                PropertyInfo {
                    name: "apiVersion".to_string(),
                    ts_type: TsType::String,
                    required: false,
                    description: None,
                },
                PropertyInfo {
                    name: "kind".to_string(),
                    ts_type: TsType::String,
                    required: false,
                    description: None,
                },
                PropertyInfo {
                    name: "metadata".to_string(),
                    ts_type: TsType::Ref("ObjectMeta".to_string()),
                    required: false,
                    description: None,
                },
                PropertyInfo {
                    name: "data".to_string(),
                    ts_type: TsType::Map(Box::new(TsType::String)),
                    required: false,
                    description: None,
                },
                PropertyInfo {
                    name: "immutable".to_string(),
                    ts_type: TsType::Boolean,
                    required: false,
                    description: None,
                },
            ],
        );
        schema.gvk = Some(GroupVersionKind {
            group: String::new(),
            version: "v1".to_string(),
            kind: "ConfigMap".to_string(),
        });

        let schemas: Vec<&SchemaInfo> = vec![&schema];
        let output = emit_group_version_js(&schemas);
        insta::assert_snapshot!(output);
    }

    /// GVK schemas must NOT emit a raw data interface — only the builder class.
    /// Emitting both causes TypeScript declaration merging which breaks method calls.
    #[test]
    fn emit_group_version_no_raw_interface_for_gvk() {
        let mut deployment = make_schema(
            "Deployment",
            vec![PropertyInfo {
                name: "spec".to_string(),
                ts_type: TsType::Ref("DeploymentSpec".to_string()),
                required: false,
                description: None,
            }],
        );
        deployment.gvk = Some(GroupVersionKind {
            group: "apps".to_string(),
            version: "v1".to_string(),
            kind: "Deployment".to_string(),
        });

        let schemas: Vec<&SchemaInfo> = vec![&deployment];
        let common = HashSet::new();
        let output = emit_group_version(&schemas, &common);

        // Must contain the builder interface
        assert!(
            output.contains("extends _ResourceBuilder"),
            "expected _ResourceBuilder in output:\n{output}"
        );
        // Must NOT contain a plain data interface (no `export interface Deployment {` without `extends`)
        assert!(
            !output.contains("export interface Deployment {"),
            "raw interface must not be emitted for GVK schema:\n{output}"
        );
    }

    /// Schemas with `resources: ResourceRequirements` must emit a union type that also
    /// accepts `ResourceRequirementsChain` from k8s/_chains.
    #[test]
    fn emit_schema_builder_resources_type() {
        let container = SchemaInfo {
            full_name: "io.k8s.api.core.v1.Container".to_string(),
            ts_name: "Container".to_string(),
            location: SchemaLocation::GroupVersion {
                group: "core".to_string(),
                version: "v1".to_string(),
            },
            properties: vec![
                PropertyInfo {
                    name: "name".to_string(),
                    ts_type: TsType::String,
                    required: true,
                    description: None,
                },
                PropertyInfo {
                    name: "resources".to_string(),
                    ts_type: TsType::Ref("ResourceRequirements".to_string()),
                    required: false,
                    description: None,
                },
            ],
            gvk: None,
            description: None,
        };

        let dts = emit_schema_builder_class(&container);
        assert!(
            dts.contains(
                "resources(value: ResourceRequirements | import(\"k8s/_chains\").ResourceRequirementsChain): this;"
            ),
            "expected union type for resources:\n{dts}"
        );
    }

    /// A complex common schema (with Ref properties) must NOT emit a raw data interface
    /// — only the builder class. The raw interface causes the same declaration-merging
    /// problem as GVK schemas.
    #[test]
    fn emit_common_no_raw_interface_for_builder() {
        let label_selector = SchemaInfo {
            full_name: "io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelector".to_string(),
            ts_name: "LabelSelector".to_string(),
            location: SchemaLocation::Common,
            properties: vec![
                PropertyInfo {
                    name: "matchLabels".to_string(),
                    ts_type: TsType::Map(Box::new(TsType::String)),
                    required: false,
                    description: None,
                },
                PropertyInfo {
                    name: "matchExpressions".to_string(),
                    ts_type: TsType::Array(Box::new(TsType::Ref(
                        "LabelSelectorRequirement".to_string(),
                    ))),
                    required: false,
                    description: None,
                },
            ],
            gvk: None,
            description: None,
        };

        assert!(should_generate_builder(&label_selector));

        let schemas: Vec<&SchemaInfo> = vec![&label_selector];
        let output = emit_common(&schemas);

        // Must emit the builder class
        assert!(
            output.contains("extends _SchemaBuilder"),
            "expected _SchemaBuilder in output:\n{output}"
        );
        // Must NOT emit a plain data interface
        assert!(
            !output.contains("export interface LabelSelector {"),
            "raw interface must not be emitted for builder schema:\n{output}"
        );
    }
}
