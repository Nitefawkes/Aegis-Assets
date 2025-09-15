# Plugin Registry Technical Specification v1.0

## Overview

The Aegis-Assets Plugin Registry provides a centralized system for publishing, versioning, distributing, and managing community-contributed format plugins. Built with security-first architecture and compliance integration.

## Design Principles

1. **Security by Default**: All plugins sandboxed, code-signed, and scanned
2. **Semantic Versioning**: Full semver compatibility with dependency resolution
3. **Compliance Integration**: Automatic risk assessment for supported formats
4. **Developer Experience**: Simple publish/install workflow
5. **Enterprise Control**: Admin override capabilities for enterprise deployments

## Plugin Package Format

### Package Structure
```
my-engine-plugin-1.2.3.tar.gz
├── plugin.toml              # Plugin manifest
├── src/
│   ├── lib.rs              # Main plugin code
│   ├── formats.rs          # Format parsers
│   └── converters.rs       # Asset converters
├── tests/
│   ├── integration_tests.rs
│   └── sample_files/
├── docs/
│   ├── README.md
│   └── COMPLIANCE.md       # Risk assessment
├── .aegis-signature        # Code signature
└── Cargo.toml             # Rust dependencies
```

### Plugin Manifest (plugin.toml)
```toml
[package]
name = "godot-plugin"
version = "1.2.3"
description = "Godot engine asset extraction support"
authors = ["community@aegis-assets.org"]
license = "MIT"
homepage = "https://github.com/aegis-assets/godot-plugin"
repository = "https://github.com/aegis-assets/godot-plugin"
keywords = ["godot", "game-engine", "3d", "extraction"]

[plugin]
# Plugin metadata for Aegis registry
aegis_version = "^0.2.0"           # Compatible Aegis version
plugin_api_version = "1.0"         # Plugin API compatibility
engine_name = "Godot"               # Target engine
format_support = [                 # Supported file formats
    { extension = "scn", description = "Godot scene files" },
    { extension = "tscn", description = "Godot text scene files" },
    { extension = "res", description = "Godot resource files" }
]

[compliance]
# Risk assessment for enterprise deployments
risk_level = "low"                  # "low" | "medium" | "high" 
publisher_policy = "permissive"     # Publisher's known stance on asset extraction
bounty_eligible = true             # Can participate in bounty program
enterprise_approved = true         # Pre-approved for enterprise use

[dependencies]
# Aegis core dependencies
aegis-core = "0.2.0"
serde = "1.0"
anyhow = "1.0"

# Optional engine-specific dependencies
godot-parser = "0.1.0"

[dev-dependencies]
tempfile = "3.8"

[build]
# Build requirements
min_rust_version = "1.70.0"
features = ["default"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc", "x86_64-apple-darwin"]

[testing]
# Test requirements
sample_files = [
    "tests/sample_files/basic_scene.scn",
    "tests/sample_files/complex_model.tscn"
]
required_success_rate = 0.95       # Minimum test pass rate
performance_benchmark = {          # Performance requirements
    max_extraction_time = "2s",
    max_memory_usage = "256MB"
}

[security]
# Security metadata for scanning
sandbox_permissions = [            # Required system permissions
    "file_read",                   # Can read input files
    "network_none"                 # No network access required
]
code_signing_cert = "community"    # Certificate level: "community" | "verified" | "enterprise"
vulnerability_scan = "required"    # Automatic security scanning
```

### Code Signature Format
```json
{
  "signature_version": "1.0",
  "plugin_name": "godot-plugin",
  "plugin_version": "1.2.3",
  "signer": "community@aegis-assets.org",
  "signature_algorithm": "Ed25519",
  "public_key": "base64-encoded-public-key",
  "signature": "base64-encoded-signature",
  "timestamp": "2025-09-04T10:30:00Z",
  "cert_chain": [
    "base64-encoded-cert-1",
    "base64-encoded-cert-2"
  ],
  "trust_level": "community"
}
```

## Registry API Specification

### Plugin Management Endpoints

#### Publish Plugin
```
POST /api/v1/registry/plugins
Content-Type: multipart/form-data
Authorization: Bearer <dev-token>

Form Data:
- package: plugin-1.2.3.tar.gz (binary)
- signature: .aegis-signature (json)
- metadata: plugin.toml (text)

Response:
{
  "plugin_id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "godot-plugin",
  "version": "1.2.3",
  "status": "pending_review",
  "scan_id": "scan_123456",
  "estimated_review_time": "2-4 hours",
  "publish_url": "/registry/plugins/godot-plugin/1.2.3"
}
```

#### List Plugins
```
GET /api/v1/registry/plugins
?engine=godot
&category=game-engine
&sort=popularity
&limit=50
&offset=0

Response:
{
  "plugins": [
    {
      "name": "godot-plugin",
      "version": "1.2.3",
      "description": "Godot engine asset extraction support",
      "author": "community@aegis-assets.org",
      "downloads": 1247,
      "rating": 4.6,
      "last_updated": "2025-09-01T00:00:00Z",
      "compliance": {
        "risk_level": "low",
        "enterprise_approved": true
      }
    }
  ],
  "pagination": {
    "total": 123,
    "page": 1,
    "per_page": 50
  }
}
```

#### Get Plugin Details
```
GET /api/v1/registry/plugins/{plugin-name}/{version}

Response:
{
  "plugin_id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "godot-plugin",
  "version": "1.2.3",
  "description": "Godot engine asset extraction support",
  "author": "community@aegis-assets.org",
  "license": "MIT",
  "homepage": "https://github.com/aegis-assets/godot-plugin",
  "downloads": {
    "total": 1247,
    "last_30_days": 89
  },
  "rating": {
    "average": 4.6,
    "count": 23
  },
  "versions": ["1.0.0", "1.1.0", "1.2.0", "1.2.3"],
  "dependencies": {
    "aegis-core": "^0.2.0",
    "serde": "1.0"
  },
  "format_support": [
    {
      "extension": "scn",
      "description": "Godot scene files",
      "tested": true
    }
  ],
  "compliance": {
    "risk_level": "low",
    "publisher_policy": "permissive",
    "enterprise_approved": true,
    "bounty_eligible": true
  },
  "security": {
    "last_scan": "2025-09-01T10:30:00Z",
    "scan_status": "passed",
    "vulnerabilities": 0,
    "trust_level": "community"
  }
}
```

#### Download Plugin
```
GET /api/v1/registry/plugins/{plugin-name}/{version}/download
Authorization: Bearer <user-token>

Response:
Content-Type: application/gzip
Content-Disposition: attachment; filename="godot-plugin-1.2.3.tar.gz"
Content-Length: 1048576
X-Plugin-Signature: base64-encoded-signature
X-Scan-Status: passed

[Binary plugin package data]
```

## Version Management

### Semantic Versioning
- **MAJOR**: Incompatible API changes
- **MINOR**: Backward-compatible functionality additions  
- **PATCH**: Backward-compatible bug fixes
- **Pre-release**: `-alpha.1`, `-beta.2`, `-rc.1`

### Version Resolution Algorithm
```rust
// Simplified dependency resolution logic
pub struct VersionResolver {
    registry: PluginRegistry,
}

impl VersionResolver {
    /// Resolve compatible plugin version based on semver constraints
    pub fn resolve_version(&self, requirement: &str) -> Result<Version> {
        let constraint = VersionReq::parse(requirement)?;
        let available_versions = self.registry.get_versions()?;
        
        // Find highest compatible version
        let compatible: Vec<_> = available_versions
            .into_iter()
            .filter(|v| constraint.matches(v))
            .collect();
            
        compatible
            .into_iter()
            .max()
            .ok_or_else(|| anyhow::anyhow!("No compatible version found"))
    }
    
    /// Resolve dependency tree with conflict detection
    pub fn resolve_dependencies(&self, root_plugin: &Plugin) -> Result<DependencyGraph> {
        let mut resolver = DependencyResolver::new();
        resolver.add_constraints(root_plugin.dependencies());
        
        // Recursive dependency resolution
        let resolved = resolver.resolve()?;
        
        // Check for conflicts
        self.validate_compatibility(&resolved)?;
        
        Ok(resolved)
    }
}
```

### Dependency Conflict Resolution
```toml
# Example conflict scenario in plugin.toml
[dependencies]
aegis-core = "0.2.0"        # Requires specific version
another-plugin = "1.0"      # Which requires aegis-core = "^0.1.0"

# Resolution strategy:
# 1. Check if 0.2.0 satisfies ^0.1.0 (yes - 0.2.0 >= 0.1.0, < 0.2.0 is false)
# 2. Use highest compatible version (0.2.0)
# 3. If incompatible, fail with clear error message
```

## Database Schema

### Plugin Registry Tables
```sql
-- Core plugin metadata
CREATE TABLE plugins (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) UNIQUE NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    description TEXT,
    author_email VARCHAR(255) NOT NULL,
    license VARCHAR(50) NOT NULL,
    homepage TEXT,
    repository TEXT,
    keywords TEXT[],
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Plugin versions
CREATE TABLE plugin_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plugin_id UUID REFERENCES plugins(id) ON DELETE CASCADE,
    version VARCHAR(50) NOT NULL,
    semver_major INTEGER NOT NULL,
    semver_minor INTEGER NOT NULL,
    semver_patch INTEGER NOT NULL,
    semver_prerelease VARCHAR(50),
    manifest JSONB NOT NULL,                    -- Full plugin.toml content
    package_size BIGINT NOT NULL,              -- Package size in bytes
    package_hash CHAR(64) NOT NULL,            -- Blake3 hash
    signature JSONB,                           -- Code signature data
    status VARCHAR(50) DEFAULT 'pending',      -- pending, approved, rejected, deprecated
    published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(plugin_id, version)
);

-- Plugin dependencies
CREATE TABLE plugin_dependencies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plugin_version_id UUID REFERENCES plugin_versions(id) ON DELETE CASCADE,
    dependency_name VARCHAR(255) NOT NULL,
    version_requirement VARCHAR(100) NOT NULL, -- Semver constraint like "^1.0.0"
    dependency_type VARCHAR(50) DEFAULT 'runtime', -- runtime, dev, build
    optional BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Supported file formats
CREATE TABLE plugin_formats (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plugin_version_id UUID REFERENCES plugin_versions(id) ON DELETE CASCADE,
    file_extension VARCHAR(20) NOT NULL,
    format_description TEXT,
    mime_type VARCHAR(100),
    tested BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Download statistics
CREATE TABLE plugin_downloads (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plugin_version_id UUID REFERENCES plugin_versions(id) ON DELETE CASCADE,
    user_id VARCHAR(255),                      -- Optional user tracking
    download_timestamp TIMESTAMPTZ DEFAULT NOW(),
    ip_address INET,
    user_agent TEXT,
    success BOOLEAN DEFAULT TRUE
);

-- Plugin ratings and reviews
CREATE TABLE plugin_reviews (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plugin_id UUID REFERENCES plugins(id) ON DELETE CASCADE,
    user_id VARCHAR(255) NOT NULL,
    rating INTEGER CHECK (rating >= 1 AND rating <= 5),
    review_text TEXT,
    version_used VARCHAR(50),
    helpful_count INTEGER DEFAULT 0,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(plugin_id, user_id)                 -- One review per user per plugin
);
```

### Indexing Strategy
```sql
-- Performance indexes
CREATE INDEX idx_plugin_versions_plugin_id ON plugin_versions(plugin_id);
CREATE INDEX idx_plugin_versions_status ON plugin_versions(status);
CREATE INDEX idx_plugin_versions_semver ON plugin_versions(semver_major, semver_minor, semver_patch);

CREATE INDEX idx_plugin_dependencies_name ON plugin_dependencies(dependency_name);
CREATE INDEX idx_plugin_formats_extension ON plugin_formats(file_extension);

CREATE INDEX idx_plugin_downloads_timestamp ON plugin_downloads(download_timestamp);
CREATE INDEX idx_plugin_downloads_plugin_version ON plugin_downloads(plugin_version_id);

-- Full-text search
CREATE INDEX idx_plugins_search ON plugins USING gin(to_tsvector('english', name || ' ' || description || ' ' || array_to_string(keywords, ' ')));
```

## Security Architecture

### Sandboxing Requirements
```toml
# Plugin security permissions in plugin.toml
[security]
sandbox_permissions = [
    "file_read",              # Read access to input files
    "file_write_temp",        # Write to temporary directories only  
    "process_spawn_none",     # No subprocess execution
    "network_none",           # No network access
    "filesystem_restricted"   # Limited filesystem access
]
```

### Code Signing Workflow
```rust
// Code signing implementation
pub struct CodeSigner {
    private_key: Ed25519PrivateKey,
    cert_chain: Vec<Certificate>,
}

impl CodeSigner {
    /// Sign plugin package
    pub fn sign_package(&self, package_path: &Path) -> Result<Signature> {
        // Calculate package hash
        let package_data = std::fs::read(package_path)?;
        let package_hash = blake3::hash(&package_data);
        
        // Create signature payload
        let payload = SignaturePayload {
            package_hash: package_hash.to_hex(),
            timestamp: Utc::now(),
            signer: self.get_signer_info(),
        };
        
        // Sign with Ed25519
        let signature_bytes = self.private_key.sign(&payload.to_bytes());
        
        Ok(Signature {
            algorithm: "Ed25519".to_string(),
            signature: base64::encode(&signature_bytes),
            public_key: base64::encode(&self.private_key.public_key().to_bytes()),
            payload,
            cert_chain: self.cert_chain.clone(),
        })
    }
    
    /// Verify plugin signature
    pub fn verify_signature(&self, package_data: &[u8], signature: &Signature) -> Result<bool> {
        // Verify certificate chain
        self.verify_cert_chain(&signature.cert_chain)?;
        
        // Verify package hash
        let actual_hash = blake3::hash(package_data);
        if actual_hash.to_hex() != signature.payload.package_hash {
            return Ok(false);
        }
        
        // Verify signature
        let public_key = Ed25519PublicKey::from_bytes(&base64::decode(&signature.public_key)?)?;
        let signature_bytes = base64::decode(&signature.signature)?;
        
        Ok(public_key.verify(&signature.payload.to_bytes(), &signature_bytes).is_ok())
    }
}
```

### Vulnerability Scanning
```rust
// Automated security scanning pipeline
pub struct SecurityScanner {
    rules: Vec<SecurityRule>,
}

impl SecurityScanner {
    /// Scan plugin code for vulnerabilities
    pub async fn scan_plugin(&self, plugin_path: &Path) -> Result<ScanReport> {
        let mut report = ScanReport::new();
        
        // Static code analysis
        report.merge(self.run_static_analysis(plugin_path).await?);
        
        // Dependency vulnerability check
        report.merge(self.check_dependencies(plugin_path).await?);
        
        // Permission analysis
        report.merge(self.analyze_permissions(plugin_path).await?);
        
        // Malware detection
        report.merge(self.run_malware_scan(plugin_path).await?);
        
        Ok(report)
    }
    
    async fn run_static_analysis(&self, plugin_path: &Path) -> Result<ScanReport> {
        // Use Clippy + custom rules for Rust code analysis
        let mut report = ScanReport::new();
        
        // Check for unsafe code blocks
        if self.contains_unsafe_code(plugin_path)? {
            report.add_finding(SecurityFinding {
                severity: Severity::High,
                category: "unsafe_code",
                message: "Plugin contains unsafe code blocks".to_string(),
                location: None,
            });
        }
        
        // Check for filesystem access patterns
        if self.has_unrestricted_file_access(plugin_path)? {
            report.add_finding(SecurityFinding {
                severity: Severity::Medium,
                category: "file_access",
                message: "Plugin may access files outside sandbox".to_string(),
                location: None,
            });
        }
        
        Ok(report)
    }
}
```

## Plugin Installation Client

### CLI Installation
```bash
# Install plugin from registry
aegis plugin install godot-plugin@1.2.3

# Install with specific constraints
aegis plugin install godot-plugin@^1.2.0

# List installed plugins
aegis plugin list
godot-plugin@1.2.3 (active)
unity-plugin@0.2.1 (active) 
unreal-plugin@0.1.5 (inactive)

# Update plugin
aegis plugin update godot-plugin
Updating godot-plugin from 1.2.3 to 1.2.4...
✓ Downloaded and verified signature
✓ Security scan passed
✓ Dependency resolution successful
✓ Installation complete

# Remove plugin
aegis plugin remove godot-plugin
Warning: This will remove godot-plugin@1.2.3
Continue? (y/N): y
✓ Plugin removed successfully
```

### Installation API
```rust
// Plugin installation client
pub struct PluginInstaller {
    registry_client: RegistryClient,
    plugin_dir: PathBuf,
    config: InstallConfig,
}

impl PluginInstaller {
    /// Install plugin from registry
    pub async fn install(&self, plugin_spec: &str) -> Result<InstallResult> {
        let spec = PluginSpec::parse(plugin_spec)?;
        
        // Resolve version and dependencies
        let resolved = self.registry_client.resolve_dependencies(&spec).await?;
        
        // Download and verify all packages
        let packages = self.download_packages(&resolved).await?;
        
        // Verify signatures
        self.verify_packages(&packages)?;
        
        // Run security scans
        self.scan_packages(&packages).await?;
        
        // Install in dependency order
        self.install_packages(&packages).await?;
        
        Ok(InstallResult {
            installed_plugins: resolved.plugins,
            total_size: packages.iter().map(|p| p.size).sum(),
            install_time: std::time::Instant::now(),
        })
    }
    
    /// Update installed plugin
    pub async fn update(&self, plugin_name: &str) -> Result<UpdateResult> {
        let current_version = self.get_installed_version(plugin_name)?;
        let latest_version = self.registry_client.get_latest_version(plugin_name).await?;
        
        if current_version >= latest_version {
            return Ok(UpdateResult::AlreadyLatest);
        }
        
        // Check for breaking changes
        if latest_version.major > current_version.major {
            return Err(anyhow::anyhow!(
                "Major version update requires manual intervention: {} -> {}",
                current_version, latest_version
            ));
        }
        
        // Perform update
        self.install(&format!("{}@{}", plugin_name, latest_version)).await?;
        
        Ok(UpdateResult::Updated {
            from: current_version,
            to: latest_version,
        })
    }
}
```

## Admin Dashboard Requirements

### Plugin Review Interface
- **Pending Reviews**: Queue of submitted plugins awaiting approval
- **Security Reports**: Vulnerability scan results with remediation suggestions
- **Compliance Flags**: Risk assessment and enterprise approval status
- **Performance Metrics**: Download stats, ratings, and usage analytics

### Moderation Tools
- **Plugin Flagging**: Community reporting system for problematic plugins
- **Version Management**: Deprecate/remove specific versions
- **Publisher Management**: Author verification and trust levels
- **Batch Operations**: Bulk approve/reject for efficiency

---

**Status**: Technical Specification Complete  
**Dependencies**: None (foundational document)  
**Next Steps**: Begin API implementation and database schema creation  
**Risk Level**: Medium (complex dependency resolution, security requirements)  

**Implementation Priority**:
1. Database schema and basic CRUD operations (Week 3)
2. Plugin package format and manifest parsing (Week 3-4)  
3. Security scanning and code signing (Week 4)
4. Dependency resolution algorithm (Week 4)
