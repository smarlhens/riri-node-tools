use clap::Parser;
use comfy_table::{presets, Table};
use console::style;
use definitely_typed::{
    Engine, LockDependencies, LockDependency, NpmDependencies, NpmLock, NpmLockEngines,
    ObjectEngines, PackageJson, PackageManagerLock, PnpmLock, VersionedDependencyOrResolved,
    YarnLockV2,
};
use semver::Version;
use std::collections::HashMap;
use std::io::Error;
use std::string::ToString;
use tracing::{debug, error, info, Level};

type ResolveDependencyKey = fn(name: &str, version: &str) -> String;

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
            let engine_str_lowercase = format!("{:?}", engine_enum).to_lowercase();
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
    let resolve_package: ResolveDependencyKey = |name, _| format!("node_modules/{}", name);

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
        resolve_dependency_key: |name, version| format!("{}@npm:{}", name, version),
    }
}

fn transform_pnpm_v5_to_lock_dependencies(
    dependencies: Option<HashMap<String, String>>,
) -> LockDependencies {
    dependencies
        .map(|deps| {
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
        .unwrap_or_else(HashMap::new)
}

fn transform_pnpm_v6_to_lock_dependencies(
    dependencies: Option<HashMap<String, LockDependency>>,
) -> LockDependencies {
    dependencies
        .map(|deps| {
            deps.into_iter()
                .map(|(key, dependency)| (key, dependency))
                .collect()
        })
        .unwrap_or_else(HashMap::new)
}

fn pnpm_resolver(pnpm_lock: PnpmLock) -> DependencyVersionResolver {
    let locked_dependencies: LockDependencies = match pnpm_lock {
        PnpmLock::Version6(lock) => {
            let importer = lock.importers.get(".").cloned().unwrap();
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
            let importer = lock.importers.get(".").cloned().unwrap();
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

#[derive(Debug)]
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

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = false)]
    quiet: bool,
    #[arg(short, long, default_value_t = false)]
    debug: bool,
}

macro_rules! trace_fn {
    ($index:expr, $total:expr, $icon:expr, $title:expr, $result:expr) => {{
        let prefix = style(format!("[{}/{}]", $index, $total,))
            .bold()
            .dim()
            .to_string();
        info!("{} [STARTED] {} {}...", prefix, $icon, $title);
        let result = $result;
        match &result {
            Ok(_) => {
                info!("{} [SUCCESS] {} {}!", prefix, $icon, $title);
            }
            Err(err) => {
                error!("{} [ERROR] {} {}: {}", prefix, $icon, $title, err);
            }
        }
        result
    }};
}

fn main() {
    let args = Args::parse();

    let format = tracing_subscriber::fmt::format()
        .with_level(true)
        .with_target(true)
        .with_timer(tracing_subscriber::fmt::time::time())
        .compact();

    let mut tracing_max_level = Level::INFO;
    if args.quiet {
        tracing_max_level = Level::ERROR
    }

    if args.debug {
        tracing_max_level = Level::DEBUG
    }

    tracing_subscriber::fmt()
        .with_max_level(tracing_max_level)
        .event_format(format)
        .init();

    let total_steps = 6;
    let package = trace_fn!(1, 5, "📦", "Resolving package.json", finder::get_package()).unwrap();
    let package_lock = trace_fn!(
        2,
        total_steps,
        "🔒",
        "Resolving lock file",
        finder::get_most_recently_modified_lock()
    )
    .unwrap();
    let parsed_package = trace_fn!(
        3,
        total_steps,
        "📦",
        "Parsing package.json",
        parser::parse_package(&package)
    )
    .unwrap();
    let parsed_lock_package = trace_fn!(
        4,
        total_steps,
        "🔒",
        "Parsing lock file",
        parser::parse_lock(&package_lock)
    )
    .unwrap();

    let resolver = match parsed_lock_package {
        PackageManagerLock::Npm(npm_lock) => npm_resolver(npm_lock),
        PackageManagerLock::Yarn(yarn_lock) => yarn_resolver(yarn_lock),
        PackageManagerLock::Pnpm(pnpm_lock) => pnpm_resolver(pnpm_lock),
    };

    let versions_to_pin = trace_fn!(
        5,
        total_steps,
        "⚙️",
        "Computing dependency versions to pin",
        compute_versions_to_pin(&parsed_package, &resolver)
    )
    .unwrap();

    if args.quiet {
        return;
    }

    let mut table = Table::new();
    table.load_preset(presets::NOTHING);
    for version_to_pin in versions_to_pin {
        table.add_row(vec![
            version_to_pin.dependency + ":",
            version_to_pin.package_version,
            "→".to_string(),
            version_to_pin.locked_version,
        ]);
    }

    for row in table.lines() {
        info!(
            "{} [RESULTS] {}",
            style(format!("[{}/{}]", 6, total_steps))
                .bold()
                .dim()
                .to_string(),
            row.trim()
        );
    }
}
