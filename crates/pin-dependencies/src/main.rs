use core::{
    LockDependencies, NpmLock, PackageJson, PackageManagerLock, PnpmLock, VersionedDependency,
    VersionedDependencyOrResolved, YarnLockV2,
};
use semver::Version;
use std::collections::HashMap;

type ResolveDependencyKey = fn(name: &str, version: &str) -> String;

#[derive(Debug)]
pub struct DependencyVersionResolver {
    pub locked_dependencies: LockDependencies,
    pub resolve_dependency_key: ResolveDependencyKey,
}

fn npm_resolver(npm_lock: NpmLock) -> DependencyVersionResolver {
    let resolve_dependency: ResolveDependencyKey = |name, _| name.to_string();
    let resolve_package: ResolveDependencyKey = |name, _| format!("node_modules/{}", name);

    match npm_lock {
        NpmLock::Version1(lock) => DependencyVersionResolver {
            locked_dependencies: lock.dependencies,
            resolve_dependency_key: resolve_dependency,
        },
        NpmLock::Version2(lock) => {
            if let Some(packages) = lock.packages {
                DependencyVersionResolver {
                    locked_dependencies: packages,
                    resolve_dependency_key: resolve_package,
                }
            } else {
                DependencyVersionResolver {
                    locked_dependencies: lock.dependencies,
                    resolve_dependency_key: resolve_dependency,
                }
            }
        }
        NpmLock::Version3(lock) => DependencyVersionResolver {
            locked_dependencies: lock.packages,
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
                VersionedDependencyOrResolved::Versioned(VersionedDependency {
                    version: dependency.version,
                }),
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
                        VersionedDependencyOrResolved::Versioned(VersionedDependency { version }),
                    )
                })
                .collect()
        })
        .unwrap_or_else(HashMap::new)
}

fn transform_pnpm_v6_to_lock_dependencies(
    dependencies: Option<HashMap<String, VersionedDependency>>,
) -> LockDependencies {
    dependencies
        .map(|deps| {
            deps.into_iter()
                .map(|(key, dependency)| {
                    (key, VersionedDependencyOrResolved::Versioned(dependency))
                })
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
    version: String,
    pinned_version: String,
}

fn compute_versions_to_pin(
    package_json: &PackageJson,
    resolver: &DependencyVersionResolver,
) -> Vec<VersionToPin> {
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
                println!(
                    "Dependency {} is using a local path as version.",
                    dependency_name
                );
                continue;
            }

            let dependency_key = (resolver.resolve_dependency_key)(dependency_name, version);
            if let Some(locked_dependency) = resolver.locked_dependencies.get(&dependency_key) {
                match locked_dependency {
                    VersionedDependencyOrResolved::Resolved(resolved_dependency) => {
                        if let Some(resolved_key) = &resolved_dependency.resolved {
                            println!(
                                "Dependency {} resolved using {}.",
                                dependency_name, resolved_key
                            );
                            if let Some(resolved_dep) =
                                resolver.locked_dependencies.get(resolved_key)
                            {
                                if let VersionedDependencyOrResolved::Versioned(versioned_dep) =
                                    resolved_dep
                                {
                                    if Version::parse(version).is_err()
                                        && &versioned_dep.version != version
                                    {
                                        println!(
                                            "Dependency {} version is not pinned: {} -> {}.",
                                            dependency_name, version, versioned_dep.version
                                        );

                                        result.push(VersionToPin {
                                            dependency: dependency_name.clone(),
                                            version: version.clone(),
                                            pinned_version: versioned_dep.version.clone(),
                                        });
                                    } else {
                                        println!(
                                            "Dependency {} version is already pinned.",
                                            dependency_name
                                        );
                                    }
                                } else {
                                    println!("Dependency {} version is undefined.", resolved_key);
                                }
                            } else {
                                println!(
                                    "Dependency {} is unresolved in dependency.",
                                    resolved_key
                                );
                            }
                        }
                    }
                    VersionedDependencyOrResolved::Versioned(versioned_dep) => {
                        if Version::parse(version).is_err() && &versioned_dep.version != version {
                            println!(
                                "Dependency {} version is not pinned: {} -> {}.",
                                dependency_name, version, versioned_dep.version
                            );

                            result.push(VersionToPin {
                                dependency: dependency_name.clone(),
                                version: version.clone(),
                                pinned_version: versioned_dep.version.clone(),
                            });
                        } else {
                            println!("Dependency {} version is already pinned.", dependency_name);
                        }
                    }
                }
            } else {
                println!(
                    "Dependency {} is unresolved in dependencies.",
                    dependency_name
                );
            }
        }
    }

    result
}

fn main() {
    let package = finder::get_package().unwrap();
    let package_lock = finder::get_most_recently_modified_lock().unwrap();
    let parsed_package = parser::parse_package(&package).unwrap();

    println!("Package content: {:?}", parsed_package);

    let parsed_lock_package = parser::parse_lock(&package_lock).unwrap();

    println!("Lock content: {:?}", parsed_lock_package);

    let resolver = match parsed_lock_package {
        PackageManagerLock::Npm(npm_lock) => npm_resolver(npm_lock),
        PackageManagerLock::Yarn(yarn_lock) => yarn_resolver(yarn_lock),
        PackageManagerLock::Pnpm(pnpm_lock) => pnpm_resolver(pnpm_lock),
    };

    let versions_to_pin = compute_versions_to_pin(&parsed_package, &resolver);

    println!("Versions to pin: {:?}", versions_to_pin);
}
