use aegis_core::{
    archive::{ArchiveHandler, ComplianceProfile, EntryMetadata, EntryId, Provenance, PluginInfo},
    resource::{Resource, TextureResource, MeshResource, TextureFormat, TextureUsage},
    PluginFactory,
};
use anyhow::{Result, Context, bail};
use byteorder::{LittleEndian, ReadBytesExt};
use std::collections::HashMap;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

mod formats;
mod compression;

use formats::{UnityVersion, AssetBundle, SerializedFile};
use compression::{decompress_lz4, decompress_lzma};

/// Unity plugin factory
pub struct UnityPluginFactory;

impl PluginFactory for UnityPluginFactory {
    fn name(&self) -> &str {
        "Unity"
    }
    
    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }
    
    fn supported_extensions(&self) -> Vec<&str> {
        vec!["unity3d", "assets", "sharedAssets", "resource", "resS"]
    }
    
    fn can_handle(&self, bytes: &[u8]) -> bool {
        UnityArchive::detect(bytes)
    }
    
    fn create_handler(&self, path: &Path) -> Result<Box<dyn ArchiveHandler>> {
        let handler = UnityArchive::open(path)?;
        Ok(Box::new(handler))
    }
    
    fn compliance_info(&self) -> PluginInfo {
        PluginInfo {
            name: "Unity".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            author: Some("Aegis-Assets Team".to_string()),
            compliance_verified: true,
        }
    }
}

/// Unity archive handler
pub struct UnityArchive {
    file_path: PathBuf,
    bundle: Option<AssetBundle>,
    serialized_file: Option<SerializedFile>,
    compliance_profile: ComplianceProfile,
    provenance: Provenance,
    entries: Vec<EntryMetadata>,
}

impl UnityArchive {
    /// Detect Unity file format from header bytes
    pub fn detect(bytes: &[u8]) -> bool {
        if bytes.len() < 16 {
            return false;
        }
        
        // Check for UnityFS signature (Unity 5.3+)
        if bytes.starts_with(b"UnityFS\0") {
            return true;
        }
        
        // Check for UnityRaw signature (older versions)
        if bytes.starts_with(b"UnityRaw") {
            return true;
        }
        
        // Check for UnityWeb signature (web builds)
        if bytes.starts_with(b"UnityWeb") {
            return true;
        }
        
        // Check for serialized file format (CAB-* or other Unity signatures)
        if bytes.len() >= 20 {
            let mut cursor = Cursor::new(bytes);
            
            // Try to read metadata table offset
            if let Ok(metadata_size) = cursor.read_u32::<LittleEndian>() {
                if let Ok(file_size) = cursor.read_u32::<LittleEndian>() {
                    // Basic sanity checks for Unity serialized file
                    if metadata_size > 0 && 
                       metadata_size < file_size && 
                       file_size < bytes.len() as u32 * 2 {
                        return true;
                    }
                }
            }
        }
        
        false
    }
    
    /// Open Unity archive file
    pub fn open(path: &Path) -> Result<Self> {
        info!("Opening Unity archive: {}", path.display());
        
        // Load compliance profile for Unity
        let compliance_profile = Self::load_compliance_profile();
        
        // Check if file exists and is readable
        if !path.exists() {
            bail!("File does not exist: {}", path.display());
        }
        
        let file_data = std::fs::read(path)
            .context("Failed to read Unity archive file")?;
        
        // Determine file type and parse
        let (bundle, serialized_file) = Self::parse_unity_file(&file_data)?;
        
        // Generate provenance
        let provenance = Self::create_provenance(path, &compliance_profile)?;
        
        // Extract entry metadata
        let entries = Self::extract_entries(&bundle, &serialized_file)?;
        
        info!("Loaded Unity archive with {} entries", entries.len());
        
        Ok(Self {
            file_path: path.to_path_buf(),
            bundle,
            serialized_file,
            compliance_profile,
            provenance,
            entries,
        })
    }
    
    /// Load compliance profile for Unity
    fn load_compliance_profile() -> ComplianceProfile {
        // In a real implementation, this would load from the compliance registry
        ComplianceProfile {
            publisher: "Unity Technologies".to_string(),
            game_id: Some("unity".to_string()),
            enforcement_level: aegis_core::ComplianceLevel::Neutral,
            official_support: false,
            bounty_eligible: true,
            enterprise_warning: Some(
                "Unity games have varying IP policies. Check publisher-specific compliance.".to_string()
            ),
            mod_policy_url: None,
            supported_formats: {
                let mut formats = HashMap::new();
                formats.insert("unity3d".to_string(), aegis_core::FormatSupport::CommunityOnly);
                formats.insert("assets".to_string(), aegis_core::FormatSupport::CommunityOnly);
                formats.insert("resource".to_string(), aegis_core::FormatSupport::CommunityOnly);
                formats
            },
        }
    }
    
    /// Create provenance information
    fn create_provenance(path: &Path, profile: &ComplianceProfile) -> Result<Provenance> {
        let source_data = std::fs::read(path)?;
        let source_hash = blake3::hash(&source_data).to_hex().to_string();
        
        Ok(Provenance {
            session_id: uuid::Uuid::new_v4(),
            game_id: Some("unity_generic".to_string()),
            source_hash,
            source_path: path.to_path_buf(),
            compliance_profile: profile.clone(),
            extraction_time: chrono::Utc::now(),
            aegis_version: aegis_core::VERSION.to_string(),
            plugin_info: PluginInfo {
                name: "Unity".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                author: Some("Aegis-Assets Team".to_string()),
                compliance_verified: true,
            },
        })
    }
    
    /// Parse Unity file format
    fn parse_unity_file(data: &[u8]) -> Result<(Option<AssetBundle>, Option<SerializedFile>)> {
        if data.starts_with(b"UnityFS\0") {
            // UnityFS format (Unity 5.3+)
            debug!("Detected UnityFS format");
            let bundle = AssetBundle::parse(data)?;
            Ok((Some(bundle), None))
        } else if data.len() >= 20 {
            // Try to parse as serialized file
            debug!("Attempting to parse as serialized file");
            let serialized = SerializedFile::parse(data)?;
            Ok((None, Some(serialized)))
        } else {
            bail!("Unsupported Unity file format");
        }
    }
    
    /// Extract entry metadata from parsed structures
    fn extract_entries(
        bundle: &Option<AssetBundle>,
        serialized_file: &Option<SerializedFile>,
    ) -> Result<Vec<EntryMetadata>> {
        let mut entries = Vec::new();
        
        if let Some(bundle) = bundle {
            for (i, block) in bundle.directory_info.iter().enumerate() {
                entries.push(EntryMetadata {
                    id: EntryId::new(format!("block_{}", i)),
                    name: block.name.clone(),
                    path: PathBuf::from(&block.name),
                    size_compressed: Some(block.compressed_size as u64),
                    size_uncompressed: block.size as u64,
                    file_type: Some("unity_block".to_string()),
                    last_modified: None,
                    checksum: None,
                });
            }
        }
        
        if let Some(serialized) = serialized_file {
            for (i, object) in serialized.objects.iter().enumerate() {
                let type_name = serialized.type_tree.get(&object.class_id)
                    .map(|t| t.type_name.clone())
                    .unwrap_or_else(|| format!("Type_{}", object.class_id));
                
                entries.push(EntryMetadata {
                    id: EntryId::new(format!("object_{}", object.path_id)),
                    name: format!("{}_{}", type_name, object.path_id),
                    path: PathBuf::from(format!("{}.asset", object.path_id)),
                    size_compressed: None,
                    size_uncompressed: object.size as u64,
                    file_type: Some(type_name),
                    last_modified: None,
                    checksum: None,
                });
            }
        }
        
        Ok(entries)
    }
}

impl ArchiveHandler for UnityArchive {
    fn detect(bytes: &[u8]) -> bool {
        UnityArchive::detect(bytes)
    }
    
    fn open(path: &Path) -> Result<Self> {
        UnityArchive::open(path)
    }
    
    fn compliance_profile(&self) -> &ComplianceProfile {
        &self.compliance_profile
    }
    
    fn list_entries(&self) -> Result<Vec<EntryMetadata>> {
        Ok(self.entries.clone())
    }
    
    fn read_entry(&self, id: &EntryId) -> Result<Vec<u8>> {
        debug!("Reading entry: {}", id.0);
        
        // Find the entry
        let entry = self.entries.iter()
            .find(|e| e.id == *id)
            .ok_or_else(|| anyhow::anyhow!("Entry not found: {}", id.0))?;
        
        // Extract data based on entry type
        if let Some(ref bundle) = self.bundle {
            if id.0.starts_with("block_") {
                let block_index: usize = id.0.strip_prefix("block_")
                    .unwrap()
                    .parse()
                    .context("Invalid block index")?;
                
                if block_index < bundle.directory_info.len() {
                    return self.extract_bundle_block(&bundle.directory_info[block_index]);
                }
            }
        }
        
        if let Some(ref serialized) = self.serialized_file {
            if id.0.starts_with("object_") {
                let path_id: u64 = id.0.strip_prefix("object_")
                    .unwrap()
                    .parse()
                    .context("Invalid path ID")?;
                
                if let Some(object) = serialized.objects.iter().find(|o| o.path_id == path_id) {
                    return self.extract_serialized_object(object, serialized);
                }
            }
        }
        
        bail!("Entry not found or unsupported: {}", id.0)
    }
    
    fn provenance(&self) -> &Provenance {
        &self.provenance
    }
}

impl UnityArchive {
    /// Extract data from a bundle block
    fn extract_bundle_block(&self, block: &formats::DirectoryInfo) -> Result<Vec<u8>> {
        let file_data = std::fs::read(&self.file_path)?;
        
        let start = block.offset as usize;
        let end = start + block.compressed_size as usize;
        
        if end > file_data.len() {
            bail!("Block extends beyond file boundaries");
        }
        
        let compressed_data = &file_data[start..end];
        
        match block.compression_type {
            0 => Ok(compressed_data.to_vec()), // No compression
            1 => bail!("LZMA compression not yet implemented"),
            2 => decompress_lz4(compressed_data, block.size as usize), // LZ4
            3 => bail!("LZ4HC compression not yet implemented"),
            4 => bail!("LZHAM compression not yet implemented"),
            _ => bail!("Unknown compression type: {}", block.compression_type),
        }
    }
    
    /// Extract data from a serialized object
    fn extract_serialized_object(
        &self,
        object: &formats::ObjectInfo,
        _serialized: &SerializedFile,
    ) -> Result<Vec<u8>> {
        let file_data = std::fs::read(&self.file_path)?;
        
        let start = object.offset as usize;
        let end = start + object.size as usize;
        
        if end > file_data.len() {
            bail!("Object extends beyond file boundaries");
        }
        
        Ok(file_data[start..end].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_unity_detection() {
        // Test UnityFS detection
        let unityfs_header = b"UnityFS\0\x07\x05\x00\x00";
        assert!(UnityArchive::detect(unityfs_header));
        
        // Test UnityRaw detection
        let unityraw_header = b"UnityRaw\x00\x00\x00\x00";
        assert!(UnityArchive::detect(unityraw_header));
        
        // Test invalid header
        let invalid_header = b"Invalid\0\x00\x00\x00";
        assert!(!UnityArchive::detect(invalid_header));
    }
    
    #[test]
    fn test_plugin_factory() {
        let factory = UnityPluginFactory;
        
        assert_eq!(factory.name(), "Unity");
        assert!(factory.supported_extensions().contains(&"unity3d"));
        assert!(factory.supported_extensions().contains(&"assets"));
        
        let info = factory.compliance_info();
        assert_eq!(info.name, "Unity");
        assert!(info.compliance_verified);
    }
}
