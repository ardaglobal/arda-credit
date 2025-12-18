//! OpenAPI component pruning utilities
//!
//! This module provides utilities to prune OpenAPI specifications to only include
//! components (schemas, parameters, etc.) that are actually referenced by the
//! filtered paths in the specification.
//!
//! This solves the problem where substring-based schema filtering includes unrelated
//! components (e.g., "Batch" pattern matching both "BatchInfo" and "BatchCreateFeeLettersRequest").

use std::collections::HashSet;
use utoipa::openapi::OpenApi;

/// Prune components to only include schemas/parameters/etc that are actually referenced
/// by the paths in the OpenAPI specification.
///
/// This function:
/// 1. Scans all paths, operations, and their parameters/request bodies/responses
/// 2. Collects all `$ref` references to components
/// 3. Computes the transitive closure of schema references
/// 4. Removes any components not in the referenced set
///
/// # Arguments
/// * `spec` - Mutable reference to the OpenAPI specification to prune
pub fn prune_components_to_references(spec: &mut OpenApi) {
    // Extract components temporarily to avoid self-referencing during scan
    let mut components = match spec.components.take() {
        Some(c) => c,
        None => return, // No components to prune
    };

    // Collect all schema references from paths
    let referenced_schemas = collect_referenced_schemas(spec);

    // Compute transitive closure of schema references
    let all_referenced = compute_transitive_closure(&components, &referenced_schemas);

    // Prune schemas to only include referenced ones
    let original_count = components.schemas.len();
    components
        .schemas
        .retain(|name, _| all_referenced.contains(name));
    let pruned_count = original_count - components.schemas.len();

    if pruned_count > 0 {
        tracing::debug!(
            "Pruned {} unreferenced schemas from OpenAPI spec (kept {} / {})",
            pruned_count,
            components.schemas.len(),
            original_count
        );
    }

    // Reattach components
    spec.components = Some(components);
}

/// Collect all schema references from the OpenAPI spec's paths
fn collect_referenced_schemas(spec: &OpenApi) -> HashSet<String> {
    let mut refs = HashSet::new();

    // Serialize the spec (without components) to JSON for easy scanning
    match serde_json::to_value(spec) {
        Ok(value) => {
            collect_refs_from_value(&value, &mut refs);
        }
        Err(e) => {
            tracing::warn!(
                "Failed to serialize OpenAPI spec for reference scanning: {}",
                e
            );
        }
    }

    refs
}

/// Recursively collect all `$ref` strings from a serde_json::Value
fn collect_refs_from_value(value: &serde_json::Value, refs: &mut HashSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            // Check if this object has a "$ref" key
            if let Some(serde_json::Value::String(ref_str)) = map.get("$ref") {
                // Extract schema name from "#/components/schemas/SchemaName"
                if let Some(schema_name) = ref_str.strip_prefix("#/components/schemas/") {
                    refs.insert(schema_name.to_string());
                }
            }
            // Recursively scan all values in the object
            for value in map.values() {
                collect_refs_from_value(value, refs);
            }
        }
        serde_json::Value::Array(arr) => {
            // Recursively scan all array elements
            for item in arr {
                collect_refs_from_value(item, refs);
            }
        }
        _ => {
            // Primitives (string, number, bool, null) - nothing to scan
        }
    }
}

/// Compute the transitive closure of schema references
///
/// Starting from the initially referenced schemas, follow all nested `$ref`s
/// within those schemas to find all transitively referenced schemas.
fn compute_transitive_closure(
    components: &utoipa::openapi::Components,
    initial_refs: &HashSet<String>,
) -> HashSet<String> {
    let mut all_refs = initial_refs.clone();
    let mut to_process: Vec<String> = initial_refs.iter().cloned().collect();
    let mut processed = HashSet::new();

    while let Some(schema_name) = to_process.pop() {
        if processed.contains(&schema_name) {
            continue;
        }
        processed.insert(schema_name.clone());

        // Get the schema from components
        if let Some(schema_ref) = components.schemas.get(&schema_name) {
            // Serialize the schema to find nested $refs
            match serde_json::to_value(schema_ref) {
                Ok(value) => {
                    let mut nested_refs = HashSet::new();
                    collect_refs_from_value(&value, &mut nested_refs);

                    // Add any new refs to the processing queue and result set
                    for nested_ref in nested_refs {
                        if all_refs.insert(nested_ref.clone()) {
                            to_process.push(nested_ref);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to serialize schema '{}' for nested reference scanning: {}",
                        schema_name,
                        e
                    );
                }
            }
        }
    }

    all_refs
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_collect_refs_from_simple_ref() {
        let value = json!({
            "$ref": "#/components/schemas/User"
        });
        let mut refs = HashSet::new();
        collect_refs_from_value(&value, &mut refs);
        assert!(refs.contains("User"));
        assert_eq!(refs.len(), 1);
    }

    #[test]
    fn test_collect_refs_from_nested_object() {
        let value = json!({
            "type": "object",
            "properties": {
                "user": {
                    "$ref": "#/components/schemas/User"
                },
                "items": {
                    "type": "array",
                    "items": {
                        "$ref": "#/components/schemas/Item"
                    }
                }
            }
        });
        let mut refs = HashSet::new();
        collect_refs_from_value(&value, &mut refs);
        assert!(refs.contains("User"));
        assert!(refs.contains("Item"));
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn test_collect_refs_from_array() {
        let value = json!([
            {"$ref": "#/components/schemas/SchemaA"},
            {"$ref": "#/components/schemas/SchemaB"}
        ]);
        let mut refs = HashSet::new();
        collect_refs_from_value(&value, &mut refs);
        assert!(refs.contains("SchemaA"));
        assert!(refs.contains("SchemaB"));
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn test_collect_refs_ignores_non_schema_refs() {
        let value = json!({
            "$ref": "#/components/parameters/SomeParam"
        });
        let mut refs = HashSet::new();
        collect_refs_from_value(&value, &mut refs);
        assert!(refs.is_empty());
    }

    #[test]
    fn test_collect_refs_handles_no_refs() {
        let value = json!({
            "type": "string",
            "example": "hello"
        });
        let mut refs = HashSet::new();
        collect_refs_from_value(&value, &mut refs);
        assert!(refs.is_empty());
    }
}
