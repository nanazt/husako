use std::collections::HashMap;
use std::fmt::Write;

use crate::DtsError;
use crate::emitter::format_ts_type;
use crate::schema::{PropertyInfo, SchemaInfo, TsType, has_complex_property};

/// Generate `.d.ts` and `.js` content from a JSON Schema for a Helm chart.
///
/// Returns `(dts_content, js_content)`.
pub fn generate_chart_types(
    chart_name: &str,
    schema: &serde_json::Value,
) -> Result<(String, String), DtsError> {
    let mut extracted = Vec::new();
    extract_schemas(schema, "Values", &mut extracted);

    if extracted.is_empty() {
        return Err(DtsError::Schema(format!(
            "chart '{chart_name}': no schemas extracted from values schema"
        )));
    }

    let dts = emit_chart_dts(&extracted);
    let js = emit_chart_js(&extracted);

    Ok((dts, js))
}

/// A named schema extracted from the JSON Schema tree.
struct ExtractedSchema {
    info: SchemaInfo,
}

/// Extract schemas from a JSON Schema, creating separate named types for
/// nested objects with their own properties (same pattern as CRD/OpenAPI).
fn extract_schemas(schema: &serde_json::Value, name: &str, out: &mut Vec<ExtractedSchema>) {
    let properties = match schema.get("properties").and_then(|p| p.as_object()) {
        Some(p) => p,
        None => return,
    };

    let required: Vec<String> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    // Check for $defs / definitions
    let defs = schema
        .get("$defs")
        .or_else(|| schema.get("definitions"))
        .and_then(|d| d.as_object());

    // Build a map of $ref targets to their definitions for inline resolution
    let mut def_map: HashMap<String, &serde_json::Value> = HashMap::new();
    if let Some(defs) = defs {
        for (key, val) in defs {
            def_map.insert(format!("#/$defs/{key}"), val);
            def_map.insert(format!("#/definitions/{key}"), val);
        }
    }

    let mut props = Vec::new();

    for (prop_name, prop_schema) in properties {
        let ts_type = resolve_json_schema_type(prop_name, prop_schema, &def_map, out);
        let description = prop_schema
            .get("description")
            .and_then(|d| d.as_str())
            .map(String::from);

        props.push(PropertyInfo {
            name: prop_name.clone(),
            ts_type,
            required: required.contains(prop_name),
            description,
        });
    }

    // Extract definitions as their own schemas too
    if let Some(defs) = defs {
        for (def_name, def_schema) in defs {
            let ts_name = to_pascal_case(def_name);
            // Only extract if it's an object with properties and not already extracted
            if def_schema.get("properties").is_some()
                && !out.iter().any(|e| e.info.ts_name == ts_name)
            {
                extract_schemas(def_schema, &ts_name, out);
            }
        }
    }

    let spec_name = format!("{name}Spec");

    out.push(ExtractedSchema {
        info: SchemaInfo {
            full_name: name.to_string(),
            ts_name: name.to_string(),
            location: crate::schema::SchemaLocation::Other,
            properties: props,
            gvk: None,
            description: schema
                .get("description")
                .and_then(|d| d.as_str())
                .map(String::from),
        },
    });

    // Also emit a Spec interface (ValuesSpec, ControllerSpec, etc.)
    // with plain types for documentation/type-annotation purposes
    let spec_props: Vec<PropertyInfo> = properties
        .iter()
        .map(|(prop_name, prop_schema)| {
            let ts_type = resolve_json_schema_type_for_spec(prop_name, prop_schema, &def_map, out);
            let description = prop_schema
                .get("description")
                .and_then(|d| d.as_str())
                .map(String::from);
            PropertyInfo {
                name: prop_name.clone(),
                ts_type,
                required: required.contains(prop_name),
                description,
            }
        })
        .collect();

    out.push(ExtractedSchema {
        info: SchemaInfo {
            full_name: spec_name.clone(),
            ts_name: spec_name,
            location: crate::schema::SchemaLocation::Other,
            properties: spec_props,
            gvk: None,
            description: None,
        },
    });
}

/// Resolve a JSON Schema property to a TsType, extracting nested objects
/// as separate named schemas.
fn resolve_json_schema_type(
    prop_name: &str,
    schema: &serde_json::Value,
    def_map: &HashMap<String, &serde_json::Value>,
    out: &mut Vec<ExtractedSchema>,
) -> TsType {
    // Handle $ref
    if let Some(ref_str) = schema.get("$ref").and_then(|r| r.as_str()) {
        if let Some(def) = def_map.get(ref_str) {
            let ref_name = ref_str.rsplit('/').next().unwrap_or(ref_str);
            let ts_name = to_pascal_case(ref_name);
            if def.get("properties").is_some() && !out.iter().any(|e| e.info.ts_name == ts_name) {
                extract_schemas(def, &ts_name, out);
            }
            if def.get("properties").is_some() {
                return TsType::Ref(ts_name);
            }
        }
        let ref_name = ref_str.rsplit('/').next().unwrap_or(ref_str);
        return TsType::Ref(to_pascal_case(ref_name));
    }

    // Handle enum (string literals)
    if let Some(enum_vals) = schema.get("enum").and_then(|e| e.as_array())
        && enum_vals.iter().all(|v| v.is_string())
    {
        return TsType::String; // string union simplified to string
    }

    // Handle oneOf/anyOf
    if schema.get("oneOf").is_some() || schema.get("anyOf").is_some() {
        return TsType::Any; // simplify unions to any
    }

    let type_str = schema.get("type").and_then(|t| t.as_str()).unwrap_or("");

    match type_str {
        "string" => TsType::String,
        "integer" | "number" => TsType::Number,
        "boolean" => TsType::Boolean,
        "array" => {
            let items_type = schema
                .get("items")
                .map(|items| resolve_json_schema_type(prop_name, items, def_map, out))
                .unwrap_or(TsType::Any);
            TsType::Array(Box::new(items_type))
        }
        "object" => {
            if let Some(additional) = schema.get("additionalProperties") {
                if additional.is_boolean() {
                    return TsType::Map(Box::new(TsType::Any));
                }
                let val_type = resolve_json_schema_type(prop_name, additional, def_map, out);
                return TsType::Map(Box::new(val_type));
            }
            if schema.get("properties").is_some() {
                // Nested object with properties → extract as named schema
                let ts_name = to_pascal_case(prop_name);
                if !out.iter().any(|e| e.info.ts_name == ts_name) {
                    extract_schemas(schema, &ts_name, out);
                }
                return TsType::Ref(ts_name);
            }
            TsType::Map(Box::new(TsType::Any))
        }
        _ => TsType::Any,
    }
}

/// Like `resolve_json_schema_type` but for the Spec interface — uses the
/// Spec name for nested object refs (e.g., `ImageSpec` instead of `Image`).
#[allow(clippy::only_used_in_recursion)]
fn resolve_json_schema_type_for_spec(
    prop_name: &str,
    schema: &serde_json::Value,
    def_map: &HashMap<String, &serde_json::Value>,
    out: &mut Vec<ExtractedSchema>,
) -> TsType {
    // Handle $ref
    if let Some(ref_str) = schema.get("$ref").and_then(|r| r.as_str()) {
        let ref_name = ref_str.rsplit('/').next().unwrap_or(ref_str);
        let ts_name = to_pascal_case(ref_name);
        let spec_name = format!("{ts_name}Spec");
        if out.iter().any(|e| e.info.ts_name == spec_name) {
            return TsType::Ref(spec_name);
        }
        return TsType::Ref(ts_name);
    }

    let type_str = schema.get("type").and_then(|t| t.as_str()).unwrap_or("");
    match type_str {
        "string" => TsType::String,
        "integer" | "number" => TsType::Number,
        "boolean" => TsType::Boolean,
        "array" => {
            let items_type = schema
                .get("items")
                .map(|items| resolve_json_schema_type_for_spec(prop_name, items, def_map, out))
                .unwrap_or(TsType::Any);
            TsType::Array(Box::new(items_type))
        }
        "object" => {
            if let Some(additional) = schema.get("additionalProperties") {
                if additional.is_boolean() {
                    return TsType::Map(Box::new(TsType::Any));
                }
                let val_type =
                    resolve_json_schema_type_for_spec(prop_name, additional, def_map, out);
                return TsType::Map(Box::new(val_type));
            }
            if schema.get("properties").is_some() {
                let ts_name = to_pascal_case(prop_name);
                let spec_name = format!("{ts_name}Spec");
                if out.iter().any(|e| e.info.ts_name == spec_name) {
                    return TsType::Ref(spec_name);
                }
                return TsType::Ref(ts_name);
            }
            TsType::Map(Box::new(TsType::Any))
        }
        _ => TsType::Any,
    }
}

/// Convert a string to PascalCase.
fn to_pascal_case(s: &str) -> String {
    s.split(['-', '_', '.'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
            }
        })
        .collect()
}

/// Emit `.d.ts` content for chart types.
fn emit_chart_dts(schemas: &[ExtractedSchema]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "// Auto-generated by husako. Do not edit.\n");

    let has_builders = schemas.iter().any(|e| has_complex_property(&e.info));
    if has_builders {
        let _ = writeln!(out, "import {{ _SchemaBuilder }} from \"husako/_base\";\n");
    }

    // Emit Spec interfaces (names ending in "Spec")
    for schema in schemas {
        if schema.info.ts_name.ends_with("Spec") {
            emit_chart_interface(&mut out, &schema.info);
            let _ = writeln!(out);
        }
    }

    // Emit builder classes for schemas with complex properties
    for schema in schemas {
        if !schema.info.ts_name.ends_with("Spec") && has_complex_property(&schema.info) {
            emit_chart_builder_dts(&mut out, &schema.info);
            let _ = writeln!(out);
        }
    }

    // Emit plain interfaces for schemas without complex properties
    // (excluding Spec interfaces which are already emitted)
    for schema in schemas {
        if !schema.info.ts_name.ends_with("Spec") && !has_complex_property(&schema.info) {
            emit_chart_interface(&mut out, &schema.info);
            let _ = writeln!(out);
        }
    }

    out
}

fn emit_chart_interface(out: &mut String, schema: &SchemaInfo) {
    if let Some(desc) = &schema.description {
        let _ = writeln!(out, "/** {desc} */");
    }
    let _ = writeln!(out, "export interface {} {{", schema.ts_name);
    for prop in &schema.properties {
        if let Some(desc) = &prop.description {
            let _ = writeln!(out, "  /** {desc} */");
        }
        let opt = if prop.required { "" } else { "?" };
        // For builder types, accept both the builder class and the spec interface
        let type_str = format_param_type_dts(&prop.ts_type);
        let _ = writeln!(out, "  {}{}: {};", prop.name, opt, type_str);
    }
    let _ = writeln!(out, "}}");
}

fn emit_chart_builder_dts(out: &mut String, schema: &SchemaInfo) {
    if let Some(desc) = &schema.description {
        let _ = writeln!(out, "/** {desc} */");
    }
    let _ = writeln!(
        out,
        "export class {} extends _SchemaBuilder {{",
        schema.ts_name
    );
    for prop in &schema.properties {
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
    let _ = writeln!(out, "}}");

    let factory = to_factory_name(&schema.ts_name);
    let _ = writeln!(out, "export function {factory}(): {};", schema.ts_name);
}

/// Format a type for use in Spec interfaces — shows both builder and spec alternatives.
fn format_param_type_dts(ty: &TsType) -> String {
    format_ts_type(ty)
}

/// Emit `.js` content for chart types.
fn emit_chart_js(schemas: &[ExtractedSchema]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "// Auto-generated by husako. Do not edit.\n");

    let has_builders = schemas.iter().any(|e| has_complex_property(&e.info));
    if has_builders {
        let _ = writeln!(out, "import {{ _SchemaBuilder }} from \"husako/_base\";\n");
    }

    // Emit builder classes
    for schema in schemas {
        if !schema.info.ts_name.ends_with("Spec") && has_complex_property(&schema.info) {
            let _ = writeln!(
                out,
                "export class {} extends _SchemaBuilder {{",
                schema.info.ts_name
            );
            for prop in &schema.info.properties {
                let _ = writeln!(
                    out,
                    "  {}(v) {{ return this._set(\"{}\", v); }}",
                    prop.name, prop.name
                );
            }
            let _ = writeln!(out, "}}");

            let factory = to_factory_name(&schema.info.ts_name);
            let _ = writeln!(
                out,
                "export function {factory}() {{ return new {}(); }}\n",
                schema.info.ts_name
            );
        }
    }

    out
}

/// Convert a class name to its factory function name (first char lowercased).
fn to_factory_name(class_name: &str) -> String {
    let mut chars = class_name.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_lowercase().to_string() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn to_pascal_case_simple() {
        assert_eq!(to_pascal_case("hello"), "Hello");
        assert_eq!(to_pascal_case("hello-world"), "HelloWorld");
        assert_eq!(to_pascal_case("hello_world"), "HelloWorld");
        assert_eq!(to_pascal_case("replicaCount"), "ReplicaCount");
    }

    #[test]
    fn basic_schema_generates_types() {
        let schema = json!({
            "type": "object",
            "properties": {
                "replicaCount": { "type": "integer", "default": 1 },
                "image": {
                    "type": "object",
                    "properties": {
                        "repository": { "type": "string" },
                        "tag": { "type": "string" }
                    }
                }
            }
        });

        let (dts, js) = generate_chart_types("my-chart", &schema).unwrap();

        // DTS checks
        assert!(dts.contains("export interface ValuesSpec"));
        assert!(dts.contains("replicaCount?: number;"));
        assert!(dts.contains("export class Values extends _SchemaBuilder"));
        assert!(dts.contains("replicaCount(value: number): this;"));
        assert!(dts.contains("image(value: Image): this;"));
        assert!(dts.contains("export function values(): Values;"));
        assert!(dts.contains("export interface ImageSpec"));

        // JS checks
        assert!(js.contains("export class Values extends _SchemaBuilder"));
        assert!(js.contains("replicaCount(v) { return this._set(\"replicaCount\", v); }"));
        assert!(js.contains("export function values() { return new Values(); }"));
    }

    #[test]
    fn nested_object_extracted_as_builder() {
        let schema = json!({
            "type": "object",
            "properties": {
                "controller": {
                    "type": "object",
                    "properties": {
                        "replicaCount": { "type": "integer" },
                        "image": {
                            "type": "object",
                            "properties": {
                                "repository": { "type": "string" },
                                "tag": { "type": "string" }
                            }
                        }
                    }
                }
            }
        });

        let (dts, js) = generate_chart_types("test", &schema).unwrap();

        // Controller should be a builder (has nested Image ref)
        assert!(dts.contains("export class Controller extends _SchemaBuilder"));
        assert!(dts.contains("export function controller(): Controller;"));
        assert!(js.contains("export class Controller extends _SchemaBuilder"));
        assert!(js.contains("export function controller() { return new Controller(); }"));

        // Image should be a plain interface (only primitive props)
        assert!(dts.contains("export interface ImageSpec"));
    }

    #[test]
    fn simple_object_becomes_interface() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "replicas": { "type": "integer" }
            }
        });

        let (dts, _js) = generate_chart_types("test", &schema).unwrap();

        // No complex properties → no builder class, just interface
        assert!(dts.contains("export interface ValuesSpec"));
        assert!(!dts.contains("class Values"));
    }

    #[test]
    fn array_properties() {
        let schema = json!({
            "type": "object",
            "properties": {
                "ports": {
                    "type": "array",
                    "items": { "type": "integer" }
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            }
        });

        let (dts, _js) = generate_chart_types("test", &schema).unwrap();
        assert!(dts.contains("ports?: number[];"));
        assert!(dts.contains("tags?: string[];"));
    }

    #[test]
    fn map_properties() {
        let schema = json!({
            "type": "object",
            "properties": {
                "labels": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "extra": {
                    "type": "object",
                    "additionalProperties": true
                }
            }
        });

        let (dts, _js) = generate_chart_types("test", &schema).unwrap();
        assert!(dts.contains("labels?: Record<string, string>;"));
        assert!(dts.contains("extra?: Record<string, any>;"));
    }

    #[test]
    fn ref_to_defs() {
        let schema = json!({
            "type": "object",
            "properties": {
                "config": { "$ref": "#/$defs/AppConfig" }
            },
            "$defs": {
                "AppConfig": {
                    "type": "object",
                    "properties": {
                        "debug": { "type": "boolean" },
                        "nested": {
                            "type": "object",
                            "properties": {
                                "level": { "type": "integer" }
                            }
                        }
                    }
                }
            }
        });

        let (dts, js) = generate_chart_types("test", &schema).unwrap();

        // AppConfig should be extracted as a builder (has nested Ref)
        assert!(dts.contains("export class AppConfig extends _SchemaBuilder"));
        assert!(js.contains("export class AppConfig extends _SchemaBuilder"));
    }

    #[test]
    fn required_properties() {
        let schema = json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": { "type": "string" },
                "optional": { "type": "string" }
            }
        });

        let (dts, _js) = generate_chart_types("test", &schema).unwrap();
        assert!(dts.contains("name: string;"));
        assert!(dts.contains("optional?: string;"));
    }

    #[test]
    fn snapshot_basic_chart() {
        let schema = json!({
            "type": "object",
            "properties": {
                "replicaCount": { "type": "integer", "default": 1, "description": "Number of replicas" },
                "image": {
                    "type": "object",
                    "properties": {
                        "repository": { "type": "string", "description": "Container image repository" },
                        "tag": { "type": "string" }
                    }
                },
                "service": {
                    "type": "object",
                    "properties": {
                        "type": { "type": "string" },
                        "port": { "type": "integer" }
                    }
                },
                "labels": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                }
            }
        });

        let (dts, js) = generate_chart_types("my-chart", &schema).unwrap();
        insta::assert_snapshot!("chart_basic_dts", dts);
        insta::assert_snapshot!("chart_basic_js", js);
    }
}
