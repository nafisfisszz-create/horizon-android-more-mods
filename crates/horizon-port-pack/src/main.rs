//! \`horizon-port-pack\`: port a Java modpack to Horizon (\`docs/modpacks.md\`).
//!
//! Commands: import, inventory, fetch, carry, report, check, fabric-check,
//! fabric-scan, port and upgrade-datapack. See \`docs/modpacks.md\` and
//! \`docs/fabric-compat.md\` for details and compatibility boundaries.
use horizon_port_pack::{fetch, inventory::Record, pipeline, upgrade};
use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

const USAGE: &str = "usage: horizon-port-pack import <pack> <work> | inventory <work> [--offline] | fetch <work> [--from <folder>] [<mod id>...] | carry <work> [--resources <prepared>] [<mod id>...] | report <work> | check <jar> <out> | fabric-check <jar> | fabric-scan <folder> | fabric-bridge <jar> | fabric-bytecode <jar> | port <pack> <work> [--from <folder>] [--resources <prepared>] | upgrade-datapack <pack> (<out> | --world <world>)";

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let result = (|| {
        let from = option(&mut args, "--from")?;
        let resources = option(&mut args, "--resources")?;
        let world = option(&mut args, "--world")?;
        let offline = take_flag(&mut args, "--offline");
        let path = |i: usize| args.get(i).map(Path::new).ok_or(USAGE.to_owned());
        let rest = || args.get(2..).unwrap_or_default().to_vec();
        let log = &|line: String| println!("{line}");
        match args.first().map(String::as_str) {
            Some("import") => pipeline::import(path(1)?, path(2)?, log).map(drop),
            Some("inventory") => pipeline::inventory(path(1)?, offline, log).map(drop),
            Some("fetch") => {
                let work = path(1)?;
                let records: Vec<Record> = pipeline::read_json(&work.join("inventory.json"))?;
                let only = rest();
                let want = |r: &Record| {
                    if only.is_empty() { pipeline::portable(r) } else { only.contains(&r.key) }
                };
                fetch::fetch(work, &records, from.as_deref(), &want, log).map(drop)
            }
            Some("carry") => pipeline::carry(path(1)?, resources.as_deref(), &rest(), log).map(drop),
            Some("report") => {
                let work = path(1)?;
                pipeline::report(work, log)?;
                println!("{}", work.join("coverage.md").display());
                Ok(())
            }
            Some("check") => {
                let c = horizon_port_pack::check::check(path(1)?, path(2)?)?;
                println!("{}: {}. {}", c.mod_id, c.verdict.label(), c.summary);
                for p in &c.ports { println!("  ports: {p}"); }
                for s in &c.skipped { println!("  skipped: {s}"); }
                Ok(())
            }
            Some("fabric-check") => fabric_check(path(1)?),
            Some("fabric-scan") => fabric_scan(path(1)?),
            Some("fabric-bridge") => fabric_bridge(path(1)?),
            Some("fabric-bytecode") => fabric_bytecode(path(1)?),
            Some("port") => {
                let work = path(2)?;
                pipeline::import(path(1)?, work, log)?;
                pipeline::inventory(work, offline, log)?;
                pipeline::port(work, resources.as_deref(), from.as_deref(), log)?;
                println!("{}", work.join("coverage.md").display());
                Ok(())
            }
            Some("upgrade-datapack") => {
                let pack = path(1)?;
                let out = match &world {
                    Some(world) => {
                        let name = pack.file_stem().unwrap_or_default().to_string_lossy();
                        world.join("datapacks").join(format!("{name}-26.3"))
                    }
                    None => path(2)?.to_path_buf(),
                };
                let report = upgrade::upgrade_into(pack, &out)?;
                for line in report.lines() { println!("{line}"); }
                println!("{}", out.display());
                Ok(())
            }
            _ => Err(USAGE.to_owned()),
        }
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn fabric_check(jar: &Path) -> Result<(), String> {
    let metadata = horizon_port_pack::fabric::read_jar(jar)?;
    let analysis = metadata.analyze();
    println!("{} {} ({})", metadata.id, metadata.version, metadata.environment);
    if !metadata.name.is_empty() { println!("name: {}", metadata.name); }
    println!("dependencies: {}", metadata.depends.iter().map(|d| d.mod_id.as_str()).collect::<Vec<_>>().join(", "));
    println!("entrypoint groups: {}", metadata.entrypoints.len());
    println!("mixin configs: {}", metadata.mixins.len());
    println!("access widener: {}", if metadata.access_widener.is_some() { "yes" } else { "no" });
    if analysis.blockers.is_empty() {
        println!("compatibility: metadata-only mod; inspect content and runtime requirements next");
    } else {
        println!("compatibility: Java code requires porting");
        for blocker in analysis.blockers { println!("  blocker: {blocker}"); }
    }
    Ok(())
}



/// Lists symbolic Java/Fabric references from class constant pools without executing bytecode.
fn fabric_bytecode(jar: &Path) -> Result<(), String> {
    let scan = horizon_port_pack::fabric_bytecode::scan_jar(jar)?;
    println!("Class files scanned: {}", scan.classes_scanned);
    println!("Fabric API references: {}", scan.fabric_api_references.len());
    for value in &scan.fabric_api_references { println!("  {value}"); }
    println!("Fabric Loader API references: {}", scan.loader_api_references.len());
    for value in &scan.loader_api_references { println!("  {value}"); }
    println!("Mixin references: {}", scan.mixin_references.len());
    for value in &scan.mixin_references { println!("  {value}"); }
    println!("Minecraft references: {}", scan.minecraft_references.len());
    for value in &scan.minecraft_references { println!("  {value}"); }
    if !scan.malformed_classes.is_empty() {
        println!("Malformed or oversized classes: {}", scan.malformed_classes.len());
        for value in &scan.malformed_classes { println!("  {value}"); }
    }
    println!("NOTE: static constant-pool scan only; reflection, generated classes and runtime behavior may be missed.");
    Ok(())
}

/// Reports documented native-hook candidates for API modules a JAR declares.
fn fabric_bridge(jar: &Path) -> Result<(), String> {
    let metadata = horizon_port_pack::fabric::read_jar(jar)?;
    println!("{} {} — Fabric API bridge assessment", metadata.id, metadata.version);
    let assessments = horizon_port_pack::fabric_api::assess(&metadata);
    if assessments.is_empty() {
        println!("No recognized Fabric API module dependencies were declared.");
        println!("This does not prove the mod has no Fabric API usage; inspect its bytecode and transitive dependencies.");
        return Ok(());
    }
    for item in assessments {
        println!("{}: {} (Horizon hooks: {})", item.fabric_module, item.status, item.horizon_hooks);
        println!("  {}", item.notes);
    }
    println!("NOTE: mapping only. Java entrypoints, Mixins and bytecode still require a native port.");
    Ok(())
}

/// Scans JARs directly inside a folder; it never extracts or executes their contents.
fn fabric_scan(folder: &Path) -> Result<(), String> {
    let entries = std::fs::read_dir(folder)
        .map_err(|e| format!("{}: {e}", folder.display()))?;
    let mut jars = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("jar")) {
            jars.push(path);
        }
    }
    jars.sort();
    if jars.is_empty() {
        return Err(format!("no .jar files found directly inside {}", folder.display()));
    }

    let mut mods = Vec::new();
    let mut failures = Vec::new();
    for jar in &jars {
        match horizon_port_pack::fabric::read_jar(jar) {
            Ok(metadata) => {
                println!("FOUND {} {} — {}", metadata.id, metadata.version, jar.display());
                mods.push(metadata);
            }
            Err(error) => {
                eprintln!("SKIP {} — {error}", jar.display());
                failures.push(jar.display().to_string());
            }
        }
    }

    let resolution = horizon_port_pack::fabric::resolve_dependencies(&mods);
    println!("\nFabric metadata scan: {} JAR(s), {} valid mod(s), {} unreadable JAR(s)",
        jars.len(), mods.len(), failures.len());
    println!("Dependency order: {}", resolution.load_order.join(" -> "));
    for id in &resolution.duplicate_ids { println!("ERROR duplicate mod id: {id}"); }
    for (owner, dep) in &resolution.missing {
        println!("MISSING dependency: {owner} requires {} ({})", dep.mod_id, dep.requirements.join(" OR "));
    }
    for (owner, dep) in &resolution.conflicts {
        println!("CONFLICT: {owner} declares conflict with {}", dep.mod_id);
    }
    for id in &resolution.cyclic { println!("ERROR dependency cycle includes: {id}"); }
    for (owner, dep) in &resolution.platform_requirements {
        println!("PLATFORM requirement: {owner} requires {} ({}) — check against Horizon's version contract",
            dep.mod_id, dep.requirements.join(" OR "));
    }
    println!("NOTE: version predicates are not evaluated; this is metadata analysis, not Java mod execution.");
    Ok(())
}

/// Removes \`--name <value>\` from \`args\`.
fn option(args: &mut Vec<String>, name: &str) -> Result<Option<PathBuf>, String> {
    match args.iter().position(|a| a == name) {
        Some(i) if i + 1 < args.len() => {
            let value = args.remove(i + 1);
            args.remove(i);
            Ok(Some(PathBuf::from(value)))
        }
        Some(_) => Err(USAGE.to_owned()),
        None => Ok(None),
    }
}

fn take_flag(args: &mut Vec<String>, name: &str) -> bool {
    let before = args.len();
    args.retain(|a| a != name);
    args.len() != before
}
