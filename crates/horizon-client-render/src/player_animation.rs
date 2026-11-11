//! Safe first-stage loader for OptiFine CEM player model resources.
//!
//! This module deliberately does not claim to render Fresh Animations yet.
//! It parses the JSON resource files and keeps their source tree intact so a
//! later model/animation evaluator can consume the exact pack-specific schema.
use serde_json::Value;
use std::{collections::BTreeSet, path::Path};

/// Maximum uncompressed JSON size accepted for one CEM resource file.
pub const MAX_RESOURCE_BYTES: usize = 1 << 20;
/// Maximum nesting depth accepted in a resource document.
pub const MAX_JSON_DEPTH: usize = 64;
/// Maximum JSON values visited in one document.
pub const MAX_JSON_NODES: usize = 20_000;

/// Parsed JSON resource and metadata used by the later CEM evaluator.
#[derive(Clone, Debug)]
pub struct CemResource {
    /// Original JSON tree; fields not yet understood are preserved.
    pub document: Value,
    /// String-valued fields that look like CEM expressions.
    pub expressions: Vec<String>,
    /// Number of objects in model/submodel arrays, when present.
    pub model_nodes: usize,
}

/// A pair of player model variants and their referenced animation modules.
#[derive(Clone, Debug)]
pub struct PlayerModelResources {
    pub normal: CemResource,
    pub slim: CemResource,
    pub modules: BTreeSet<String>,
}

/// Parse a player .jem model file.
pub fn parse_jem(source: &str) -> Result<CemResource, String> {
    let resource = parse_json_resource(source, "JEM")?;
    let models = resource.document.get("models")
        .and_then(Value::as_array)
        .ok_or("JEM: expected a top-level models array")?;
    if models.is_empty() {
        return Err("JEM: models array is empty".into());
    }
    Ok(resource)
}

/// Parse a referenced .jpm module. The original document is retained because
/// OptiFine CEM extensions may add fields this loader does not interpret yet.
pub fn parse_jpm(source: &str) -> Result<CemResource, String> {
    parse_json_resource(source, "JPM")
}

/// Load the two known player model files from a directory supplied by the
/// caller. This function does not assume a particular resource-pack root.
pub fn load_player_models(root: &Path) -> Result<PlayerModelResources, String> {
    let normal_path = root.join("player.jem");
    let slim_path = root.join("player_slim.jem");
    let normal_text = read_resource(&normal_path)?;
    let slim_text = read_resource(&slim_path)?;
    let normal = parse_jem(&normal_text)?;
    let slim = parse_jem(&slim_text)?;
    let mut modules = BTreeSet::new();
    collect_module_references(&normal.document, &mut modules);
    collect_module_references(&slim.document, &mut modules);
    Ok(PlayerModelResources { normal, slim, modules })
}

/// Read a resource with a hard size cap before allocating its full contents.
pub fn read_resource(path: &Path) -> Result<String, String> {
    let metadata = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if metadata.len() > MAX_RESOURCE_BYTES as u64 {
        return Err(format!("{}: resource exceeds {} bytes", path.display(), MAX_RESOURCE_BYTES));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    String::from_utf8(bytes).map_err(|_| format!("{}: resource is not UTF-8", path.display()))
}

fn parse_json_resource(source: &str, kind: &str) -> Result<CemResource, String> {
    if source.len() > MAX_RESOURCE_BYTES {
        return Err(format!("{kind}: resource exceeds {} bytes", MAX_RESOURCE_BYTES));
    }
    let document: Value = serde_json::from_str(source)
        .map_err(|e| format!("{kind}: invalid JSON: {e}"))?;
    if !document.is_object() {
        return Err(format!("{kind}: top-level JSON value must be an object"));
    }
    let mut count = 0usize;
    let mut expressions = Vec::new();
    inspect_value(&document, 0, &mut count, &mut expressions)?;
    let model_nodes = count_model_nodes(&document);
    Ok(CemResource { document, expressions, model_nodes })
}

fn inspect_value(
    value: &Value,
    depth: usize,
    count: &mut usize,
    expressions: &mut Vec<String>,
) -> Result<(), String> {
    if depth > MAX_JSON_DEPTH {
        return Err(format!("CEM resource exceeds maximum JSON depth ({MAX_JSON_DEPTH})"));
    }
    *count += 1;
    if *count > MAX_JSON_NODES {
        return Err(format!("CEM resource exceeds maximum JSON node count ({MAX_JSON_NODES})"));
    }
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if let Some(text) = child.as_str()
                    && looks_like_expression(key, text)
                {
                    expressions.push(text.to_owned());
                }
                inspect_value(child, depth + 1, count, expressions)?;
            }
        }
        Value::Array(items) => {
            for child in items {
                inspect_value(child, depth + 1, count, expressions)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn looks_like_expression(key: &str, text: &str) -> bool {
    let key = key.to_ascii_lowercase();
    let expression_field = key.contains("anim")
        || key.ends_with(".rx") || key.ends_with(".ry") || key.ends_with(".rz")
        || key.ends_with(".tx") || key.ends_with(".ty") || key.ends_with(".tz")
        || key.ends_with(".sx") || key.ends_with(".sy") || key.ends_with(".sz");
    expression_field && !text.trim().is_empty()
}

fn count_model_nodes(document: &Value) -> usize {
    fn visit(value: &Value) -> usize {
        match value {
            Value::Object(map) => {
                let own = if map.contains_key("id") && (map.contains_key("boxes") || map.contains_key("submodels")) { 1 } else { 0 };
                own + map.values().map(visit).sum::<usize>()
            }
            Value::Array(items) => items.iter().map(visit).sum(),
            _ => 0,
        }
    }
    visit(document)
}

fn collect_module_references(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key.to_ascii_lowercase().contains("model")
                    && let Some(name) = child.as_str()
                    && name.ends_with(".jpm")
                    && !name.contains('/') && !name.contains('\\')
                {
                    out.insert(name.to_owned());
                }
                collect_module_references(child, out);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_module_references(child, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_player_model_and_preserves_unknown_fields() {
        let jem = r#"{"texture":"player.png","models":[{"id":"body","boxes":[],"future_field":{"keep":true}}]}"#;
        let parsed = parse_jem(jem).unwrap();
        assert_eq!(parsed.model_nodes, 1);
        assert_eq!(parsed.document["models"][0]["future_field"]["keep"], true);
    }

    #[test]
    fn rejects_invalid_or_unbounded_documents() {
        assert!(parse_jem(r#"{"models":[]}"#).is_err());
        assert!(parse_jpm(&" ".repeat(MAX_RESOURCE_BYTES + 1)).is_err());
        let deep = format!(r#"{{"a":{}}}"#, "[".repeat(MAX_JSON_DEPTH + 1) + "0" + &"]".repeat(MAX_JSON_DEPTH + 1));
        assert!(parse_jpm(&deep).is_err());
    }

    #[test]
    fn extracts_expression_fields_and_jpm_references() {
        let jem = r#"{"models":[{"id":"body","boxes":[],"animations":{"body.rx":"sin(time)","body.ry":"cos(time)"}}],"model":"player_idle.jpm"}"#;
        let parsed = parse_jem(jem).unwrap();
        assert_eq!(parsed.expressions.len(), 2);
        let mut refs = BTreeSet::new();
        collect_module_references(&parsed.document, &mut refs);
        assert!(refs.contains("player_idle.jpm"));
    }
}
