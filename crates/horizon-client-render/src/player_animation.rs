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


/// Per-frame numeric inputs supplied by the player renderer / CEM variable layer.
#[derive(Clone, Debug, Default)]
pub struct ExpressionContext {
    pub variables: std::collections::BTreeMap<String, f64>,
}

/// Evaluate the arithmetic and conditional subset used by CEM animation expressions.
/// Unknown variables/functions fail closed instead of silently becoming zero.
pub fn evaluate_expression(source: &str, context: &ExpressionContext) -> Result<f64, String> {
    let tokens = tokenize_expression(source)?;
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
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '.') { i += 1; }
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
                    apply_function(&name, &args)?
                } else {
                    match name.as_str() {
                        "pi" | "PI" => std::f64::consts::PI,
                        "true" => 1.0,
                        "false" => 0.0,
                        _ => *self.context.variables.get(&name).ok_or_else(|| format!("unknown CEM variable: {name}"))?,
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

fn apply_function(name: &str, args: &[f64]) -> Result<f64, String> {
    let unary = |f: fn(f64) -> f64| -> Result<f64, String> {
        if args.len() == 1 { Ok(f(args[0])) } else { Err(format!("{name} expects 1 argument")) }
    };
    let binary = |f: fn(f64, f64) -> f64| -> Result<f64, String> {
        if args.len() == 2 { Ok(f(args[0], args[1])) } else { Err(format!("{name} expects 2 arguments")) }
    };
    match name {
        "sin" => unary(f64::sin), "cos" => unary(f64::cos), "tan" => unary(f64::tan),
        "asin" => unary(f64::asin), "acos" => unary(f64::acos), "atan" => unary(f64::atan),
        "sqrt" => unary(f64::sqrt), "abs" => unary(f64::abs), "floor" => unary(f64::floor),
        "ceil" => unary(f64::ceil), "round" => unary(f64::round), "exp" => unary(f64::exp),
        "ln" => unary(f64::ln), "signum" => unary(f64::signum),
        "atan2" => binary(f64::atan2), "pow" => binary(f64::powf), "min" => binary(f64::min),
        "max" => binary(f64::max),
        "if" => {
            if args.len() == 3 { Ok(if args[0] != 0.0 { args[1] } else { args[2] }) }
            else { Err("if expects 3 arguments".into()) }
        }
        "clamp" => {
            if args.len() == 3 { Ok(args[0].clamp(args[1], args[2])) }
            else { Err("clamp expects 3 arguments".into()) }
        }
        "lerp" => {
            if args.len() == 3 { Ok(args[0] + (args[1] - args[0]) * args[2]) }
            else { Err("lerp expects 3 arguments".into()) }
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
