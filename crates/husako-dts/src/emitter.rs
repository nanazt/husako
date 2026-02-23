use std::collections::HashSet;
use std::fmt::Write;

use crate::schema::{PropertyInfo, SchemaInfo, TsType, has_complex_property};

/// Properties to skip when generating spec property methods on resource builders.
const RESOURCE_SPEC_SKIP: &[&str] = &["status", "apiVersion", "kind", "metadata"];

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
        let _ = writeln!(
            out,
            "  {}(value: {}): this;",
            prop.name,
            format_ts_type(&prop.ts_type)
        );
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
                    "  /** Set pod containers (shortcut for template.spec.containers). */"
                );
                let _ = writeln!(out, "  containers(value: any[]): this;");
                let _ = writeln!(
                    out,
                    "  /** Set pod init containers (shortcut for template.spec.initContainers). */"
                );
                let _ = writeln!(out, "  initContainers(value: any[]): this;");
            }
        }
    }

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

    let _ = writeln!(
        out,
        "class _{} extends _SchemaBuilder {{",
        schema.ts_name
    );

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

    // Collect all referenced types
    let refs = collect_refs(schemas);
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

    // Emit interfaces
    for schema in schemas {
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
                    let _ = writeln!(
                        out,
                        "  containers(v) {{ return this._setDeep(\"template.spec.containers\", v); }}"
                    );
                    let _ = writeln!(
                        out,
                        "  initContainers(v) {{ return this._setDeep(\"template.spec.initContainers\", v); }}"
                    );
                }
            }

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
}
