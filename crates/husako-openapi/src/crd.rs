use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::OpenApiError;

/// Convert CRD YAML (one or more documents) to the OpenAPI JSON format
/// expected by `husako-dts`.
///
/// Returns `{"components": {"schemas": { ... }}}` with all extracted schemas.
pub fn crd_yaml_to_openapi(yaml: &str) -> Result<Value, OpenApiError> {
    let mut schemas = BTreeMap::new();

    for doc in serde_yaml_ng::Deserializer::from_str(yaml) {
        let value: Value = serde::Deserialize::deserialize(doc)
            .map_err(|e| crd_err(format!("YAML parse: {e}")))?;

        if !is_crd(&value) {
            continue;
        }

        extract_crd(&value, &mut schemas)?;
    }

    if schemas.is_empty() {
        return Err(crd_err("no valid CRD documents found".to_string()));
    }

    let schemas_obj: Map<String, Value> = schemas.into_iter().collect();
    Ok(json!({
        "components": {
            "schemas": schemas_obj
        }
    }))
}

/// Check if a YAML document is a CRD (`apiextensions.k8s.io/v1`).
fn is_crd(doc: &Value) -> bool {
    let api_version = doc.get("apiVersion").and_then(Value::as_str);
    let kind = doc.get("kind").and_then(Value::as_str);
    matches!(
        (api_version, kind),
        (
            Some("apiextensions.k8s.io/v1"),
            Some("CustomResourceDefinition")
        )
    )
}

/// Extract schemas from a single CRD document.
fn extract_crd(crd: &Value, schemas: &mut BTreeMap<String, Value>) -> Result<(), OpenApiError> {
    let spec = crd
        .get("spec")
        .ok_or_else(|| crd_err("missing spec".to_string()))?;

    let group = spec
        .get("group")
        .and_then(Value::as_str)
        .ok_or_else(|| crd_err("missing spec.group".to_string()))?;

    let kind = spec
        .pointer("/names/kind")
        .and_then(Value::as_str)
        .ok_or_else(|| crd_err("missing spec.names.kind".to_string()))?;

    let versions = spec
        .get("versions")
        .and_then(Value::as_array)
        .ok_or_else(|| crd_err("missing spec.versions".to_string()))?;

    let prefix = reverse_domain(group);

    for ver in versions {
        let version = ver
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| crd_err("missing version name".to_string()))?;

        let openapi_schema = ver.pointer("/schema/openAPIV3Schema");
        let Some(raw_schema) = openapi_schema else {
            continue;
        };

        let base = format!("{prefix}.{version}");

        // Extract nested schemas from the openAPIV3Schema
        let mut extracted = BTreeMap::new();
        let top_schema = extract_nested_schemas(raw_schema, kind, &base, &mut extracted)?;

        // Build the top-level resource schema
        let resource_name = format!("{base}.{kind}");
        let resource_schema = build_resource_schema(top_schema, group, version, kind);
        schemas.insert(resource_name, resource_schema);

        // Insert all extracted sub-schemas
        schemas.extend(extracted);
    }

    Ok(())
}

/// Recursively extract nested object properties into separate named schemas.
///
/// Returns the transformed schema with nested objects replaced by `$ref`.
fn extract_nested_schemas(
    schema: &Value,
    context_name: &str,
    base: &str,
    extracted: &mut BTreeMap<String, Value>,
) -> Result<Value, OpenApiError> {
    let Some(obj) = schema.as_object() else {
        return Ok(schema.clone());
    };

    let mut result = obj.clone();

    // Process properties
    if let Some(Value::Object(props)) = result.get("properties") {
        let mut new_props = Map::new();
        for (prop_name, prop_schema) in props {
            let new_schema =
                maybe_extract_property(prop_schema, prop_name, context_name, base, extracted)?;
            new_props.insert(prop_name.clone(), new_schema);
        }
        result.insert("properties".to_string(), Value::Object(new_props));
    }

    // Process array items
    if let Some(items) = obj.get("items") {
        let new_items = maybe_extract_property(items, context_name, context_name, base, extracted)?;
        result.insert("items".to_string(), new_items);
    }

    Ok(Value::Object(result))
}

/// Decide whether a property schema should be extracted into a separate named schema.
fn maybe_extract_property(
    prop_schema: &Value,
    prop_name: &str,
    context_name: &str,
    base: &str,
    extracted: &mut BTreeMap<String, Value>,
) -> Result<Value, OpenApiError> {
    let Some(obj) = prop_schema.as_object() else {
        return Ok(prop_schema.clone());
    };

    // Extract inline objects with properties
    if is_extractable_object(obj) {
        let sub_name = format!("{context_name}{}", to_pascal_case(prop_name));
        let full_name = format!("{base}.{sub_name}");

        let sub_schema = extract_nested_schemas(prop_schema, &sub_name, base, extracted)?;

        // Preserve description from the property
        let desc = obj.get("description").cloned();

        extracted.insert(full_name.clone(), sub_schema);

        // Return a $ref (optionally with description)
        let ref_value = format!("#/components/schemas/{full_name}");
        return if let Some(d) = desc {
            Ok(json!({ "$ref": ref_value, "description": d }))
        } else {
            Ok(json!({ "$ref": ref_value }))
        };
    }

    // Recurse into array items
    if obj.get("type").and_then(Value::as_str) == Some("array")
        && let Some(items) = obj.get("items")
        && items.as_object().is_some_and(is_extractable_object)
    {
        let sub_name = format!("{context_name}{}", to_pascal_case(prop_name));
        let full_name = format!("{base}.{sub_name}");

        let sub_schema = extract_nested_schemas(items, &sub_name, base, extracted)?;
        extracted.insert(full_name.clone(), sub_schema);

        let mut result = obj.clone();
        result.insert(
            "items".to_string(),
            json!({ "$ref": format!("#/components/schemas/{full_name}") }),
        );
        return Ok(Value::Object(result));
    }

    Ok(prop_schema.clone())
}

/// An object is extractable if it has type "object" and has named properties.
fn is_extractable_object(obj: &Map<String, Value>) -> bool {
    obj.get("type").and_then(Value::as_str) == Some("object")
        && obj.get("properties").is_some_and(|p| p.is_object())
}

/// Build the top-level resource schema with standard Kubernetes fields.
fn build_resource_schema(mut schema: Value, group: &str, version: &str, kind: &str) -> Value {
    let obj = schema.as_object_mut().unwrap();

    // Ensure properties map exists
    let props = obj
        .entry("properties")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .unwrap();

    // Add apiVersion/kind/metadata if not present
    props
        .entry("apiVersion")
        .or_insert_with(|| json!({"description": "APIVersion defines the versioned schema of this representation of an object.", "type": "string"}));
    props
        .entry("kind")
        .or_insert_with(|| json!({"description": "Kind is a string value representing the REST resource this object represents.", "type": "string"}));
    props.entry("metadata").or_insert_with(
        || json!({"$ref": "#/components/schemas/io.k8s.apimachinery.pkg.apis.meta.v1.ObjectMeta"}),
    );

    // Add x-kubernetes-group-version-kind
    obj.insert(
        "x-kubernetes-group-version-kind".to_string(),
        json!([{ "group": group, "kind": kind, "version": version }]),
    );

    // Ensure type is "object"
    obj.entry("type").or_insert_with(|| json!("object"));

    schema
}

/// Reverse a domain name for schema naming.
/// `cert-manager.io` → `io.cert-manager`
/// `postgresql.cnpg.io` → `io.cnpg.postgresql`
fn reverse_domain(group: &str) -> String {
    let parts: Vec<&str> = group.split('.').collect();
    let reversed: Vec<&str> = parts.into_iter().rev().collect();
    reversed.join(".")
}

/// Convert a string to PascalCase.
/// `spec` → `Spec`, `privateKey` → `PrivateKey`, `dns_names` → `DnsNames`
fn to_pascal_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = true;
    for ch in s.chars() {
        if ch == '_' || ch == '-' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}

fn crd_err(msg: String) -> OpenApiError {
    OpenApiError::Crd(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_CRD: &str = r#"
apiVersion: apiextensions.k8s.io/v1
kind: CustomResourceDefinition
metadata:
  name: certificates.cert-manager.io
spec:
  group: cert-manager.io
  names:
    kind: Certificate
    plural: certificates
  scope: Namespaced
  versions:
    - name: v1
      served: true
      storage: true
      schema:
        openAPIV3Schema:
          type: object
          properties:
            spec:
              type: object
              properties:
                secretName:
                  type: string
                  description: Name of the Secret resource.
                issuerRef:
                  type: object
                  description: Reference to the issuer.
                  properties:
                    name:
                      type: string
                    kind:
                      type: string
                    group:
                      type: string
                  required:
                    - name
                duration:
                  type: string
                isCA:
                  type: boolean
              required:
                - secretName
                - issuerRef
            status:
              type: object
              properties:
                ready:
                  type: boolean
                conditions:
                  type: array
                  items:
                    type: object
                    properties:
                      type:
                        type: string
                      status:
                        type: string
                    required:
                      - type
                      - status
"#;

    #[test]
    fn simple_crd_conversion() {
        let result = crd_yaml_to_openapi(SIMPLE_CRD).unwrap();
        let schemas = &result["components"]["schemas"];

        // Top-level resource exists with GVK
        let cert = &schemas["io.cert-manager.v1.Certificate"];
        assert!(cert["x-kubernetes-group-version-kind"].is_array());
        let gvk = &cert["x-kubernetes-group-version-kind"][0];
        assert_eq!(gvk["group"], "cert-manager.io");
        assert_eq!(gvk["kind"], "Certificate");
        assert_eq!(gvk["version"], "v1");

        // Has apiVersion, kind, metadata
        assert!(cert["properties"]["apiVersion"]["type"].is_string());
        assert!(cert["properties"]["kind"]["type"].is_string());
        assert!(cert["properties"]["metadata"]["$ref"].is_string());
    }

    #[test]
    fn nested_extraction() {
        let result = crd_yaml_to_openapi(SIMPLE_CRD).unwrap();
        let schemas = &result["components"]["schemas"];

        // Spec was extracted
        let cert = &schemas["io.cert-manager.v1.Certificate"];
        assert!(
            cert["properties"]["spec"]["$ref"]
                .as_str()
                .unwrap()
                .contains("CertificateSpec")
        );

        // Spec schema exists
        let spec = &schemas["io.cert-manager.v1.CertificateSpec"];
        assert_eq!(spec["properties"]["secretName"]["type"], "string");
        assert_eq!(spec["properties"]["isCA"]["type"], "boolean");
        assert!(spec["required"].as_array().unwrap().len() >= 2);

        // Nested issuerRef was extracted
        assert!(
            spec["properties"]["issuerRef"]["$ref"]
                .as_str()
                .unwrap()
                .contains("IssuerRef")
        );
    }

    #[test]
    fn array_items_extraction() {
        let result = crd_yaml_to_openapi(SIMPLE_CRD).unwrap();
        let schemas = &result["components"]["schemas"];

        // Status was extracted
        let status = &schemas["io.cert-manager.v1.CertificateStatus"];
        assert!(status.is_object());

        // conditions array items were extracted
        let conditions = &status["properties"]["conditions"];
        assert_eq!(conditions["type"], "array");
        assert!(conditions["items"]["$ref"].as_str().is_some());
    }

    #[test]
    fn gvk_present_on_resource() {
        let result = crd_yaml_to_openapi(SIMPLE_CRD).unwrap();
        let cert = &result["components"]["schemas"]["io.cert-manager.v1.Certificate"];
        let gvk = cert["x-kubernetes-group-version-kind"].as_array().unwrap();
        assert_eq!(gvk.len(), 1);
        assert_eq!(gvk[0]["group"], "cert-manager.io");
    }

    #[test]
    fn multi_version_crd() {
        let yaml = r#"
apiVersion: apiextensions.k8s.io/v1
kind: CustomResourceDefinition
metadata:
  name: widgets.example.com
spec:
  group: example.com
  names:
    kind: Widget
    plural: widgets
  scope: Namespaced
  versions:
    - name: v1
      served: true
      storage: true
      schema:
        openAPIV3Schema:
          type: object
          properties:
            spec:
              type: object
              properties:
                size:
                  type: integer
    - name: v1beta1
      served: true
      storage: false
      schema:
        openAPIV3Schema:
          type: object
          properties:
            spec:
              type: object
              properties:
                count:
                  type: integer
"#;
        let result = crd_yaml_to_openapi(yaml).unwrap();
        let schemas = &result["components"]["schemas"];

        assert!(schemas["com.example.v1.Widget"].is_object());
        assert!(schemas["com.example.v1beta1.Widget"].is_object());
        assert!(schemas["com.example.v1.WidgetSpec"]["properties"]["size"].is_object());
        assert!(schemas["com.example.v1beta1.WidgetSpec"]["properties"]["count"].is_object());
    }

    #[test]
    fn multi_doc_yaml() {
        let yaml = r#"
apiVersion: v1
kind: Namespace
metadata:
  name: test
---
apiVersion: apiextensions.k8s.io/v1
kind: CustomResourceDefinition
metadata:
  name: things.test.io
spec:
  group: test.io
  names:
    kind: Thing
    plural: things
  scope: Namespaced
  versions:
    - name: v1
      served: true
      storage: true
      schema:
        openAPIV3Schema:
          type: object
          properties:
            spec:
              type: object
              properties:
                value:
                  type: string
---
apiVersion: apiextensions.k8s.io/v1
kind: CustomResourceDefinition
metadata:
  name: gadgets.test.io
spec:
  group: test.io
  names:
    kind: Gadget
    plural: gadgets
  scope: Namespaced
  versions:
    - name: v1
      served: true
      storage: true
      schema:
        openAPIV3Schema:
          type: object
          properties:
            spec:
              type: object
              properties:
                name:
                  type: string
"#;
        let result = crd_yaml_to_openapi(yaml).unwrap();
        let schemas = &result["components"]["schemas"];

        // Non-CRD documents are skipped
        assert!(schemas.get("Namespace").is_none());

        // Both CRDs are processed
        assert!(schemas["io.test.v1.Thing"].is_object());
        assert!(schemas["io.test.v1.Gadget"].is_object());
    }

    #[test]
    fn non_crd_only_returns_error() {
        let yaml = r#"
apiVersion: v1
kind: Namespace
metadata:
  name: test
"#;
        let err = crd_yaml_to_openapi(yaml).unwrap_err();
        assert!(err.to_string().contains("no valid CRD"));
    }

    #[test]
    fn metadata_ref_added() {
        let result = crd_yaml_to_openapi(SIMPLE_CRD).unwrap();
        let cert = &result["components"]["schemas"]["io.cert-manager.v1.Certificate"];
        let meta_ref = cert["properties"]["metadata"]["$ref"].as_str().unwrap();
        assert!(meta_ref.contains("ObjectMeta"));
    }

    #[test]
    fn reverse_domain_conversion() {
        assert_eq!(reverse_domain("cert-manager.io"), "io.cert-manager");
        assert_eq!(reverse_domain("postgresql.cnpg.io"), "io.cnpg.postgresql");
        assert_eq!(
            reverse_domain("kustomize.toolkit.fluxcd.io"),
            "io.fluxcd.toolkit.kustomize"
        );
        assert_eq!(reverse_domain("example.com"), "com.example");
    }

    #[test]
    fn pascal_case_conversion() {
        assert_eq!(to_pascal_case("spec"), "Spec");
        assert_eq!(to_pascal_case("privateKey"), "PrivateKey");
        assert_eq!(to_pascal_case("dns_names"), "DnsNames");
        assert_eq!(to_pascal_case("issuerRef"), "IssuerRef");
    }
}
