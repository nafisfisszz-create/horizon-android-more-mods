//! Extracts portable Fabric JAR resources into a data-only Horizon HMOD.
//! Java classes and loader metadata are never copied or executed.
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Read,
    path::Path,
};

const MAX_FILES: usize = horizon_mods::content::MAX_FILES;
const MAX_FILE: u64 = horizon_mods::content::MAX_FILE;
const MAX_TOTAL: u64 = horizon_mods::content::MAX_TOTAL;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ExtractReport {
    pub files: usize,
    pub bytes: u64,
    pub skipped: Vec<String>,
    pub namespaces: Vec<String>,
}

/// Extracts assets/ and data/ into a content-only HMOD at `output`.
/// It deliberately does not port entrypoints, Mixins, classes or Java behavior.
pub fn extract_to_hmod(jar: &Path, output: &Path) -> Result<ExtractReport, String> {
    if output.exists() {
        return Err(format!("{} already exists; refusing to overwrite", output.display()));
    }

    let metadata = crate::fabric::read_jar(jar)?;
    let id = metadata.id.replace('-', "_");
    let file = fs::File::open(jar).map_err(|e| format!("{}: {e}", jar.display()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut namespaces = BTreeSet::new();
    let mut seen = BTreeSet::new();
    let mut skipped = Vec::new();
    let mut total = 0u64;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let name = entry
            .name()
            .map_err(|e| format!("invalid ZIP entry name: {e}"))?
            .into_owned();
        if !(name.starts_with("assets/") || name.starts_with("data/")) {
            continue;
        }
        if let Err(error) = horizon_mods::content::check_path(&name) {
            skipped.push(format!("{name}: {error}"));
            continue;
        }
        let parts: Vec<&str> = name.split('/').collect();
        let namespace = parts[1];
        // Vanilla and convention namespaces are only valid for tag additions.
        let permitted_shared_tag = name.starts_with("data/minecraft/tags/")
            || name.starts_with("data/c/tags/");
        if matches!(namespace, "horizon") || (matches!(namespace, "minecraft" | "c") && !permitted_shared_tag) {
            skipped.push(format!("{name}: reserved namespace content is not portable as a normal mod asset"));
            continue;
        }
        if !seen.insert(name.clone()) {
            return Err(format!("duplicate ZIP entry path: {name}"));
        }
        let size = entry.size();
        if size > MAX_FILE {
            return Err(format!("{name}: exceeds Horizon's {} MiB per-file limit", MAX_FILE >> 20));
        }
        if files.len() >= MAX_FILES {
            return Err(format!("JAR exceeds Horizon's {MAX_FILES} content-file limit"));
        }
        if total.saturating_add(size) > MAX_TOTAL {
            return Err(format!("JAR content exceeds Horizon's {} MiB per-mod limit", MAX_TOTAL >> 20));
        }
        let mut bytes = Vec::with_capacity(size as usize);
        entry.take(MAX_FILE + 1).read_to_end(&mut bytes).map_err(|e| format!("{name}: {e}"))?;
        if bytes.len() as u64 != size {
            return Err(format!("{name}: ZIP entry size mismatch or truncated content"));
        }
        total += size;
        if !matches!(namespace, "minecraft" | "c") && namespace != id {
            namespaces.insert(namespace.to_owned());
        }
        files.push((name, bytes));
    }

    if files.is_empty() {
        return Err("no portable assets/ or data/ files found in this JAR".into());
    }

    let name = if metadata.name.trim().is_empty() { metadata.id.clone() } else { metadata.name.clone() };
    let version = if metadata.version.trim().is_empty() { "0.1.0".to_owned() } else { metadata.version.clone() };
    let toml_string = |value: &str| serde_json::to_string(value).expect("string serialization cannot fail");
    let namespace_list = namespaces.iter().map(|ns| toml_string(ns)).collect::<Vec<_>>().join(", ");
    let manifest = format!(
        "id = {}\nname = {}\nversion = {}\ndescription = {}\napi = \"0.2\"\nside = \"both\"\ncapabilities = [\"registries\"]\nnamespaces = [{}]\n",
        toml_string(&id), toml_string(&name), toml_string(&version),
        toml_string("Extracted portable resources from a Fabric JAR; Java behavior is not included."),
        namespace_list
    );
    let borrowed: Vec<(&str, &[u8])> = files.iter().map(|(path, bytes)| (path.as_str(), bytes.as_slice())).collect();
    let hmod = horizon_mods::Package::write_files(&manifest, None, &borrowed)?;

    let mut target = OpenOptions::new().write(true).create_new(true).open(output)
        .map_err(|e| format!("{}: {e}", output.display()))?;
    use std::io::Write;
    if let Err(error) = target.write_all(&hmod) {
        let _ = fs::remove_file(output);
        return Err(format!("{}: {error}", output.display()));
    }

    Ok(ExtractReport {
        files: files.len(),
        bytes: total,
        skipped,
        namespaces: namespaces.into_iter().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("fabric-hmod-{}-{id}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn extracts_only_resource_and_data_files_to_valid_content_package() {
        let dir = temp_dir();
        let jar = dir.join("sample.jar");
        let output = dir.join("sample.hmod");
        let mut zip = zip::ZipWriter::new(fs::File::create(&jar).unwrap());
        let entries = [
            ("fabric.mod.json", r#"{"id":"sample-mod","version":"1.2.0","name":"Sample"}"#),
            ("assets/sample-mod/lang/en_us.json", r#"{"item.sample":"Sample"}"#),
            ("assets/extra_namespace/textures/item/sample.png", "png"),
            ("data/sample-mod/tags/item/test.json", r#"{"values":[]}"#),
            ("com/example/Entrypoint.class", "not copied"),
        ];
        for (name, content) in entries {
            zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(content.as_bytes()).unwrap();
        }
        zip.finish().unwrap();

        let report = extract_to_hmod(&jar, &output).unwrap();
        let package = horizon_mods::Package::read(&output).unwrap();
        assert_eq!(report.files, 3);
        assert_eq!(package.manifest.id, "sample_mod");
        assert_eq!(package.manifest.api, "0.2");
        assert!(package.wasm.is_none());
        // Validate the package through the same content loader Horizon uses,
        // not just the ZIP/manifest reader.
        let content = horizon_mods::Content::read(&output, &package.manifest).unwrap();
        assert_eq!(content.files.len(), 3);
        assert!(content.namespaces.contains(&"sample-mod".to_owned()));
        let mut archive = zip::ZipArchive::new(fs::File::open(&output).unwrap()).unwrap();
        assert!(archive.by_name("assets/sample-mod/lang/en_us.json").is_ok());
        assert!(archive.by_name("com/example/Entrypoint.class").is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn refuses_to_overwrite_existing_hmod() {
        let dir = temp_dir();
        let jar = dir.join("missing.jar");
        let output = dir.join("existing.hmod");
        fs::write(&output, b"keep").unwrap();
        let error = extract_to_hmod(&jar, &output).unwrap_err();
        assert!(error.contains("already exists"));
        assert_eq!(fs::read(&output).unwrap(), b"keep");
        let _ = fs::remove_dir_all(dir);
    }
}
