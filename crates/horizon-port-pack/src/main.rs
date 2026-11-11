//! \`horizon-port-pack\`: port a Java modpack to Horizon (\`docs/modpacks.md\`).
//!
//! - \`import <pack> <work>\`: a \`.mrpack\`, CurseForge zip or instance folder
//!   to \`<work>/pack.json\`.
//! - \`inventory <work> [--offline]\`: Modrinth metadata and jar inventory.
//! - \`fetch <work> [--from <folder>] [<mod id>...]\`: fetch mod jars.
//! - \`carry <work> [--resources <prepared>] [<mod id>...]\`: carry over data/assets.
//! - \`report <work>\`: write the coverage report.
//! - \`check <jar> <out>\`: inspect one mod jar for content carry-over.
//! - \`fabric-check <jar>\`: inspect Fabric metadata and report Java-only blockers.
//! - \`port <pack> <work> [--from <folder>] [--resources <prepared>]\`: run the port pipeline.
//! - \`upgrade-datapack <pack> <out> | --world <world>\`: upgrade a data pack.
//!
//! The work folder belongs to the user: jars and carried-over files stay
//! there and are never uploaded or committed.
use horizon_port_pack::{fetch, inventory::Record, pipeline, upgrade};
use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

const USAGE: &str = "usage: horizon-port-pack import <pack> <work> | inventory <work> [--offline] | fetch <work> [--from <folder>] [<mod id>...] | carry <work> [--resources <prepared>] [<mod id>...] | report <work> | check <jar> <out> | fabric-check <jar> | port <pack> <work> [--from <folder>] [--resources <prepared>] | upgrade-datapack <pack> (<out> | --world <world>)";

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
            Some("fabric-check") => {
                let metadata = horizon_port_pack::fabric::read_jar(path(1)?)?;
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
