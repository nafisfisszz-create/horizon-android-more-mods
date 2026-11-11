//! Safe, non-executing scan of JVM class constant pools in a Fabric JAR.
//! This finds symbolic references; it does not decompile or run Java code.

use std::{collections::BTreeSet, io::Read, path::Path};

const MAX_CLASS_BYTES: u64 = 16 * 1024 * 1024;
const PREFIXES: &[(&str, &str)] = &[
    ("net/fabricmc/fabric/api/", "fabric-api"),
    ("net/fabricmc/api/", "fabric-loader-api"),
    ("org/spongepowered/asm/mixin/", "mixin"),
    ("net/minecraft/", "minecraft"),
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BytecodeScan {
    pub classes_scanned: usize,
    pub fabric_api_references: Vec<String>,
    pub loader_api_references: Vec<String>,
    pub mixin_references: Vec<String>,
    pub minecraft_references: Vec<String>,
    pub malformed_classes: Vec<String>,
}

fn read_u16(bytes: &[u8], at: &mut usize) -> Result<u16, String> {
    let end = at.checked_add(2).ok_or("class offset overflow")?;
    let part = bytes.get(*at..end).ok_or("truncated class file")?;
    *at = end;
    Ok(u16::from_be_bytes([part[0], part[1]]))
}

fn read_u32(bytes: &[u8], at: &mut usize) -> Result<u32, String> {
    let end = at.checked_add(4).ok_or("class offset overflow")?;
    let part = bytes.get(*at..end).ok_or("truncated class file")?;
    *at = end;
    Ok(u32::from_be_bytes([part[0], part[1], part[2], part[3]]))
}

fn skip(bytes: &[u8], at: &mut usize, count: usize) -> Result<(), String> {
    let end = at.checked_add(count).ok_or("class offset overflow")?;
    if end > bytes.len() { return Err("truncated class file".into()); }
    *at = end;
    Ok(())
}

fn constant_pool_utf8(bytes: &[u8]) -> Result<Vec<String>, String> {
    if bytes.get(0..4) != Some(&[0xCA, 0xFE, 0xBA, 0xBE]) {
        return Err("invalid JVM class magic".into());
    }
    let mut at = 8usize; // magic + minor + major
    let count = read_u16(bytes, &mut at)? as usize;
    let mut strings = Vec::new();
    let mut index = 1usize;
    while index < count {
        let tag = *bytes.get(at).ok_or("truncated constant pool")?;
        at += 1;
        match tag {
            1 => {
                let len = read_u16(bytes, &mut at)? as usize;
                let end = at.checked_add(len).ok_or("constant pool offset overflow")?;
                let raw = bytes.get(at..end).ok_or("truncated UTF-8 constant")?;
                // JVM modified UTF-8 is ASCII-compatible for package names; retain
                // only entries that can be represented as standard UTF-8.
                if let Ok(value) = std::str::from_utf8(raw) {
                    strings.push(value.to_owned());
                }
                at = end;
            }
            3 | 4 => skip(bytes, &mut at, 4)?,
            5 | 6 => { skip(bytes, &mut at, 8)?; index += 1; }
            7 | 8 | 16 | 19 | 20 => skip(bytes, &mut at, 2)?,
            9 | 10 | 11 | 12 | 17 | 18 => skip(bytes, &mut at, 4)?,
            15 => skip(bytes, &mut at, 3)?,
            _ => return Err(format!("unknown constant-pool tag {tag}")),
        }
        index += 1;
    }
    Ok(strings)
}

/// Scan class files inside a JAR without loading or executing them.
pub fn scan_jar(path: &Path) -> Result<BytecodeScan, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut result = BytecodeScan::default();
    let mut buckets: [BTreeSet<String>; 4] = Default::default();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().map_err(|e| e.to_string())?.into_owned();
        if !name.ends_with(".class") { continue; }
        if entry.size() > MAX_CLASS_BYTES {
            result.malformed_classes.push(format!("{} (over 16 MiB)", entry.name()));
            continue;
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        if let Err(error) = entry.by_ref().take(MAX_CLASS_BYTES + 1).read_to_end(&mut bytes) {
            result.malformed_classes.push(format!("{name} ({error})"));
            continue;
        }
        if bytes.len() as u64 > MAX_CLASS_BYTES {
            result.malformed_classes.push(format!("{name} (over 16 MiB)"));
            continue;
        }
        match constant_pool_utf8(&bytes) {
            Ok(strings) => {
                result.classes_scanned += 1;
                for value in strings {
                    for (prefix, bucket) in PREFIXES {
                        if value.contains(prefix) {
                            let idx = match *bucket {
                                "fabric-api" => 0,
                                "fabric-loader-api" => 1,
                                "mixin" => 2,
                                _ => 3,
                            };
                            buckets[idx].insert(value.clone());
                        }
                    }
                }
            }
            Err(error) => result.malformed_classes.push(format!("{name} ({error})")),
        }
    }
    result.fabric_api_references = buckets[0].iter().cloned().collect();
    result.loader_api_references = buckets[1].iter().cloned().collect();
    result.mixin_references = buckets[2].iter().cloned().collect();
    result.minecraft_references = buckets[3].iter().cloned().collect();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_class_bytes() {
        assert!(constant_pool_utf8(b"not a class").is_err());
    }

    #[test]
    fn reads_empty_constant_pool_class_header() {
        let bytes = [0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61, 0, 1];
        assert!(constant_pool_utf8(&bytes).unwrap().is_empty());
    }
}
