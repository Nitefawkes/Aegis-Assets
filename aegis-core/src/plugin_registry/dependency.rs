//! Dependency resolution system for plugin packages
//!
//! Handles semantic versioning, dependency graph resolution, conflict detection,
//! and package management for plugin dependencies.

use anyhow::{Result, Context, bail, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::str::FromStr;
use tracing::{info, warn, debug, error};

use super::models::*;

/// Dependency resolution result
#[derive(Debug, Clone)]
pub struct ResolutionResult {
    /// Successfully resolved dependencies with their versions
    pub resolved: HashMap<String, ResolvedDependency>,
    /// Any conflicts that were encountered
    pub conflicts: Vec<DependencyConflict>,
    /// Dependencies that could not be resolved
    pub failures: Vec<DependencyFailure>,
}

/// Resolved dependency with version information
#[derive(Debug, Clone)]
pub struct ResolvedDependency {
    pub name: String,
    pub version: String,
    pub source: DependencySource,
    pub required_by: Vec<String>, // Plugins that require this dependency
}

/// Source of a dependency (registry, local, etc.)
#[derive(Debug, Clone)]
pub enum DependencySource {
    Registry,
    Local,
    Git(String),
    Path(String),
}

/// Dependency conflict information
#[derive(Debug, Clone)]
pub struct DependencyConflict {
    pub dependency: String,
    pub required_by: Vec<(String, String)>, // (plugin_name, version_requirement)
    pub resolution: ConflictResolution,
}

/// How a conflict was resolved
#[derive(Debug, Clone)]
pub enum ConflictResolution {
    /// Selected a specific version
    SelectedVersion(String),
    /// Failed to resolve conflict
    Failed(String),
    /// Backtracked and chose different version
    Backtracked,
}

/// Dependency resolution failure
#[derive(Debug, Clone)]
pub struct DependencyFailure {
    pub dependency: String,
    pub required_by: String,
    pub reason: String,
}

/// Version requirement (parsed from plugin.toml)
#[derive(Debug, Clone)]
pub struct VersionRequirement {
    pub raw: String,
    pub requirement: VersionReq,
}

/// Internal version requirement representation
#[derive(Debug, Clone)]
pub enum VersionReq {
    /// Exact version (1.0.0)
    Exact(String),
    /// Compatible release (^1.0.0)
    Compatible(String),
    /// Approximately equivalent (~1.0.0)
    Tilde(String),
    /// Greater than or equal (>=1.0.0)
    GreaterEqual(String),
    /// Less than (<1.0.0)
    Less(String),
    /// Greater than (>1.0.0)
    Greater(String),
    /// Wildcard (*)
    Wildcard,
    /// Any version (x.x.x)
    Any,
}

/// Semantic version representation
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemanticVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub pre_release: Option<String>,
    pub build: Option<String>,
}

impl fmt::Display for SemanticVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;

        if let Some(pre) = &self.pre_release {
            write!(f, "-{}", pre)?;
        }

        if let Some(build) = &self.build {
            write!(f, "+{}", build)?;
        }

        Ok(())
    }
}

impl FromStr for SemanticVersion {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let (main, pre, build) = Self::parse_version_string(s)?;

        Ok(SemanticVersion {
            major: main.0,
            minor: main.1,
            patch: main.2,
            pre_release: pre,
            build,
        })
    }
}

impl SemanticVersion {
    /// Parse version string into components
    fn parse_version_string(s: &str) -> Result<(u32, u32, u32), anyhow::Error> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            bail!("Invalid version format: {}", s);
        }

        let major = parts[0].parse().context("Invalid major version")?;
        let minor = parts[1].parse().context("Invalid minor version")?;
        let patch = parts[2].parse().context("Invalid patch version")?;

        Ok((major, minor, patch))
    }

    /// Check if this version satisfies a version requirement
    pub fn satisfies(&self, req: &VersionReq) -> bool {
        match req {
            VersionReq::Exact(version) => {
                if let Ok(other) = version.parse::<SemanticVersion>() {
                    self == &other
                } else {
                    false
                }
            }
            VersionReq::Compatible(version) => {
                if let Ok(other) = version.parse::<SemanticVersion>() {
                    self.major == other.major && self >= other
                } else {
                    false
                }
            }
            VersionReq::Tilde(version) => {
                if let Ok(other) = version.parse::<SemanticVersion>() {
                    self.major == other.major && self.minor == other.minor && self >= other
                } else {
                    false
                }
            }
            VersionReq::GreaterEqual(version) => {
                if let Ok(other) = version.parse::<SemanticVersion>() {
                    self >= other
                } else {
                    false
                }
            }
            VersionReq::Less(version) => {
                if let Ok(other) = version.parse::<SemanticVersion>() {
                    self < other
                } else {
                    false
                }
            }
            VersionReq::Greater(version) => {
                if let Ok(other) = version.parse::<SemanticVersion>() {
                    self > other
                } else {
                    false
                }
            }
            VersionReq::Wildcard => true,
            VersionReq::Any => true,
        }
    }
}

impl FromStr for VersionReq {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim();

        if s.is_empty() {
            bail!("Empty version requirement");
        }

        match s {
            "*" => Ok(VersionReq::Wildcard),
            "x.x.x" => Ok(VersionReq::Any),
            s if s.starts_with("^") => Ok(VersionReq::Compatible(s[1..].to_string())),
            s if s.starts_with("~") => Ok(VersionReq::Tilde(s[1..].to_string())),
            s if s.starts_with(">=") => Ok(VersionReq::GreaterEqual(s[2..].to_string())),
            s if s.starts_with("<") => Ok(VersionReq::Less(s[1..].to_string())),
            s if s.starts_with(">") => Ok(VersionReq::Greater(s[1..].to_string())),
            s if s.chars().all(|c| c.is_ascii_digit() || c == '.') => {
                Ok(VersionReq::Exact(s.to_string()))
            }
            _ => bail!("Invalid version requirement: {}", s),
        }
    }
}

/// Dependency resolver
pub struct DependencyResolver {
    registry: HashMap<String, Vec<PluginVersion>>,
}

impl DependencyResolver {
    /// Create a new dependency resolver
    pub fn new() -> Self {
        Self {
            registry: HashMap::new(),
        }
    }

    /// Add available plugin versions to the registry
    pub fn add_plugin_versions(&mut self, plugin_name: String, versions: Vec<PluginVersion>) {
        self.registry.insert(plugin_name, versions);
    }

    /// Resolve dependencies for a plugin
    pub fn resolve(&self, manifest: &PluginManifest) -> Result<ResolutionResult> {
        info!("Resolving dependencies for plugin: {}", manifest.package.name);

        let mut resolved = HashMap::new();
        let mut conflicts = Vec::new();
        let mut failures = Vec::new();
        let mut visited = HashSet::new();
        let mut to_visit = VecDeque::new();

        // Start with direct dependencies
        for (dep_name, dep_version) in &manifest.dependencies {
            to_visit.push_back((
                dep_name.clone(),
                dep_version.clone(),
                manifest.package.name.clone(),
                Vec::new(), // dependency chain
            ));
        }

        while let Some((dep_name, dep_version_req, required_by, mut dep_chain)) = to_visit.pop_front() {
            // Avoid circular dependencies
            if dep_chain.contains(&dep_name) {
                failures.push(DependencyFailure {
                    dependency: dep_name,
                    required_by,
                    reason: format!("Circular dependency detected: {:?}", dep_chain),
                });
                continue;
            }

            // Skip if already resolved
            if resolved.contains_key(&dep_name) {
                continue;
            }

            // Skip if already visited in this resolution path
            if visited.contains(&dep_name) {
                continue;
            }
            visited.insert(dep_name.clone());

            dep_chain.push(dep_name.clone());

            // Find compatible version
            let compatible_version = self.find_compatible_version(&dep_name, &dep_version_req)?;

            match compatible_version {
                Some(version) => {
                    // Check for conflicts with existing resolution
                    if let Some(existing) = resolved.get(&dep_name) {
                        if existing.version != version.version {
                            conflicts.push(DependencyConflict {
                                dependency: dep_name.clone(),
                                required_by: vec![
                                    (required_by, dep_version_req.clone()),
                                    (existing.required_by[0].clone(), "existing".to_string()),
                                ],
                                resolution: ConflictResolution::Failed(format!(
                                    "Version conflict: {} vs {}",
                                    existing.version, version.version
                                )),
                            });
                            continue;
                        }
                    }

                    let resolved_dep = ResolvedDependency {
                        name: dep_name.clone(),
                        version: version.version.clone(),
                        source: DependencySource::Registry,
                        required_by: vec![required_by.clone()],
                    };

                    resolved.insert(dep_name.clone(), resolved_dep);

                    // Add transitive dependencies
                    if let Some(transitive_manifest) = self.get_plugin_manifest(&dep_name, &version.version)? {
                        for (transitive_dep, transitive_req) in &transitive_manifest.dependencies {
                            to_visit.push_back((
                                transitive_dep.clone(),
                                transitive_req.clone(),
                                dep_name.clone(),
                                dep_chain.clone(),
                            ));
                        }
                    }
                }
                None => {
                    failures.push(DependencyFailure {
                        dependency: dep_name,
                        required_by,
                        reason: format!("No compatible version found for requirement: {}", dep_version_req),
                    });
                }
            }
        }

        Ok(ResolutionResult {
            resolved,
            conflicts,
            failures,
        })
    }

    /// Find a compatible version for a dependency
    fn find_compatible_version(&self, name: &str, requirement: &str) -> Result<Option<PluginVersion>> {
        let versions = match self.registry.get(name) {
            Some(versions) => versions,
            None => return Ok(None),
        };

        let req = VersionReq::from_str(requirement)
            .context("Invalid version requirement")?;

        // Filter versions that satisfy the requirement
        let compatible_versions: Vec<_> = versions
            .iter()
            .filter(|version| {
                if let Ok(semver) = version.version.parse::<SemanticVersion>() {
                    semver.satisfies(&req)
                } else {
                    false
                }
            })
            .collect();

        // Return the highest compatible version
        Ok(compatible_versions.into_iter().max().cloned())
    }

    /// Get plugin manifest for a specific version (mock implementation)
    fn get_plugin_manifest(&self, name: &str, version: &str) -> Result<Option<PluginManifest>> {
        // This would normally fetch from the database or registry
        // For now, return None as a placeholder
        Ok(None)
    }

    /// Perform topological sort of dependencies
    pub fn topological_sort(&self, dependencies: HashMap<String, ResolvedDependency>) -> Result<Vec<String>> {
        let mut graph = HashMap::new();
        let mut in_degree = HashMap::new();
        let mut all_nodes = HashSet::new();

        // Build dependency graph
        for (name, resolved_dep) in &dependencies {
            all_nodes.insert(name.clone());
            graph.insert(name.clone(), Vec::new());
            in_degree.insert(name.clone(), 0);
        }

        // This is a simplified version - in practice we'd need to fetch
        // the actual dependency information for each resolved dependency
        // For now, return nodes in a reasonable order
        let mut result = Vec::new();
        let mut queue = VecDeque::new();

        // Find nodes with no incoming edges (no dependencies)
        for node in &all_nodes {
            if let Some(degree) = in_degree.get(node) {
                if *degree == 0 {
                    queue.push_back(node.clone());
                }
            }
        }

        while let Some(node) = queue.pop_front() {
            result.push(node.clone());

            // Remove this node from graph
            all_nodes.remove(&node);
        }

        if !all_nodes.is_empty() {
            bail!("Circular dependency detected in dependency graph");
        }

        Ok(result)
    }
}

/// Plugin package builder
pub struct PackageBuilder;

impl PackageBuilder {
    /// Build a plugin package from manifest and source code
    pub fn build_package(manifest: &PluginManifest, source_dir: &str) -> Result<Vec<u8>> {
        info!("Building plugin package: {} v{}", manifest.package.name, manifest.package.version);

        // Create temporary directory for package
        let temp_dir = tempfile::tempdir()?;
        let package_dir = temp_dir.path().join(&manifest.package.name);

        // Copy source files
        Self::copy_source_files(source_dir, &package_dir)?;

        // Write plugin.toml
        let manifest_content = toml::to_string_pretty(manifest)?;
        std::fs::write(package_dir.join("plugin.toml"), manifest_content)?;

        // Create tarball
        let tarball = Self::create_tarball(&package_dir, &manifest.package.name)?;

        info!("Successfully built plugin package: {} bytes", tarball.len());
        Ok(tarball)
    }

    /// Copy source files to package directory
    fn copy_source_files(source_dir: &str, package_dir: &str) -> Result<()> {
        // Simple implementation - in practice, this would use .gitignore patterns
        // and exclude unnecessary files
        for entry in std::fs::read_dir(source_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "rs") {
                let dest = std::path::Path::new(package_dir).join(path.file_name().unwrap());
                std::fs::copy(&path, dest)?;
            }
        }
        Ok(())
    }

    /// Create tarball from package directory
    fn create_tarball(package_dir: &str, package_name: &str) -> Result<Vec<u8>> {
        // Simple implementation - in practice, this would create a proper tarball
        // For now, just return a placeholder
        Ok(format!("Tarball for {}", package_name).into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_version_parsing() {
        let version: SemanticVersion = "1.2.3".parse().unwrap();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 2);
        assert_eq!(version.patch, 3);
        assert_eq!(version.pre_release, None);
        assert_eq!(version.build, None);

        let version: SemanticVersion = "1.0.0-alpha.1".parse().unwrap();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
        assert_eq!(version.pre_release, Some("alpha.1".to_string()));
    }

    #[test]
    fn test_version_requirement_parsing() {
        assert!(matches!(VersionReq::from_str("^1.0.0").unwrap(), VersionReq::Compatible(_)));
        assert!(matches!(VersionReq::from_str("~1.0.0").unwrap(), VersionReq::Tilde(_)));
        assert!(matches!(VersionReq::from_str(">=1.0.0").unwrap(), VersionReq::GreaterEqual(_)));
        assert!(matches!(VersionReq::from_str("1.0.0").unwrap(), VersionReq::Exact(_)));
        assert!(matches!(VersionReq::from_str("*").unwrap(), VersionReq::Wildcard));
    }

    #[test]
    fn test_version_satisfaction() {
        let version: SemanticVersion = "1.2.3".parse().unwrap();

        assert!(version.satisfies(&VersionReq::Compatible("1.0.0".to_string())));
        assert!(version.satisfies(&VersionReq::Tilde("1.2.0".to_string())));
        assert!(version.satisfies(&VersionReq::GreaterEqual("1.0.0".to_string())));
        assert!(!version.satisfies(&VersionReq::Exact("1.0.0".to_string())));
    }

    #[test]
    fn test_dependency_resolver() {
        let resolver = DependencyResolver::new();

        // Add some mock plugin versions
        let versions = vec![
            PluginVersion {
                id: "1".to_string(),
                plugin_id: "test-plugin".to_string(),
                version: "1.0.0".to_string(),
                status: PluginStatus::Approved,
                package_size: 1000,
                package_hash: "hash".to_string(),
                manifest: PluginManifest {
                    package: PackageInfo {
                        name: "test-plugin".to_string(),
                        version: "1.0.0".to_string(),
                        description: None,
                        authors: vec![],
                        license: "MIT".to_string(),
                        homepage: None,
                        repository: None,
                        keywords: vec![],
                    },
                    plugin: PluginInfo {
                        aegis_version: "^0.2.0".to_string(),
                        plugin_api_version: "^1.0.0".to_string(),
                        engine_name: None,
                        format_support: vec![],
                        features: vec![],
                    },
                    compliance: ComplianceInfo {
                        risk_level: RiskLevel::Low,
                        publisher_policy: PublisherPolicy::Permissive,
                        bounty_eligible: true,
                        enterprise_approved: true,
                        notes: None,
                    },
                    dependencies: HashMap::new(),
                    dev_dependencies: HashMap::new(),
                    build: None,
                    testing: None,
                    security: None,
                },
                signature: None,
                published_at: chrono::Utc::now(),
            }
        ];

        resolver.add_plugin_versions("test-plugin".to_string(), versions);

        let manifest = PluginManifest {
            package: PackageInfo {
                name: "test-app".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                authors: vec![],
                license: "MIT".to_string(),
                homepage: None,
                repository: None,
                keywords: vec![],
            },
            plugin: PluginInfo {
                aegis_version: "^0.2.0".to_string(),
                plugin_api_version: "^1.0.0".to_string(),
                engine_name: None,
                format_support: vec![],
                features: vec![],
            },
            compliance: ComplianceInfo {
                risk_level: RiskLevel::Low,
                publisher_policy: PublisherPolicy::Permissive,
                bounty_eligible: true,
                enterprise_approved: true,
                notes: None,
            },
            dependencies: {
                let mut deps = HashMap::new();
                deps.insert("test-plugin".to_string(), "^1.0.0".to_string());
                deps
            },
            dev_dependencies: HashMap::new(),
            build: None,
            testing: None,
            security: None,
        };

        let result = resolver.resolve(&manifest).unwrap();
        assert!(result.failures.is_empty());
        assert!(result.conflicts.is_empty());
        assert_eq!(result.resolved.len(), 1);
    }
}
