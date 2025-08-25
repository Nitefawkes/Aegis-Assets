use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Compliance level for publishers/games
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplianceLevel {
    /// Publisher explicitly allows modding/extraction
    Permissive,
    /// No explicit stance, proceed with caution
    Neutral,
    /// History of aggressive IP enforcement
    HighRisk,
}

/// Compliance profile for a specific game/publisher
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceProfile {
    pub publisher: String,
    pub game_id: Option<String>,
    pub enforcement_level: ComplianceLevel,
    pub official_support: bool,
    pub bounty_eligible: bool,
    pub enterprise_warning: Option<String>,
    pub mod_policy_url: Option<String>,
    pub supported_formats: HashMap<String, FormatSupport>,
}

/// Level of support for a specific format
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormatSupport {
    /// Officially supported with full documentation
    Supported,
    /// Community plugins only, use at your own risk
    CommunityOnly,
    /// Explicitly not supported due to legal concerns
    NotSupported,
}

/// Metadata about a file entry in an archive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryMetadata {
    pub id: EntryId,
    pub name: String,
    pub path: PathBuf,
    pub size_compressed: Option<u64>,
    pub size_uncompressed: u64,
    pub file_type: Option<String>,
    pub last_modified: Option<DateTime<Utc>>,
    pub checksum: Option<String>,
}

/// Unique identifier for an entry within an archive
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryId(pub String);

impl EntryId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Provenance information for tracking asset extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    /// Unique extraction session ID
    pub session_id: Uuid,
    /// Game identifier (if detected)
    pub game_id: Option<String>,
    /// BLAKE3 hash of source archive
    pub source_hash: String,
    /// Path to original source file
    pub source_path: PathBuf,
    /// Compliance profile applied
    pub compliance_profile: ComplianceProfile,
    /// Time of extraction
    pub extraction_time: DateTime<Utc>,
    /// Version of aegis-core used
    pub aegis_version: String,
    /// Plugin name and version that performed extraction
    pub plugin_info: PluginInfo,
}

/// Information about the plugin that performed extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub author: Option<String>,
    pub compliance_verified: bool,
}

/// Main trait for archive handlers - plugins must implement this
pub trait ArchiveHandler: Send + Sync {
    /// Detect if this handler can process the given file
    fn detect(bytes: &[u8]) -> bool
    where
        Self: Sized;

    /// Open an archive file with compliance checking
    fn open(path: &Path) -> Result<Self>
    where
        Self: Sized;

    /// Get compliance profile for this archive
    fn compliance_profile(&self) -> &ComplianceProfile;

    /// List all entries in the archive
    fn list_entries(&self) -> Result<Vec<EntryMetadata>>;

    /// Read a specific entry by ID
    fn read_entry(&self, id: &EntryId) -> Result<Vec<u8>>;

    /// Get provenance information for this extraction session
    fn provenance(&self) -> &Provenance;

    /// Check if extraction is allowed based on compliance profile
    fn is_extraction_allowed(&self) -> bool {
        match self.compliance_profile().enforcement_level {
            ComplianceLevel::Permissive | ComplianceLevel::Neutral => true,
            ComplianceLevel::HighRisk => {
                // High-risk formats require explicit user consent
                false
            }
        }
    }

    /// Get warning message for high-risk extractions
    fn compliance_warning(&self) -> Option<&str> {
        match self.compliance_profile().enforcement_level {
            ComplianceLevel::HighRisk => {
                self.compliance_profile().enterprise_warning.as_deref()
            }
            _ => None,
        }
    }
}

/// Registry for managing compliance profiles
pub struct ComplianceRegistry {
    profiles: HashMap<String, ComplianceProfile>,
}

impl ComplianceRegistry {
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
        }
    }

    /// Load compliance profiles from directory
    pub fn load_from_directory(dir: &Path) -> Result<Self> {
        let mut registry = Self::new();
        
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().map_or(false, |ext| ext == "yaml" || ext == "yml") {
                let content = std::fs::read_to_string(&path)?;
                let profile: ComplianceProfile = serde_yaml::from_str(&content)?;
                let key = path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                registry.profiles.insert(key, profile);
            }
        }
        
        Ok(registry)
    }

    /// Get compliance profile for a game/publisher
    pub fn get_profile(&self, game_id: &str) -> Option<&ComplianceProfile> {
        self.profiles.get(game_id)
    }

    /// Get default profile for unknown games
    pub fn default_profile() -> ComplianceProfile {
        ComplianceProfile {
            publisher: "Unknown".to_string(),
            game_id: None,
            enforcement_level: ComplianceLevel::Neutral,
            official_support: false,
            bounty_eligible: false,
            enterprise_warning: Some(
                "Unknown game/publisher. Proceed with caution and ensure you own the content."
                    .to_string()
            ),
            mod_policy_url: None,
            supported_formats: HashMap::new(),
        }
    }
}

impl Default for ComplianceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_compliance_levels() {
        let permissive = ComplianceProfile {
            publisher: "Bethesda".to_string(),
            game_id: Some("skyrim".to_string()),
            enforcement_level: ComplianceLevel::Permissive,
            official_support: true,
            bounty_eligible: true,
            enterprise_warning: None,
            mod_policy_url: Some("https://bethesda.net/modding-policy".to_string()),
            supported_formats: HashMap::new(),
        };

        assert_eq!(permissive.enforcement_level, ComplianceLevel::Permissive);
        assert!(permissive.official_support);
    }

    #[test]
    fn test_entry_metadata_creation() {
        let metadata = EntryMetadata {
            id: EntryId::new("test_asset.png"),
            name: "test_asset.png".to_string(),
            path: PathBuf::from("textures/test_asset.png"),
            size_compressed: Some(1024),
            size_uncompressed: 4096,
            file_type: Some("texture".to_string()),
            last_modified: None,
            checksum: Some("blake3:abc123...".to_string()),
        };

        assert_eq!(metadata.name, "test_asset.png");
        assert_eq!(metadata.size_uncompressed, 4096);
    }
}
