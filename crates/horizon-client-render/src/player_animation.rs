//! OptiFine CEM player-resource loading and expression evaluation.
//!
//! Horizon's player renderer consumes these resources to attach custom JEM
//! geometry and apply supported CEM animation channels to player poses.
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};

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
    /// Animation destination/expression pairs, preserving source order.
    /// CEM stores these in each model's "animations" array.
    pub animation_bindings: Vec<(String, String)>,
    /// Number of objects in model/submodel arrays, when present.
    pub model_nodes: usize,
}

/// A pair of player model variants and their referenced animation modules.
#[derive(Clone, Debug)]
pub struct PlayerModelResources {
    pub normal: CemResource,
    pub slim: CemResource,
    /// Referenced JPM modules loaded from the same CEM resource directory.
    /// The optional face module is intentionally excluded: this implementation
    /// targets body/player animation only.
    pub modules: BTreeMap<String, CemResource>,
    normal_animation_bindings: Vec<(String, String)>,
    slim_animation_bindings: Vec<(String, String)>,
    variable_state: Arc<Mutex<HashMap<u32, BTreeMap<String, f64>>>>,
    /// Per-player render timing for OptiFine CEM frame_time/frame_counter.
    frame_state: Arc<Mutex<HashMap<u32, (Instant, u32)>>>,
}

impl PlayerModelResources {
    /// Ordered animation bindings for the selected player arm geometry.
    pub fn animation_bindings(&self, slim: bool) -> &[(String, String)] {
        if slim {
            &self.slim_animation_bindings
        } else {
            &self.normal_animation_bindings
        }
    }

    /// Retrieve per-entity CEM variables that persist between rendered frames.
    pub fn variables_for_entity(&self, entity_id: u32) -> BTreeMap<String, f64> {
        self.variable_state
            .lock()
            .ok()
            .and_then(|state| state.get(&entity_id).cloned())
            .unwrap_or_default()
    }

    /// Return the elapsed render-frame time and per-entity frame counter.
    /// OptiFine defines frame_time in seconds, not ticks.
    pub fn frame_inputs_for_entity(&self, entity_id: u32) -> (u32, f64) {
        let now = Instant::now();
        let Ok(mut state) = self.frame_state.lock() else {
            return (0, 1.0 / 60.0);
        };
        if !state.contains_key(&entity_id) && state.len() >= 256 {
            state.clear();
        }
        let entry = state.entry(entity_id).or_insert((now, 0));
        let elapsed = now.duration_since(entry.0).as_secs_f64();
        entry.0 = now;
        entry.1 = (entry.1 + 1) % 27_720;
        (entry.1, if elapsed.is_finite() && elapsed > 0.0 {
            elapsed.min(0.25)
        } else {
            1.0 / 60.0
        })
    }

    /// Save only user-defined CEM variables; built-in render inputs are recalculated.
    pub fn store_variables_for_entity(
        &self,
        entity_id: u32,
        context: &ExpressionContext,
    ) {
        let variables = context
            .variables
            .iter()
            .filter(|(name, _)| name.starts_with("var.") || name.starts_with("varb."))
            .map(|(name, value)| (name.clone(), *value))
            .collect();
        let Ok(mut state) = self.variable_state.lock() else {
            return;
        };
        if !state.contains_key(&entity_id) && state.len() >= 256 {
            state.clear();
        }
        state.insert(entity_id, variables);
    }
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
    let mut module_names = BTreeSet::new();
    collect_module_references(&normal.document, &mut module_names);
    collect_module_references(&slim.document, &mut module_names);
    // Fresh Animations may reference player_face.jpm, but face animation is
    // explicitly outside this feature's scope.
    module_names.remove("player_face.jpm");

    let mut modules = BTreeMap::new();
    for name in module_names {
        let path = root.join(&name);
        let source = read_resource(&path)?;
        modules.insert(name, parse_jpm(&source)?);
    }

    let normal_animation_bindings = collect_player_animation_bindings(&normal, &modules);
    let slim_animation_bindings = collect_player_animation_bindings(&slim, &modules);
    Ok(PlayerModelResources {
        normal,
        slim,
        modules,
        normal_animation_bindings,
        slim_animation_bindings,
        variable_state: Arc::new(Mutex::new(HashMap::new())),
        frame_state: Arc::new(Mutex::new(HashMap::new())),
    })
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
    let mut animation_bindings = Vec::new();
    collect_animation_bindings(&document, &mut animation_bindings);
    let model_nodes = count_model_nodes(&document);
    Ok(CemResource { document, expressions, animation_bindings, model_nodes })
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

/// Collect OptiFine/EMF animation pairs from every model and JPM module.
/// The source format uses an array of objects; keeping a Vec preserves the
/// order in which a pack declares animation maps.
fn rewrite_expression_references(
    expression: &str,
    model_id: Option<&str>,
    part_name: Option<&str>,
) -> String {
    let chars: Vec<char> = expression.chars().collect();
    let mut output = String::with_capacity(expression.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_alphabetic() || chars[i] == '_' {
            let start = i;
            i += 1;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric()
                    || chars[i] == '_'
                    || chars[i] == '.'
                    || chars[i] == ':')
            {
                i += 1;
            }
            let token: String = chars[start..i].iter().collect();
            let rewritten = if let Some(id) = model_id
                && (token.starts_with("this.") || token.starts_with("this:"))
            {
                format!("{id}{}", &token[4..])
            } else if let Some(part) = part_name
                && (token.starts_with("part.") || token.starts_with("part:"))
            {
                format!("{part}{}", &token[4..])
            } else if token == "headwear"
                || token.starts_with("headwear.")
                || token.starts_with("headwear:")
            {
                format!("hat{}", &token["headwear".len()..])
            } else {
                token
            };
            output.push_str(&rewritten);
        } else {
            output.push(chars[i]);
            i += 1;
        }
    }
    output
}

fn rewrite_binding_target(
    target: &str,
    model_id: Option<&str>,
    part_name: Option<&str>,
) -> String {
    for (prefix, replacement) in [("this", model_id), ("part", part_name)] {
        if let Some(replacement) = replacement
            && let Some(suffix) = target.strip_prefix(prefix)
            && (suffix.starts_with('.') || suffix.starts_with(':'))
        {
            return format!("{replacement}{suffix}");
        }
    }
    if target == "headwear"
        || target.starts_with("headwear.")
        || target.starts_with("headwear:")
    {
        format!("hat{}", &target["headwear".len()..])
    } else {
        target.to_owned()
    }
}

fn collect_animation_bindings(value: &Value, out: &mut Vec<(String, String)>) {
    collect_animation_bindings_with_context(value, out, None, None);
}

fn collect_animation_bindings_with_context(
    value: &Value,
    out: &mut Vec<(String, String)>,
    inherited_id: Option<&str>,
    inherited_part: Option<&str>,
) {
    match value {
        Value::Object(map) => {
            let model_id = map.get("id").and_then(Value::as_str).or(inherited_id);
            let part_name = map.get("part").and_then(Value::as_str).or(inherited_part);
            for (key, child) in map {
                if key == "animations" {
                    collect_animation_entries(child, out, model_id, part_name);
                }
                collect_animation_bindings_with_context(child, out, model_id, part_name);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_animation_bindings_with_context(
                    child,
                    out,
                    inherited_id,
                    inherited_part,
                );
            }
        }
        _ => {}
    }
}

fn collect_animation_entries(
    value: &Value,
    out: &mut Vec<(String, String)>,
    model_id: Option<&str>,
    part_name: Option<&str>,
) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_animation_entries(item, out, model_id, part_name);
            }
        }
        Value::Object(map) => {
            for (target, expression) in map {
                if let Some(expression) = expression.as_str()
                    && !target.trim().is_empty()
                    && !expression.trim().is_empty()
                {
                    out.push((
                        rewrite_binding_target(target, model_id, part_name),
                        rewrite_expression_references(expression, model_id, part_name),
                    ));
                }
            }
        }
        _ => {}
    }
}

fn collect_player_animation_bindings(
    jem: &CemResource,
    modules: &BTreeMap<String, CemResource>,
) -> Vec<(String, String)> {
    fn visit(
        value: &Value,
        modules: &BTreeMap<String, CemResource>,
        out: &mut Vec<(String, String)>,
        inherited_id: Option<&str>,
        inherited_part: Option<&str>,
    ) {
        match value {
            Value::Object(map) => {
                let model_id = map.get("id").and_then(Value::as_str).or(inherited_id);
                let part_name = map.get("part").and_then(Value::as_str).or(inherited_part);
                if let Some(module_name) = map.get("model").and_then(Value::as_str)
                    && let Some(module) = modules.get(module_name)
                {
                    out.extend(module.animation_bindings.iter().map(|(target, expression)| {
                        (
                            rewrite_binding_target(target, model_id, part_name),
                            rewrite_expression_references(expression, model_id, part_name),
                        )
                    }));
                }
                for child in map.values() {
                    visit(child, modules, out, model_id, part_name);
                }
            }
            Value::Array(items) => {
                for child in items {
                    visit(child, modules, out, inherited_id, inherited_part);
                }
            }
            _ => {}
        }
    }

    let mut out = jem.animation_bindings.clone();
    visit(&jem.document, modules, &mut out, None, None);
    out
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
    fn rewrites_this_and_part_targets_to_the_enclosing_jem_model() {
        let jem = r#"{"models":[{"id":"head_custom","part":"head","animations":[{"this.rx":"part.ry + this:hand.rx","part.ry":"2","this:hand.rx":"3"}]}]}"#;
        let parsed = parse_jem(jem).unwrap();
        assert_eq!(
            parsed.animation_bindings,
            vec![
                (
                    "head_custom.rx".to_owned(),
                    "head.ry + head_custom:hand.rx".to_owned(),
                ),
                ("head.ry".to_owned(), "2".to_owned()),
                ("head_custom:hand.rx".to_owned(), "3".to_owned()),
            ]
        );
    }

    #[test]
    fn extracts_expression_fields_and_jpm_references() {
        let jem = r#"{"models":[{"id":"body","boxes":[],"animations":{"body.rx":"sin(time)","body.ry":"cos(time)"}}],"model":"player_idle.jpm"}"#;
        let parsed = parse_jem(jem).unwrap();
        assert_eq!(parsed.expressions.len(), 2);
        assert_eq!(
            parsed.animation_bindings,
            vec![
                ("body.rx".to_owned(), "sin(time)".to_owned()),
                ("body.ry".to_owned(), "cos(time)".to_owned()),
            ]
        );
        let mut refs = BTreeSet::new();
        collect_module_references(&parsed.document, &mut refs);
        assert!(refs.contains("player_idle.jpm"));
    }

    #[test]
    fn loads_referenced_jpm_modules_but_not_face_module() {
        let root = std::env::temp_dir().join(format!(
            "horizon-player-cem-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let jem = r#"{"models":[{"id":"body","boxes":[]}],"model":"player_idle.jpm","face_model":"player_face.jpm"}"#;
        std::fs::write(root.join("player.jem"), jem).unwrap();
        std::fs::write(root.join("player_slim.jem"), jem).unwrap();
        std::fs::write(root.join("player_idle.jpm"), r#"{"animations":{}}"#).unwrap();
        std::fs::write(root.join("player_face.jpm"), r#"{"animations":{}}"#).unwrap();

        let loaded = load_player_models(&root).unwrap();
        assert!(loaded.modules.contains_key("player_idle.jpm"));
        assert!(!loaded.modules.contains_key("player_face.jpm"));

        std::fs::remove_dir_all(root).unwrap();
    }
}


/// Per-frame numeric inputs supplied by the player renderer / CEM variable layer.
#[derive(Clone, Debug, Default)]
pub struct ExpressionContext {
    pub variables: std::collections::BTreeMap<String, f64>,
}

/// Evaluate the arithmetic and conditional subset used by CEM animation expressions.
/// Unknown variables/functions fail closed instead of silently becoming zero.
pub const MAX_EXPRESSION_BYTES: usize = 16 * 1024;

/// Replace the small, explicit NBT predicate subset used by Fresh Animations.
/// The player renderer supplies these booleans from its held-item and pose state;
/// unsupported NBT queries evaluate false rather than breaking the whole expression.
fn expand_nbt_predicates(source: &str, context: &ExpressionContext) -> String {
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find("nbt(") {
        let start = cursor + relative;
        output.push_str(&source[cursor..start]);
        let args_start = start + 4;
        let Some(close_relative) = source[args_start..].find(')') else {
            output.push_str(&source[start..]);
            return output;
        };
        let close = args_start + close_relative;
        let args: String = source[args_start..close]
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let key = match args.as_str() {
            "Inventory,raw:iregex:.*Slot:-106b.*filled_map.*" => "cem.nbt.off_map",
            "SelectedItem.id,raw:iregex:.*filled_map.*" => "cem.nbt.main_map",
            "SleepingX,exists:true" => "cem.nbt.sleeping",
            "abilities.flying,1" => "cem.nbt.flying",
            "abilities.flying,0" => "cem.nbt.not_flying",
            _ => "",
        };
        let value = if key.is_empty() {
            0.0
        } else {
            context.variables.get(key).copied().unwrap_or(0.0)
        };
        output.push(if value != 0.0 { '1' } else { '0' });
        cursor = close + 1;
    }
    output.push_str(&source[cursor..]);
    output
}

pub fn evaluate_expression(source: &str, context: &ExpressionContext) -> Result<f64, String> {
    if source.len() > MAX_EXPRESSION_BYTES {
        return Err(format!("expression exceeds {MAX_EXPRESSION_BYTES} bytes"));
    }
    let expanded = expand_nbt_predicates(source, context);
    let tokens = tokenize_expression(&expanded)?;
    let mut parser = ExpressionParser { tokens, cursor: 0, context };
    let value = parser.parse_expression(0)?;
    if parser.cursor != parser.tokens.len() {
        return Err(format!("unexpected token in expression: {:?}", parser.tokens[parser.cursor]));
    }
    if !value.is_finite() {
        return Err("expression produced a non-finite value".into());
    }
    Ok(value)
}

#[derive(Clone, Debug, PartialEq)]
enum ExprToken {
    Number(f64),
    Ident(String),
    Op(String),
    LeftParen,
    RightParen,
    Comma,
    Question,
    Colon,
}

fn tokenize_expression(source: &str) -> Result<Vec<ExprToken>, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() { i += 1; continue; }
        if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(char::is_ascii_digit)) {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') { i += 1; }
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                i += 1;
                if i < chars.len() && (chars[i] == '+' || chars[i] == '-') { i += 1; }
                while i < chars.len() && chars[i].is_ascii_digit() { i += 1; }
            }
            let text: String = chars[start..i].iter().copied().collect();
            let value = text.parse::<f64>().map_err(|_| format!("invalid number: {text}"))?;
            out.push(ExprToken::Number(value));
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            i += 1;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric()
                    || chars[i] == '_'
                    || chars[i] == '.'
                    || chars[i] == ':')
            {
                i += 1;
            }
            out.push(ExprToken::Ident(chars[start..i].iter().copied().collect()));
            continue;
        }
        match c {
            '(' => { out.push(ExprToken::LeftParen); i += 1; }
            ')' => { out.push(ExprToken::RightParen); i += 1; }
            ',' => { out.push(ExprToken::Comma); i += 1; }
            '?' => { out.push(ExprToken::Question); i += 1; }
            ':' => { out.push(ExprToken::Colon); i += 1; }
            _ => {
                let two: String = chars[i..(i + 2).min(chars.len())].iter().copied().collect();
                if ["&&", "||", "==", "!=", "<=", ">="].contains(&two.as_str()) {
                    out.push(ExprToken::Op(two)); i += 2;
                } else if "+-*/%^!<>".contains(c) {
                    out.push(ExprToken::Op(c.to_string())); i += 1;
                } else {
                    return Err(format!("unsupported character in expression: {c}"));
                }
            }
        }
        if out.len() > 4096 { return Err("expression exceeds token limit".into()); }
    }
    Ok(out)
}

struct ExpressionParser<'a> {
    tokens: Vec<ExprToken>,
    cursor: usize,
    context: &'a ExpressionContext,
}

impl ExpressionParser<'_> {
    fn parse_expression(&mut self, min_bp: u8) -> Result<f64, String> {
        let token = self.tokens.get(self.cursor).cloned().ok_or("expected expression")?;
        self.cursor += 1;
        let mut lhs = match token {
            ExprToken::Number(v) => v,
            ExprToken::Ident(name) => {
                if self.peek() == Some(&ExprToken::LeftParen) {
                    self.cursor += 1;
                    let mut args = Vec::new();
                    if self.peek() != Some(&ExprToken::RightParen) {
                        loop {
                            args.push(self.parse_expression(0)?);
                            if self.peek() == Some(&ExprToken::Comma) { self.cursor += 1; continue; }
                            break;
                        }
                    }
                    self.expect(ExprToken::RightParen)?;
                    apply_function(&name, &args, self.context)?
                } else {
                    match name.as_str() {
                        "pi" | "PI" => std::f64::consts::PI,
                        "true" => 1.0,
                        "false" => 0.0,
                        _ => match self.context.variables.get(&name) {
                            Some(value) => *value,
                            // OptiFine CEM user variables start at zero until their first assignment.
                            None if name.starts_with("var.") || name.starts_with("varb.") => 0.0,
                            None => return Err(format!("unknown CEM variable: {name}")),
                        },
                    }
                }
            }
            ExprToken::Op(op) if op == "-" => -self.parse_expression(13)?,
            ExprToken::Op(op) if op == "+" => self.parse_expression(13)?,
            ExprToken::Op(op) if op == "!" => bool_num(self.parse_expression(13)? == 0.0),
            ExprToken::LeftParen => {
                let value = self.parse_expression(0)?;
                self.expect(ExprToken::RightParen)?;
                value
            }
            _ => return Err("expected number, variable, function, or parenthesized expression".into()),
        };

        loop {
            if self.peek() == Some(&ExprToken::Question) && min_bp <= 1 {
                self.cursor += 1;
                let yes = self.parse_expression(0)?;
                self.expect(ExprToken::Colon)?;
                let no = self.parse_expression(1)?;
                lhs = if lhs != 0.0 { yes } else { no };
                continue;
            }
            let Some(ExprToken::Op(op)) = self.peek().cloned() else { break; };
            let Some((left_bp, right_bp)) = binding_power(&op) else { break; };
            if left_bp < min_bp { break; }
            self.cursor += 1;
            let rhs = self.parse_expression(right_bp)?;
            lhs = match op.as_str() {
                "+" => lhs + rhs, "-" => lhs - rhs, "*" => lhs * rhs,
                "/" => { if rhs == 0.0 { return Err("division by zero".into()); } lhs / rhs },
                "%" => { if rhs == 0.0 { return Err("modulo by zero".into()); } lhs % rhs },
                "^" => lhs.powf(rhs),
                "<" => bool_num(lhs < rhs), "<=" => bool_num(lhs <= rhs),
                ">" => bool_num(lhs > rhs), ">=" => bool_num(lhs >= rhs),
                "==" => bool_num(lhs == rhs), "!=" => bool_num(lhs != rhs),
                "&&" => bool_num(lhs != 0.0 && rhs != 0.0),
                "||" => bool_num(lhs != 0.0 || rhs != 0.0),
                _ => return Err(format!("unsupported operator: {op}")),
            };
        }
        Ok(lhs)
    }

    fn peek(&self) -> Option<&ExprToken> { self.tokens.get(self.cursor) }

    fn expect(&mut self, expected: ExprToken) -> Result<(), String> {
        if self.peek() == Some(&expected) { self.cursor += 1; Ok(()) }
        else { Err(format!("expected {expected:?}, found {:?}", self.peek())) }
    }
}

fn binding_power(op: &str) -> Option<(u8, u8)> {
    Some(match op {
        "||" => (2, 3), "&&" => (4, 5),
        "==" | "!=" | "<" | "<=" | ">" | ">=" => (6, 7),
        "+" | "-" => (8, 9), "*" | "/" | "%" => (10, 11),
        "^" => (12, 12), _ => return None,
    })
}
fn bool_num(value: bool) -> f64 { if value { 1.0 } else { 0.0 } }

fn apply_function(
    name: &str,
    args: &[f64],
    context: &ExpressionContext,
) -> Result<f64, String> {
    let unary = |f: fn(f64) -> f64| -> Result<f64, String> {
        if args.len() == 1 { Ok(f(args[0])) } else { Err(format!("{name} expects 1 argument")) }
    };
    let binary = |f: fn(f64, f64) -> f64| -> Result<f64, String> {
        if args.len() == 2 { Ok(f(args[0], args[1])) } else { Err(format!("{name} expects 2 arguments")) }
    };
    match name {
        "sin" => unary(f64::sin),
        "cos" => unary(f64::cos),
        "tan" => unary(f64::tan),
        "asin" => unary(f64::asin),
        "acos" => unary(f64::acos),
        "atan" => unary(f64::atan),
        "sqrt" => unary(f64::sqrt),
        "abs" => unary(f64::abs),
        "floor" => unary(f64::floor),
        "ceil" => unary(f64::ceil),
        "round" => unary(f64::round),
        "exp" => unary(f64::exp),
        "ln" | "log" => unary(f64::ln),
        "frac" => unary(|x| x - x.floor()),
        "signum" => unary(f64::signum),
        "atan2" => binary(f64::atan2),
        "pow" => binary(f64::powf),
        "min" if args.len() >= 2 => Ok(args.iter().copied().fold(f64::INFINITY, f64::min)),
        "max" if args.len() >= 2 => Ok(args.iter().copied().fold(f64::NEG_INFINITY, f64::max)),
        "min" | "max" => Err(format!("{name} expects at least 2 arguments")),
        "fmod" => binary(|a, b| {
            let remainder = a % b;
            if remainder != 0.0 && remainder.is_sign_negative() != b.is_sign_negative() {
                remainder + b
            } else {
                remainder
            }
        }),
        "torad" => unary(f64::to_radians),
        "todeg" => unary(f64::to_degrees),
        // OptiFine CEM wraps angles to the signed-radian interval. Fresh Animations
        // uses this to avoid discontinuities when the player's head crosses +/-PI.
        "wraprad" => unary(|angle| {
            let tau = 2.0 * std::f64::consts::PI;
            (angle + std::f64::consts::PI).rem_euclid(tau) - std::f64::consts::PI
        }),
        "random" => {
            if args.len() > 1 { return Err("random expects 0 or 1 argument".into()); }
            let seed = args.first().copied().or_else(|| context.variables.get("time").copied()).unwrap_or(0.0);
            let value = (seed * 12.9898 + 78.233).sin() * 43_758.5453;
            Ok(value - value.floor())
        }
        "if" | "ifb" => {
            if args.len() >= 3 && args.len() % 2 == 1 {
                for pair in args[..args.len() - 1].chunks_exact(2) {
                    if pair[0] != 0.0 { return Ok(pair[1]); }
                }
                Ok(*args.last().unwrap())
            } else {
                Err(format!("{name} expects condition/value pairs and a final fallback"))
            }
        }
        "between" => {
            if args.len() == 3 { Ok(bool_num(args[0] >= args[1] && args[0] <= args[2])) }
            else { Err("between expects 3 arguments".into()) }
        }
        "equals" => {
            if args.len() == 3 { Ok(bool_num((args[0] - args[1]).abs() <= args[2].abs())) }
            else { Err("equals expects 3 arguments".into()) }
        }
        "in" => {
            if args.len() >= 2 { Ok(bool_num(args[1..].iter().any(|value| *value == args[0]))) }
            else { Err("in expects at least 2 arguments".into()) }
        }
        "clamp" => {
            if args.len() != 3 { return Err("clamp expects 3 arguments".into()); }
            if args[1] > args[2] { return Err("clamp minimum exceeds maximum".into()); }
            Ok(args[0].clamp(args[1], args[2]))
        }
        "lerp" => {
            if args.len() == 3 {
                // OptiFine order: lerp(amount, start, end).
                Ok(args[1] + (args[2] - args[1]) * args[0])
            } else {
                Err("lerp expects 3 arguments".into())
            }
        }
        _ => Err(format!("unsupported CEM function: {name}")),
    }
}

#[cfg(test)]
mod expression_tests {
    use super::*;

    fn context() -> ExpressionContext {
        ExpressionContext { variables: [("limb_swing".into(), 2.0), ("age".into(), 10.0)].into() }
    }

    #[test]
    fn evaluates_precedence_functions_and_variables() {
        let c = context();
        assert_eq!(evaluate_expression("sin(pi / 2) + limb_swing * 3", &c).unwrap(), 7.0);
        assert_eq!(evaluate_expression("clamp(age, 0, 5)", &c).unwrap(), 5.0);
        assert_eq!(evaluate_expression("lerp(0.5, 2, 6)", &c).unwrap(), 4.0);
        assert_eq!(evaluate_expression("frac(2.75)", &c).unwrap(), 0.75);
        assert_eq!(evaluate_expression("min(5, 2, 7)", &c).unwrap(), 2.0);
        assert_eq!(evaluate_expression("between(3, 1, 4)", &c).unwrap(), 1.0);
        assert!((evaluate_expression("wraprad(3.5)", &c).unwrap() + 2.783185307179586).abs() < 1e-12);
        assert!((evaluate_expression("wraprad(-3.5)", &c).unwrap() - 2.783185307179586).abs() < 1e-12);
        assert_eq!(evaluate_expression("var.counter + 1", &c).unwrap(), 1.0);
        assert_eq!(evaluate_expression("varb.flag ? 1 : 0", &c).unwrap(), 0.0);
        let mut hierarchical = c.clone();
        hierarchical.variables.insert("head:hand.rx".into(), 0.75);
        assert_eq!(
            evaluate_expression("head:hand.rx + 0.25", &hierarchical).unwrap(),
            1.0
        );
    }

    #[test]
    fn evaluates_conditions_and_ternary() {
        let c = context();
        assert_eq!(evaluate_expression("if(age > 5, 1, 0)", &c).unwrap(), 1.0);
        assert_eq!(evaluate_expression("age > 5 ? 4 : 9", &c).unwrap(), 4.0);
        assert_eq!(evaluate_expression("!(age < 5) && true", &c).unwrap(), 1.0);
    }

    #[test]
    fn rejects_unknown_inputs_and_division_by_zero() {
        let c = context();
        assert!(evaluate_expression("unknown_var + 1", &c).is_err());
        assert!(evaluate_expression("4 / 0", &c).is_err());
        assert!(evaluate_expression("nbt('foo')", &c).is_err());
    }
}


/// One evaluated CEM transform for a named model part.
/// Values use Horizon's Part pose layout: translation in model pixels,
/// rotations in radians, and scale as multipliers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CemPartPose {
    pub pose: [f32; 9],
    pub assigned: [bool; 9],
}

/// Evaluate expression bindings whose keys use part.rx, part.ty, etc.
/// Supported suffixes are rx/ry/rz, tx/ty/tz and sx/sy/sz. The caller supplies
/// the part's base pose so fields not animated by the pack remain unchanged.
pub fn evaluate_pose_expressions(
    bindings: &std::collections::BTreeMap<String, String>,
    base_poses: &std::collections::BTreeMap<String, [f32; 9]>,
    context: &ExpressionContext,
) -> Result<std::collections::BTreeMap<String, CemPartPose>, String> {
    use std::collections::BTreeMap;
    let mut result = BTreeMap::new();
    for (binding, expression) in bindings {
        if binding.len() > 256 {
            return Err("CEM pose binding name exceeds 256 bytes".into());
        }
        let (part, channel) = binding.rsplit_once('.')
            .ok_or_else(|| format!("invalid CEM pose binding: {binding}"))?;
        if part.is_empty() || part.contains('/') || part.contains('\\') {
            return Err(format!("invalid CEM part name in binding: {binding}"));
        }
        let index = match channel {
            "tx" => 0, "ty" => 1, "tz" => 2,
            "rx" => 3, "ry" => 4, "rz" => 5,
            "sx" => 6, "sy" => 7, "sz" => 8,
            _ => return Err(format!("unsupported CEM pose channel: {channel}")),
        };
        let base = *base_poses.get(part)
            .ok_or_else(|| format!("CEM animation references missing part: {part}"))?;
        let value = evaluate_expression(expression, context)?;
        if value.abs() > 1_000_000.0 {
            return Err(format!("CEM pose value out of safe range: {binding}"));
        }
        let entry = result.entry(part.to_owned()).or_insert_with(|| CemPartPose {
            pose: base,
            assigned: [false; 9],
        });
        entry.pose[index] = value as f32;
        entry.assigned[index] = true;
    }
    Ok(result)
}


#[cfg(test)]
mod pose_binding_tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn applies_named_pose_channels_without_overwriting_base_pose() {
        let bindings = BTreeMap::from([
            ("head.rx".into(), "sin(age)".into()),
            ("head.ty".into(), "3 + limb_swing".into()),
            ("head.sx".into(), "1.2".into()),
        ]);
        let base = BTreeMap::from([("head".into(), [1.0, 2.0, 3.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0])]);
        let ctx = ExpressionContext { variables: BTreeMap::from([
            ("age".into(), 0.0), ("limb_swing".into(), 2.0)
        ]) };
        let out = evaluate_pose_expressions(&bindings, &base, &ctx).unwrap();
        let head = out.get("head").unwrap();
        assert_eq!(head.pose[0], 1.0);
        assert_eq!(head.pose[1], 5.0);
        assert_eq!(head.pose[3], 0.0);
        assert_eq!(head.pose[6], 1.2);
        assert!(head.assigned[3] && head.assigned[1] && head.assigned[6]);
        assert!(!head.assigned[0]);
    }

    #[test]
    fn rejects_missing_parts_unknown_channels_and_extreme_values() {
        let base = BTreeMap::from([("head".into(), [0.0; 9])]);
        let ctx = ExpressionContext::default();
        assert!(evaluate_pose_expressions(&BTreeMap::from([
            ("body.rx".into(), "1".into())
        ]), &base, &ctx).is_err());
        assert!(evaluate_pose_expressions(&BTreeMap::from([
            ("head.visible".into(), "1".into())
        ]), &base, &ctx).is_err());
        assert!(evaluate_pose_expressions(&BTreeMap::from([
            ("head.tx".into(), "1000001".into())
        ]), &base, &ctx).is_err());
    }
}
