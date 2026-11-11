//! First-stage Fabric metadata reader and dependency analyzer.
//! This does not execute Java bytecode or apply Sponge Mixin transformations.
use serde::Deserialize;
use serde_json::Value;
use std::{collections::{BTreeMap, BTreeSet, VecDeque}, fs, io::Read, path::Path};

const METADATA: &str = "fabric.mod.json";
const MAX_METADATA: u64 = 1 << 20;
const PLATFORM_IDS: [&str; 3] = ["minecraft", "java", "fabricloader"];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    schema_version: Option<u32>,
    id: String,
    version: String,
    #[serde(default)] name: String,
    #[serde(default)] description: String,
    environment: Option<String>,
    #[serde(default)] depends: BTreeMap<String, Value>,
    #[serde(default)] recommends: BTreeMap<String, Value>,
    #[serde(default)] suggests: BTreeMap<String, Value>,
    #[serde(default)] breaks: BTreeMap<String, Value>,
    #[serde(default)] conflicts: BTreeMap<String, Value>,
    #[serde(default)] provides: Vec<String>,
    #[serde(default)] entrypoints: BTreeMap<String, Value>,
    #[serde(default)] mixins: Vec<Value>,
    access_widener: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dependency {
    pub mod_id: String,
    /// Fabric version predicates, preserved verbatim. Evaluation is a later milestone.
    pub requirements: Vec<String>,
}

fn dependencies(field: &str, map: BTreeMap<String, Value>) -> Result<Vec<Dependency>, String> {
    map.into_iter().map(|(mod_id, value)| {
        let requirements = match value {
            Value::String(s) => vec![s],
            Value::Array(values) => values.into_iter().map(|v| {
                v.as_str().map(str::to_owned).ok_or_else(|| format!("{METADATA}: {field}.{mod_id} must contain strings"))
            }).collect::<Result<Vec<_>, _>>()?,
            _ => return Err(format!("{METADATA}: {field}.{mod_id} must be a string or string array")),
        };
        if requirements.is_empty() {
            return Err(format!("{METADATA}: {field}.{mod_id} has no version requirement"));
        }
        Ok(Dependency { mod_id, requirements })
    }).collect()
}

fn entrypoint_classes(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => out.push(s.clone()),
        Value::Array(items) => items.iter().for_each(|v| entrypoint_classes(v, out)),
        Value::Object(object) => {
            if let Some(s) = object.get("value").and_then(Value::as_str) {
                out.push(s.to_owned());
            }
        }
        _ => {}
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FabricMetadata {
    pub schema_version: Option<u32>,
    pub id: String,
    pub version: String,
    pub name: String,
    pub description: String,
    pub environment: String,
    pub depends: Vec<Dependency>,
    pub recommends: Vec<Dependency>,
    pub suggests: Vec<Dependency>,
    pub breaks: Vec<Dependency>,
    pub conflicts: Vec<Dependency>,
    pub provides: Vec<String>,
    pub entrypoints: BTreeMap<String, Vec<String>>,
    pub mixins: Vec<String>,
    pub access_widener: Option<String>,
}

impl FabricMetadata {
    pub fn parse(text: &str) -> Result<Self, String> {
        let raw: Raw = serde_json::from_str(text).map_err(|e| format!("{METADATA}: {e}"))?;
        let valid_id = (1..=64).contains(&raw.id.len()) && raw.id.bytes().all(|b|
            b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
        if !valid_id {
            return Err(format!("{METADATA}: invalid mod id {:?}", raw.id));
        }
        if raw.version.trim().is_empty() {
            return Err(format!("{METADATA}: version must not be empty"));
        }
        let environment = raw.environment.unwrap_or_else(|| "*".into());
        if !matches!(environment.as_str(), "*" | "client" | "server") {
            return Err(format!("{METADATA}: unsupported environment {environment:?}"));
        }

        let mut entrypoints = BTreeMap::new();
        for (key, value) in raw.entrypoints {
            let mut classes = Vec::new();
            entrypoint_classes(&value, &mut classes);
            if classes.is_empty() {
                return Err(format!("{METADATA}: entrypoints.{key} has no class names"));
            }
            entrypoints.insert(key, classes);
        }
        let mixins = raw.mixins.into_iter().map(|v| match v {
            Value::String(s) => Ok(s),
            Value::Object(o) => o.get("config").and_then(Value::as_str).map(str::to_owned)
                .ok_or_else(|| format!("{METADATA}: mixin object needs a string config")),
            _ => Err(format!("{METADATA}: mixin entries must be strings or objects")),
        }).collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            schema_version: raw.schema_version, id: raw.id, version: raw.version,
            name: raw.name, description: raw.description, environment,
            depends: dependencies("depends", raw.depends)?,
            recommends: dependencies("recommends", raw.recommends)?,
            suggests: dependencies("suggests", raw.suggests)?,
            breaks: dependencies("breaks", raw.breaks)?,
            conflicts: dependencies("conflicts", raw.conflicts)?,
            provides: raw.provides, entrypoints, mixins, access_widener: raw.access_widener,
        })
    }

    pub fn analyze(&self) -> ModAnalysis {
        let mut blockers = Vec::new();
        if !self.entrypoints.is_empty() {
            blockers.push("Java entrypoints need a Horizon-native implementation".into());
        }
        if !self.mixins.is_empty() {
            blockers.push("Mixin injections need mapping to Horizon-native hooks".into());
        }
        if self.access_widener.is_some() {
            blockers.push("Access widener rules target Java classes and need manual porting".into());
        }
        ModAnalysis {
            mod_id: self.id.clone(),
            has_java_entrypoints: !self.entrypoints.is_empty(),
            has_mixins: !self.mixins.is_empty(),
            has_access_widener: self.access_widener.is_some(),
            blockers,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModAnalysis {
    pub mod_id: String,
    pub has_java_entrypoints: bool,
    pub has_mixins: bool,
    pub has_access_widener: bool,
    pub blockers: Vec<String>,
}

/// Read fabric.mod.json only; never load or execute classes from the JAR.
pub fn read_jar(path: &Path) -> Result<FabricMetadata, String> {
    let file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut entry = zip.by_name(METADATA).map_err(|e| format!("{}: {METADATA}: {e}", path.display()))?;
    if entry.size() > MAX_METADATA {
        return Err(format!("{METADATA} exceeds {} KiB", MAX_METADATA / 1024));
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.take(MAX_METADATA + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_METADATA {
        return Err(format!("{METADATA} exceeds {} KiB", MAX_METADATA / 1024));
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| format!("{METADATA} is not UTF-8"))?;
    FabricMetadata::parse(text)
}

/// Checks IDs and produces dependency-first order. Version predicates are not
/// evaluated yet and platform requirements are reported for a later adapter.
pub fn resolve_dependencies(mods: &[FabricMetadata]) -> DependencyResolution {
    let mut ids = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    for m in mods {
        if !ids.insert(m.id.as_str()) { duplicates.insert(m.id.clone()); }
    }
    let mut missing = Vec::new();
    let mut platform = Vec::new();
    let mut conflicts = Vec::new();
    let mut edges: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut degree: BTreeMap<&str, usize> = mods.iter().map(|m| (m.id.as_str(), 0)).collect();

    for m in mods {
        for dep in &m.depends {
            if PLATFORM_IDS.contains(&dep.mod_id.as_str()) {
                platform.push((m.id.clone(), dep.clone()));
            } else if !ids.contains(dep.mod_id.as_str()) {
                missing.push((m.id.clone(), dep.clone()));
            } else if dep.mod_id != m.id && edges.entry(dep.mod_id.as_str()).or_default().insert(m.id.as_str()) {
                *degree.entry(m.id.as_str()).or_default() += 1;
            }
        }
        for dep in m.breaks.iter().chain(&m.conflicts) {
            if ids.contains(dep.mod_id.as_str()) { conflicts.push((m.id.clone(), dep.clone())); }
        }
    }

    let mut ready: VecDeque<&str> = degree.iter().filter_map(|(id, n)| (*n == 0).then_some(*id)).collect();
    let mut order = Vec::new();
    while let Some(id) = ready.pop_front() {
        order.push(id.to_owned());
        if let Some(next) = edges.get(id) {
            for dependent in next {
                if let Some(n) = degree.get_mut(dependent) {
                    *n -= 1;
                    if *n == 0 { ready.push_back(dependent); }
                }
            }
        }
    }
    let cyclic = degree.iter().filter_map(|(id, n)| (*n > 0).then_some((*id).to_owned())).collect();
    missing.sort_by(|a, b| (&a.0, &a.1.mod_id).cmp(&(&b.0, &b.1.mod_id)));
    conflicts.sort_by(|a, b| (&a.0, &a.1.mod_id).cmp(&(&b.0, &b.1.mod_id)));
    platform.sort_by(|a, b| (&a.0, &a.1.mod_id).cmp(&(&b.0, &b.1.mod_id)));
    DependencyResolution {
        load_order: order, missing, conflicts,
        duplicate_ids: duplicates.into_iter().collect(), cyclic,
        platform_requirements: platform, version_checks_deferred: true,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DependencyResolution {
    pub load_order: Vec<String>,
    pub missing: Vec<(String, Dependency)>,
    pub conflicts: Vec<(String, Dependency)>,
    pub duplicate_ids: Vec<String>,
    pub cyclic: Vec<String>,
    pub platform_requirements: Vec<(String, Dependency)>,
    pub version_checks_deferred: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entrypoints_mixins_and_dependency_predicates() {
        let text = r#"{"id":"sample_mod","version":"1.0","entrypoints":{"main":["a.Main",{"value":"b.Main","adapter":"default"}]},"mixins":["a.mixins.json",{"config":"a.client.mixins.json"}],"depends":{"library":["^2.0","3.x"]},"accessWidener":"a.accesswidener"}"#;
        let m = FabricMetadata::parse(text).unwrap();
        assert_eq!(m.entrypoints["main"], ["a.Main", "b.Main"]);
        assert_eq!(m.mixins, ["a.mixins.json", "a.client.mixins.json"]);
        assert_eq!(m.depends[0].requirements, ["^2.0", "3.x"]);
        let a = m.analyze();
        assert!(a.has_java_entrypoints && a.has_mixins && a.has_access_widener);
        assert_eq!(a.blockers.len(), 3);
    }

    #[test]
    fn rejects_invalid_metadata() {
        assert!(FabricMetadata::parse(r#"{"id":"Bad ID","version":"1"}"#).is_err());
        assert!(FabricMetadata::parse(r#"{"id":"ok","version":""}"#).is_err());
        assert!(FabricMetadata::parse(r#"{"id":"ok","version":"1","environment":"both"}"#).is_err());
        assert!(FabricMetadata::parse(r#"{"id":"ok","version":"1","depends":{"lib":true}}"#).is_err());
    }

    #[test]
    fn resolves_order_and_reports_platform_conflict_and_cycle() {
        let parse = |s: &str| FabricMetadata::parse(s).unwrap();
        let app = parse(r#"{"id":"app","version":"1","depends":{"library":"*","fabricloader":">=0.16"}}"#);
        let library = parse(r#"{"id":"library","version":"1"}"#);
        let other = parse(r#"{"id":"other","version":"1","breaks":{"app":"*"}} "#);
        let r = resolve_dependencies(&[app, library, other]);
        assert_eq!(r.load_order[0], "library");
        assert!(r.missing.is_empty());
        assert_eq!(r.conflicts.len(), 1);
        assert_eq!(r.platform_requirements.len(), 1);
        assert!(r.version_checks_deferred);
        let a = parse(r#"{"id":"a","version":"1","depends":{"b":"*"}} "#);
        let b = parse(r#"{"id":"b","version":"1","depends":{"a":"*"}} "#);
        assert_eq!(resolve_dependencies(&[a,b]).cyclic, ["a","b"]);
    }
}
