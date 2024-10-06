mod finder;
mod parser;
mod types;

use clap::Parser;
use clap_verbosity_flag::Verbosity;
use comfy_table::{presets, Table};
use console::style;
use detect_indent::Indent;
use semver::Version;
use serde::ser::Serialize;
use serde_json::ser::PrettyFormatter;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{Error, Write};
use std::path::PathBuf;
use std::string::ToString;
use tracing::{debug, info};
use tracing_log::AsTrace;
use types::{
    Engine, LockDependency, NpmDependencies, NpmLock, NpmLockEngines, ObjectEngines, PackageJson,
    PackageManagerLock, PnpmLock, VersionedDependencyOrResolved, YarnLockV2,
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::thread;
use anyhow::{anyhow, Result};

type ResolveDependencyKey = fn(name: &str, version: &str) -> String;
type LockDependencies = HashMap<String, LockDependency>;

#[derive(Debug)]
pub struct DependencyVersionResolver {
    pub locked_dependencies: LockDependencies,
    pub resolve_dependency_key: ResolveDependencyKey,
}

fn convert_array_to_object_engines(engines: Vec<String>) -> ObjectEngines {
    let mut object_engines = ObjectEngines::new();

    for engine_str in engines {
        let lowercase_engine_str = engine_str.to_lowercase();

        for engine_enum in [Engine::Node, Engine::Npm, Engine::Yarn] {
            let engine_str_lowercase = format!("{engine_enum:?}").to_lowercase();
            if lowercase_engine_str.contains(&engine_str_lowercase) {
                let value = engine_str.trim_start_matches(&engine_str_lowercase).trim();

                object_engines.insert(engine_enum, value.to_owned());
                break;
            }
        }
    }

    object_engines
}

fn convert_npm_engines_to_object_engines(engines: Option<NpmLockEngines>) -> Option<ObjectEngines> {
    match engines {
        Some(NpmLockEngines::Object(object_engines)) => Some(object_engines),
        Some(NpmLockEngines::Array(array_engines)) => {
            Some(convert_array_to_object_engines(array_engines))
        }
        _ => None,
    }
}

#[tracing::instrument]
fn convert_npm_to_lock_dependencies(npm_dependencies: NpmDependencies) -> LockDependencies {
    let mut lock_dependencies = LockDependencies::new();

    for (dependency_name, versioned_or_resolved) in npm_dependencies.clone() {
        if dependency_name.starts_with("node_modules/") {
            continue;
        }

        let lock_dependency = match versioned_or_resolved {
            VersionedDependencyOrResolved::Versioned(versioned_dependency) => LockDependency {
                version: versioned_dependency.version,
                engines: convert_npm_engines_to_object_engines(versioned_dependency.engines),
            },
            VersionedDependencyOrResolved::Resolved(resolved_dependency) => {
                if let Some(resolved_key) = &resolved_dependency.resolved {
                    debug!(
                        "Dependency {} resolved using {}.",
                        dependency_name, resolved_key
                    );
                    if let Some(resolved_dep) = npm_dependencies.get(resolved_key) {
                        if let VersionedDependencyOrResolved::Versioned(versioned_dep) =
                            resolved_dep
                        {
                            LockDependency {
                                version: versioned_dep.version.clone(),
                                engines: convert_npm_engines_to_object_engines(
                                    versioned_dep.engines.clone(),
                                ),
                            }
                        } else {
                            debug!("Dependency {} version is undefined.", resolved_key);
                            continue;
                        }
                    } else {
                        debug!("Dependency {} is unresolved in dependencies.", resolved_key);
                        continue;
                    }
                } else {
                    continue;
                }
            }
        };

        lock_dependencies.insert(dependency_name, lock_dependency);
    }

    lock_dependencies
}

#[tracing::instrument]
fn npm_resolver(npm_lock: NpmLock) -> DependencyVersionResolver {
    let resolve_dependency: ResolveDependencyKey = |name, _| name.to_string();
    let resolve_package: ResolveDependencyKey = |name, _| format!("node_modules/{name}");

    match npm_lock {
        NpmLock::Version1(lock) => DependencyVersionResolver {
            locked_dependencies: convert_npm_to_lock_dependencies(lock.dependencies),
            resolve_dependency_key: resolve_dependency,
        },
        NpmLock::Version2(lock) => {
            if let Some(packages) = lock.packages {
                DependencyVersionResolver {
                    locked_dependencies: convert_npm_to_lock_dependencies(packages),
                    resolve_dependency_key: resolve_package,
                }
            } else {
                DependencyVersionResolver {
                    locked_dependencies: convert_npm_to_lock_dependencies(lock.dependencies),
                    resolve_dependency_key: resolve_dependency,
                }
            }
        }
        NpmLock::Version3(lock) => DependencyVersionResolver {
            locked_dependencies: convert_npm_to_lock_dependencies(lock.packages),
            resolve_dependency_key: resolve_package,
        },
    }
}

fn transform_yarn_v2_to_lock_dependencies(yarn_lock: YarnLockV2) -> LockDependencies {
    yarn_lock
        .into_iter()
        .map(|(name, dependency)| {
            (
                name,
                LockDependency {
                    version: dependency.version,
                    engines: None,
                },
            )
        })
        .collect()
}

fn yarn_resolver(yarn_lock_file: YarnLockV2) -> DependencyVersionResolver {
    DependencyVersionResolver {
        locked_dependencies: transform_yarn_v2_to_lock_dependencies(yarn_lock_file),
        resolve_dependency_key: |name, version| format!("{name}@npm:{version}"),
    }
}

fn transform_pnpm_v5_to_lock_dependencies(
    dependencies: Option<HashMap<String, String>>,
) -> LockDependencies {
    dependencies.map_or_else(HashMap::new, |deps| {
        deps.into_iter()
            .map(|(key, version)| {
                (
                    key,
                    LockDependency {
                        version,
                        engines: None,
                    },
                )
            })
            .collect()
    })
}

fn transform_pnpm_v6_to_lock_dependencies(
    dependencies: Option<HashMap<String, LockDependency>>,
) -> LockDependencies {
    dependencies.map_or_else(HashMap::new, |deps| deps.into_iter().collect())
}

fn pnpm_resolver(pnpm_lock: PnpmLock) -> DependencyVersionResolver {
    let locked_dependencies: LockDependencies = match pnpm_lock {
        PnpmLock::Version6(lock) => {
            let importer = lock
                .importers
                .get(".")
                .cloned()
                .expect("Expect Pnpm to have resolved dependencies in current directory.");
            let dependencies = transform_pnpm_v6_to_lock_dependencies(importer.dependencies);
            let dev_dependencies =
                transform_pnpm_v6_to_lock_dependencies(importer.dev_dependencies);
            let optional_dependencies =
                transform_pnpm_v6_to_lock_dependencies(importer.optional_dependencies);

            [dependencies, dev_dependencies, optional_dependencies]
                .into_iter()
                .flatten()
                .collect()
        }
        PnpmLock::Version5(lock) => {
            let importer = lock
                .importers
                .get(".")
                .cloned()
                .expect("Expect Pnpm to have resolved dependencies in current directory.");
            let dependencies = transform_pnpm_v5_to_lock_dependencies(importer.dependencies);
            let dev_dependencies =
                transform_pnpm_v5_to_lock_dependencies(importer.dev_dependencies);
            let optional_dependencies =
                transform_pnpm_v5_to_lock_dependencies(importer.optional_dependencies);

            [dependencies, dev_dependencies, optional_dependencies]
                .into_iter()
                .flatten()
                .collect()
        }
    };

    DependencyVersionResolver {
        locked_dependencies,
        resolve_dependency_key: |name, _| name.to_string(),
    }
}

#[derive(Debug, Clone)]
struct VersionToPin {
    dependency: String,
    package_version: String,
    locked_version: String,
}

#[tracing::instrument(skip_all)]
fn compute_versions_to_pin(
    package_json: &PackageJson,
    resolver: &DependencyVersionResolver,
) -> Result<Vec<VersionToPin>, Error> {
    let mut result = Vec::new();
    let is_file_dependency = |name: &str| name.starts_with("file");
    let dependencies_per_type = vec![
        &package_json.dependencies,
        &package_json.dev_dependencies,
        &package_json.optional_dependencies,
    ];

    for dependencies in dependencies_per_type.into_iter().flatten() {
        for (dependency_name, version) in dependencies {
            if is_file_dependency(dependency_name) {
                debug!(
                    "Dependency {} is using a local path as version.",
                    dependency_name
                );
                continue;
            }

            let dependency_key = (resolver.resolve_dependency_key)(dependency_name, version);
            if let Some(locked_dependency) = resolver.locked_dependencies.get(&dependency_key) {
                if Version::parse(version).is_err() && &locked_dependency.version != version {
                    debug!(
                        "Dependency {} version is not pinned: {} -> {}.",
                        dependency_name, version, locked_dependency.version
                    );

                    result.push(VersionToPin {
                        dependency: dependency_name.clone(),
                        package_version: version.clone(),
                        locked_version: locked_dependency.version.clone(),
                    });
                } else {
                    debug!("Dependency {} version is already pinned.", dependency_name);
                }
            } else {
                debug!(
                    "Dependency {} is unresolved in dependencies.",
                    dependency_name
                );
            }
        }
    }

    Ok(result)
}

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(flatten)]
    verbose: Verbosity,
    #[arg(short, long, default_value_t = false)]
    update: bool,
}

fn run_task_with_progress<T, F>(
    index: usize,
    total: usize,
    icon: &str,
    title: &str,
    task: F,
    multi_progress: &MultiProgress,
) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    let prefix = format!("[{}/{}]", index, total);
    let pb = multi_progress.add(ProgressBar::new_spinner());
    pb.set_style(
        ProgressStyle::with_template("{prefix:.bold.dim} {spinner} {wide_msg}")
            .unwrap()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ "),
    );
    pb.set_prefix(prefix.clone());
    pb.set_message(format!("{} [STARTED] {}...", icon, title));

    let handle = thread::spawn(move || {
        let result = task();
        match &result {
            Ok(_) => {
                pb.finish_with_message(format!("{} [SUCCESS] {}!", icon, title));
            }
            Err(err) => {
                pb.finish_with_message(format!("{} [ERROR] {}: {}", icon, title, err));
            }
        }
        result
    });

    handle.join().unwrap()
}


fn write_pinned_versions(package_json: &mut Value, versions_to_pin: &Vec<VersionToPin>) {
    fn update_dependencies(dependencies: Option<&mut Value>, versions_to_pin: &Vec<VersionToPin>) {
        if let Some(dep_map) = dependencies {
            for version_to_pin in versions_to_pin {
                if let Some(locked_version) = dep_map.get_mut(&version_to_pin.dependency) {
                    *locked_version = Value::String(version_to_pin.clone().locked_version);
                }
            }
        }
    }

    update_dependencies(package_json.get_mut("dependencies"), versions_to_pin);
    update_dependencies(package_json.get_mut("dev_dependencies"), versions_to_pin);
    update_dependencies(
        package_json.get_mut("optional_dependencies"),
        versions_to_pin,
    );
}

fn write_json_to_file(path: &PathBuf, indent: &Indent, content: &Value) -> std::io::Result<()> {
    let mut buf = Vec::new();
    let formatter = PrettyFormatter::with_indent(indent.indent().as_bytes());
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    content
        .serialize(&mut ser)
        .expect("Failed to serialize JSON content");
    buf.push(b'\n');

    let mut file = OpenOptions::new().write(true).truncate(true).open(path)?;
    let _ = file.write_all(buf.as_ref());
    Ok(())
}

fn generate_update_command_from_args(args: &Args) -> String {
    let mut update_command = vec!["npd"];
    let mut hint = "-".to_string();

    if args.verbose.is_silent() {
        update_command.push("-q");
    } else {
        let level_value: i8 = match args.verbose.log_level() {
            None => -1,
            Some(log::Level::Error) => 0,
            Some(log::Level::Warn) => 1,
            Some(log::Level::Info) => 2,
            Some(log::Level::Debug) => 3,
            Some(log::Level::Trace) => 4,
        };

        if level_value > 0 {
            #[allow(clippy::cast_sign_loss)]
            hint.push_str(&("v".repeat(level_value as usize)));
            update_command.push(hint.as_str());
        }
    }

    update_command.push("-u");
    update_command.join(" ")
}


#[cfg(test)]
mod tests {
    use super::*;
    use clap_verbosity_flag::Verbosity;

    #[test]
    fn generate_update_command() {
        let tests = [
            // verbose, quiet, expected_command
            (0, 0, "npd -u"),
            (1, 0, "npd -v -u"),
            (2, 0, "npd -vv -u"),
            (3, 0, "npd -vvv -u"),
            (4, 0, "npd -vvvv -u"),
            (5, 0, "npd -vvvv -u"),
            (255, 0, "npd -vvvv -u"),
            (0, 1, "npd -q -u"),
            (0, 2, "npd -q -u"),
            (0, 255, "npd -q -u"),
            (255, 255, "npd -u"),
        ];

        for (verbose, quiet, expected_command) in &tests {
            let args = Args {
                verbose: Verbosity::new(*verbose, *quiet),
                update: false,
            };
            assert_eq!(generate_update_command_from_args(&args), *expected_command,  "verbose = {verbose}, quiet = {quiet}, expected = {expected_command}");
        }
    }
}

#[allow(clippy::too_many_lines)]
fn main() {
    let multi_progress = MultiProgress::new();
    let args = Args::parse();

    let format = tracing_subscriber::fmt::format()
        .with_level(true)
        .with_target(true)
        .with_timer(tracing_subscriber::fmt::time::time())
        .compact();

    tracing_subscriber::fmt()
        .with_max_level(args.verbose.log_level_filter().as_trace())
        .event_format(format)
        .init();

    let total_steps = if args.update { 7 } else { 6 };
    let package = run_task_with_progress(
        1,
        total_steps,
        "📦",
        "Resolving package.json",
        || finder::get_package().map_err(|e| e.into()),
        &multi_progress,
    )
        .expect("Unable to get package.json file in the current directory");

    let package_lock = run_task_with_progress(
        2,
        total_steps,
        "🔒",
        "Resolving lock file",
        || finder::get_most_recently_modified_lock().map_err(|e| e.into()),
        &multi_progress,
    )
        .expect("Unable to get the most recently modified lock file in the current directory");

    let (parsed_package, mut raw_package, indent) = run_task_with_progress(
        3,
        total_steps,
        "📦",
        "Parsing package.json",
        || parser::parse_package(&package).map_err(|e| e.into()),
        &multi_progress,
    )
        .expect("Unable to parse package.json file");

    let parsed_lock_package = run_task_with_progress(
        4,
        total_steps,
        "🔒",
        "Parsing lock file",
        || parser::parse_lock(&package_lock).map_err(|e| e.into()),
        &multi_progress,
    )
        .expect("Unable to parse lock file");

    let resolver = match parsed_lock_package {
        PackageManagerLock::Npm(npm_lock) => npm_resolver(npm_lock),
        PackageManagerLock::Yarn(yarn_lock) => yarn_resolver(yarn_lock),
        PackageManagerLock::Pnpm(pnpm_lock) => pnpm_resolver(pnpm_lock),
    };

    let versions_to_pin = run_task_with_progress(
        5,
        total_steps,
        "⚙️",
        "Computing dependency versions to pin",
        || compute_versions_to_pin(&parsed_package, &resolver).map_err(|e| e.into()),
        &multi_progress,
    )
        .unwrap();

    if args.verbose.is_silent() {
        return;
    }

    let mut table = Table::new();
    table.load_preset(presets::NOTHING);
    for version_to_pin in versions_to_pin.clone() {
        table.add_row(vec![
            version_to_pin.dependency + ":",
            version_to_pin.package_version,
            "→".to_string(),
            version_to_pin.locked_version,
        ]);
    }

    let total_steps_str = style(format!("[{}/{}]", 6, total_steps))
        .bold()
        .dim()
        .to_string();

    if table.is_empty() {
        info!(
            "{} [RESULTS] {}{}",
            total_steps_str,
            "All dependency versions are already pinned ",
            style(":)").green().to_string()
        );
        return;
    }

    info!(
        "{} [RESULTS] {}",
        total_steps_str,
        if args.update {
            "Dependency versions pinned"
        } else {
            "Dependency versions that can be pinned"
        }
    );

    for row in table.lines() {
        info!("{} [RESULTS] {}", total_steps_str, row.trim());
    }

    if !args.update {
        info!(
            "{} [RESULTS] {}",
            total_steps_str,
            format!(
                "Run {} to upgrade package.json.",
                style(generate_update_command_from_args(&args))
                    .bold()
                    .cyan()
            )
        );
        return;
    }

    write_pinned_versions(&mut raw_package, &versions_to_pin);
    run_task_with_progress(
        7,
        total_steps,
        "💾",
        "Updating package.json",
        || write_json_to_file(&package, &indent, &raw_package).map_err(|e| e.into()),
        &multi_progress,
    )
        .expect("Failed to update package.json content");
}
