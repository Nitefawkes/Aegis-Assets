use aegis_core::{
    archive::{ArchiveHandler, ComplianceProfile, EntryId, EntryMetadata, PluginInfo, Provenance},
    PluginFactory,
};
use anyhow::{bail, Context, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use memmap2::{Mmap, MmapOptions};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs::File;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

mod compression;
mod converters;
mod formats;
mod mesh_pipeline;
mod audio_pipeline;
mod firelight_adpcm;

#[cfg(test)]
mod integration_test;

use compression::decompress_unity_data;
use audio_pipeline::{convert_unity_audio_clip, AudioPipelineOptions, AudioConversionResult};
use converters::{convert_unity_asset, UnityAudioClip, UnityMesh};
use formats::{AssetBundle, SerializedFile};
use mesh_pipeline::{convert_unity_mesh, MeshPipelineOptions};

/// Asset information for better categorization and AI tagging
#[derive(Debug, Clone)]
struct AssetInfo {
    name: String,
    category: String,
    extension: String,
    ai_tags: Vec<String>,
}

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

    fn compliance_info(&self) -> aegis_core::PluginInfo {
        aegis_core::PluginInfo::new(
            "Unity".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
            vec!["unity3d".to_string(), "assets".to_string()]
        )
    }
}

/// Unity archive handler backed by a shared memory map for streaming access
pub struct UnityArchive {
    file_path: PathBuf,
    data: Mmap,
    bundle: Option<AssetBundle>,
    serialized_file: Option<SerializedFile>,
    compliance_profile: ComplianceProfile,
    provenance: Provenance,
    entries: Vec<EntryMetadata>,
    mesh_options: MeshPipelineOptions,
    audio_options: AudioPipelineOptions,
}

impl UnityArchive {
    /// Detect Unity file format from header bytes
    pub fn detect(bytes: &[u8]) -> bool {
        if bytes.len() < 16 {
            debug!("File too small for Unity detection: {} bytes", bytes.len());
            return false;
        }

        // Check for UnityFS signature (Unity 5.3+) - includes UnityFS2, UnityFS3, etc.
        if bytes.starts_with(b"UnityFS") {
            debug!("Detected UnityFS signature");
            return true;
        }

        // Check for UnityRaw signature (older versions)
        if bytes.starts_with(b"UnityRaw") {
            debug!("Detected UnityRaw signature");
            return true;
        }

        // Check for UnityWeb signature (web builds)
        if bytes.starts_with(b"UnityWeb") {
            debug!("Detected UnityWeb signature");
            return true;
        }

        // Check for serialized file format (CAB-* or other Unity signatures)
        if bytes.len() >= 20 {
            let mut cursor = Cursor::new(bytes);

            // Try to read metadata table offset
            if let Ok(metadata_size) = cursor.read_u32::<LittleEndian>() {
                if let Ok(file_size) = cursor.read_u32::<LittleEndian>() {
                    // Basic sanity checks for Unity serialized file
                    if metadata_size > 0
                        && metadata_size < file_size
                        && file_size < bytes.len() as u32 * 2
                    {
                        debug!("Detected Unity serialized file format");
                        return true;
                    }
                }
            }
        }

        debug!("No Unity signature detected");
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
        
        // Create memory map for efficient access
        let file = std::fs::File::open(path)?;
        let data = unsafe { Mmap::map(&file)? };
        
        // Determine file type and parse
        let (bundle, serialized_file) = Self::parse_unity_file(&data)?;
        
        // Generate provenance
        let provenance = Self::create_provenance(path, &compliance_profile)?;
        
        // Extract entry metadata
        let entries = Self::extract_entries(&bundle, &serialized_file)?;

        info!("Loaded Unity archive with {} entries", entries.len());

        Ok(Self {
            file_path: path.to_path_buf(),
            data,
            bundle,
            serialized_file,
            compliance_profile,
            provenance,
            entries,
            mesh_options: MeshPipelineOptions::default(),
            audio_options: AudioPipelineOptions::default(),
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
                "Unity games have varying IP policies. Check publisher-specific compliance."
                    .to_string(),
            ),
            mod_policy_url: None,
            supported_formats: {
                let mut formats = HashMap::new();
                formats.insert(
                    "unity3d".to_string(),
                    aegis_core::FormatSupport::CommunityOnly,
                );
                formats.insert(
                    "assets".to_string(),
                    aegis_core::FormatSupport::CommunityOnly,
                );
                formats.insert(
                    "resource".to_string(),
                    aegis_core::FormatSupport::CommunityOnly,
                );
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
        if data.starts_with(b"UnityFS") {
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
                let block_info = Self::categorize_bundle_block(&block.name, i);

                entries.push(EntryMetadata {
                    id: EntryId::new(format!("block_{}", i)),
                    name: block_info.name,
                    path: PathBuf::from(format!("{}.{}", block.name, block_info.extension)),
                    size_compressed: Some(block.compressed_size as u64),
                    size_uncompressed: block.size as u64,
                    file_type: Some(block_info.category),
                    last_modified: None,
                    checksum: None,
                });
            }
        }

        if let Some(serialized) = serialized_file {
            for object in serialized.objects.iter() {
                let asset_info = Self::get_asset_info(object.class_id, &serialized.type_tree);

                entries.push(EntryMetadata {
                    id: EntryId::new(format!("object_{}", object.path_id)),
                    name: asset_info.name,
                    path: PathBuf::from(format!("{}.{}", object.path_id, asset_info.extension)),
                    size_compressed: None,
                    size_uncompressed: object.size as u64,
                    file_type: Some(asset_info.category),
                    last_modified: None,
                    checksum: None,
                });
            }
        }

        Ok(entries)
    }

    /// Categorize UnityFS bundle blocks for better organization
    fn categorize_bundle_block(block_name: &str, index: usize) -> AssetInfo {
        // Try to infer asset type from block name patterns
        let name_lower = block_name.to_lowercase();

        if name_lower.contains("texture") || name_lower.contains("tex") {
            AssetInfo {
                name: format!("Texture_{}", block_name),
                category: "texture".to_string(),
                extension: "png".to_string(),
                ai_tags: vec!["texture".to_string(), "image".to_string()],
            }
        } else if name_lower.contains("mesh") || name_lower.contains("model") {
            AssetInfo {
                name: format!("Mesh_{}", block_name),
                category: "mesh".to_string(),
                extension: "gltf".to_string(),
                ai_tags: vec!["mesh".to_string(), "geometry".to_string(), "3d".to_string()],
            }
        } else if name_lower.contains("material") || name_lower.contains("mat") {
            AssetInfo {
                name: format!("Material_{}", block_name),
                category: "material".to_string(),
                extension: "material.json".to_string(),
                ai_tags: vec!["material".to_string(), "shader".to_string()],
            }
        } else if name_lower.contains("audio") || name_lower.contains("sound") {
            AssetInfo {
                name: format!("Audio_{}", block_name),
                category: "audio".to_string(),
                extension: "ogg".to_string(),
                ai_tags: vec!["audio".to_string(), "sound".to_string()],
            }
        } else if name_lower.contains("anim") {
            AssetInfo {
                name: format!("Animation_{}", block_name),
                category: "animation".to_string(),
                extension: "animation.json".to_string(),
                ai_tags: vec!["animation".to_string(), "motion".to_string()],
            }
        } else if name_lower.contains("scene") {
            AssetInfo {
                name: format!("Scene_{}", block_name),
                category: "scene".to_string(),
                extension: "scene".to_string(),
                ai_tags: vec!["scene".to_string(), "level".to_string()],
            }
        } else if name_lower.contains("prefab") {
            AssetInfo {
                name: format!("Prefab_{}", block_name),
                category: "prefab".to_string(),
                extension: "prefab".to_string(),
                ai_tags: vec!["prefab".to_string(), "template".to_string()],
            }
        } else if name_lower.contains("script") || name_lower.contains("mono") {
            AssetInfo {
                name: format!("Script_{}", block_name),
                category: "script".to_string(),
                extension: "cs".to_string(),
                ai_tags: vec!["script".to_string(), "code".to_string()],
            }
        } else {
            // Default categorization
            AssetInfo {
                name: format!("Asset_{}", block_name),
                category: "asset".to_string(),
                extension: "bin".to_string(),
                ai_tags: vec!["asset".to_string()],
            }
        }
    }

    /// Get detailed asset information for better categorization and metadata
    fn get_asset_info(
        class_id: i32,
        type_tree: &std::collections::HashMap<i32, formats::TypeInfo>,
    ) -> AssetInfo {
        // Get base type name from type tree
        let base_name = type_tree
            .get(&class_id)
            .map(|t| t.type_name.clone())
            .unwrap_or_else(|| format!("Type_{}", class_id));

        // Unity class ID mappings for better categorization
        match class_id {
            // Textures
            28 => AssetInfo {
                name: format!("Texture2D_{}", base_name),
                category: "texture".to_string(),
                extension: "png".to_string(),
                ai_tags: vec!["texture".to_string(), "image".to_string()],
            },
            89 => AssetInfo {
                name: format!("Cubemap_{}", base_name),
                category: "texture".to_string(),
                extension: "png".to_string(),
                ai_tags: vec!["texture".to_string(), "cubemap".to_string()],
            },
            117 => AssetInfo {
                name: format!("Texture2DArray_{}", base_name),
                category: "texture".to_string(),
                extension: "png".to_string(),
                ai_tags: vec!["texture".to_string(), "array".to_string()],
            },

            // Meshes
            43 => AssetInfo {
                name: format!("Mesh_{}", base_name),
                category: "mesh".to_string(),
                extension: "gltf".to_string(),
                ai_tags: vec!["mesh".to_string(), "geometry".to_string(), "3d".to_string()],
            },
            156 => AssetInfo {
                name: format!("Terrain_{}", base_name),
                category: "mesh".to_string(),
                extension: "gltf".to_string(),
                ai_tags: vec![
                    "mesh".to_string(),
                    "terrain".to_string(),
                    "landscape".to_string(),
                ],
            },

            // Materials
            21 => AssetInfo {
                name: format!("Material_{}", base_name),
                category: "material".to_string(),
                extension: "material.json".to_string(),
                ai_tags: vec!["material".to_string(), "shader".to_string()],
            },
            45 => AssetInfo {
                name: format!("Shader_{}", base_name),
                category: "shader".to_string(),
                extension: "shader".to_string(),
                ai_tags: vec!["shader".to_string(), "program".to_string()],
            },

            // Audio
            83 => AssetInfo {
                name: format!("AudioClip_{}", base_name),
                category: "audio".to_string(),
                extension: "ogg".to_string(),
                ai_tags: vec!["audio".to_string(), "sound".to_string()],
            },

            // Animations
            74 => AssetInfo {
                name: format!("AnimationClip_{}", base_name),
                category: "animation".to_string(),
                extension: "animation.json".to_string(),
                ai_tags: vec!["animation".to_string(), "motion".to_string()],
            },

            // Prefabs and Scenes
            1001 => AssetInfo {
                name: format!("Prefab_{}", base_name),
                category: "prefab".to_string(),
                extension: "prefab".to_string(),
                ai_tags: vec!["prefab".to_string(), "template".to_string()],
            },
            115 => AssetInfo {
                name: format!("MonoBehaviour_{}", base_name),
                category: "script".to_string(),
                extension: "cs".to_string(),
                ai_tags: vec!["script".to_string(), "component".to_string()],
            },

            // UI Elements
            224 => AssetInfo {
                name: format!("RectTransform_{}", base_name),
                category: "ui".to_string(),
                extension: "ui".to_string(),
                ai_tags: vec!["ui".to_string(), "interface".to_string()],
            },
            222 => AssetInfo {
                name: format!("Canvas_{}", base_name),
                category: "ui".to_string(),
                extension: "ui".to_string(),
                ai_tags: vec!["ui".to_string(), "canvas".to_string()],
            },

            // Physics
            54 => AssetInfo {
                name: format!("Rigidbody_{}", base_name),
                category: "physics".to_string(),
                extension: "physics".to_string(),
                ai_tags: vec!["physics".to_string(), "rigidbody".to_string()],
            },
            65 => AssetInfo {
                name: format!("BoxCollider_{}", base_name),
                category: "physics".to_string(),
                extension: "physics".to_string(),
                ai_tags: vec!["physics".to_string(), "collider".to_string()],
            },

            // Lighting
            108 => AssetInfo {
                name: format!("Light_{}", base_name),
                category: "lighting".to_string(),
                extension: "light".to_string(),
                ai_tags: vec!["lighting".to_string(), "light".to_string()],
            },

            // Particles
            198 => AssetInfo {
                name: format!("ParticleSystem_{}", base_name),
                category: "particles".to_string(),
                extension: "particles".to_string(),
                ai_tags: vec!["particles".to_string(), "effects".to_string()],
            },

            // Default fallback
            _ => AssetInfo {
                name: base_name.clone(),
                category: "asset".to_string(),
                extension: "asset".to_string(),
                ai_tags: vec!["asset".to_string()],
            },
        }
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
        let _entry = self
            .entries
            .iter()
            .find(|e| e.id == *id)
            .ok_or_else(|| anyhow::anyhow!("Entry not found: {}", id.0))?;

        // Extract data based on entry type
        if let Some(ref bundle) = self.bundle {
            if id.0.starts_with("block_") {
                let block_index: usize =
                    id.0.strip_prefix("block_")
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
                let path_id: u64 =
                    id.0.strip_prefix("object_")
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
        
        // Use the unified decompression function
        let result = decompress_unity_data(
            compressed_data,
            block.compression_type,
            block.size as usize,
        );

        if let Err(ref err) = result {
            warn!(
                compression_type = block.compression_type,
                expected_size = block.size,
                actual_size = compressed_data.len(),
                error = %err,
                "Failed to decompress Unity bundle block"
            );
        }

        result
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

    /// Get converted asset (PNG, glTF, OGG, etc.) - Unity-specific functionality
    pub fn read_converted_entry(&self, id: &EntryId) -> Result<(String, Vec<u8>)> {
        debug!("Reading converted entry: {}", id.0);

        // First get the raw data
        let raw_data = self.read_entry(id)?;

        // Try to convert based on Unity object type
        if let Some(ref serialized) = self.serialized_file {
            if id.0.starts_with("object_") {
                let path_id: u64 =
                    id.0.strip_prefix("object_")
                        .unwrap()
                        .parse()
                        .context("Invalid path ID")?;

                if let Some(object) = serialized.objects.iter().find(|o| o.path_id == path_id) {
                    // Special handling for AudioClip to use enhanced audio pipeline
                    if object.class_id == 83 { // AudioClip
                        match UnityAudioClip::parse(&raw_data) {
                            Ok(clip) => {
                                match convert_unity_audio_clip(&clip, &self.audio_options) {
                                    Ok(result) => {
                                        info!(
                                            "Audio pipeline produced {} ({})",
                                            result.primary.filename,
                                            result.stats
                                        );
                                        if let Some(ref secondary) = result.secondary {
                                            info!(
                                                "Audio secondary artifact available: {} ({} bytes)",
                                                secondary.filename,
                                                secondary.bytes.len()
                                            );
                                        }
                                        if let Some(ref loop_meta) = result.loop_metadata {
                                            info!("Audio loop metadata: {}", loop_meta);
                                        }
                                        info!("Audio validation: {}", result.validation);
                                        for warning in &result.warnings {
                                            warn!("Audio conversion warning: {}", warning);
                                        }
                                        
                                        // Create AudioResource for core system integration
                                        let audio_resource = Self::create_audio_resource_from_result(&result, &clip);
                                        
                                        // Store the AudioResource for potential re-export with different codecs
                                        // For now, return the primary artifact as before
                                        return Ok((result.primary.filename, result.primary.bytes));
                                    }
                                    Err(e) => {
                                        warn!("Audio pipeline failed for {}: {}. Returning raw data.", id.0, e);
                                        return Ok((format!("{}.bin", id.0), raw_data));
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse AudioClip {}: {}. Falling back to generic converter.", id.0, e);
                            }
                        }
                    }
                    
                    // Special handling for Mesh to use enhanced mesh pipeline
                    if object.class_id == 43 { // Mesh
                        match UnityMesh::parse(&raw_data) {
                            Ok(mesh) => {
                                match convert_unity_mesh(&mesh, &self.mesh_options) {
                                    Ok(result) => {
                                        info!(
                                            "Mesh pipeline produced {} ({})",
                                            result.primary.filename,
                                            result.stats
                                        );
                                        if let Some(ref fallback) = result.fallback {
                                            info!(
                                                "Mesh fallback artifact available: {} ({} bytes)",
                                                fallback.filename,
                                                fallback.bytes.len()
                                            );
                                        }
                                        info!("Mesh validation: {:?}", result.validation);
                                        
                                        return Ok((result.primary.filename, result.primary.bytes));
                                    }
                                    Err(e) => {
                                        warn!("Mesh pipeline failed for {}: {}. Falling back to generic converter.", id.0, e);
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse Mesh {}: {}. Falling back to generic converter.", id.0, e);
                            }
                        }
                    }
                    
                    // Try to convert the asset
                    match convert_unity_asset(object.class_id, &raw_data) {
                        Ok((filename, converted_data)) => {
                            info!("Successfully converted {} to {}", id.0, filename);
                            return Ok((filename, converted_data));
                        }
                        Err(e) => {
                            warn!("Failed to convert asset {}: {}. Returning raw data.", id.0, e);
                            // Fall back to raw data with .bin extension
                            return Ok((format!("{}.bin", id.0), raw_data));
                        }
                    }
                }
            }
        }

        // For bundle blocks or unsupported types, return raw data
        Ok((format!("{}.bin", id.0), raw_data))
    }
    
    /// Create an AudioResource from Unity audio pipeline results for core system integration
    fn create_audio_resource_from_result(
        result: &AudioConversionResult,
        clip: &UnityAudioClip,
    ) -> aegis_core::resource::AudioResource {
        use aegis_core::resource::AudioFormat;
        
        // Determine the audio format based on the primary artifact
        let format = if result.primary.filename.ends_with(".wav") {
            AudioFormat::WAV
        } else if result.primary.filename.ends_with(".ogg") {
            if result.primary.media_type.contains("vorbis") {
                AudioFormat::Vorbis
            } else {
                AudioFormat::OGG
            }
        } else if result.primary.filename.ends_with(".mp3") {
            AudioFormat::MP3
        } else if result.primary.filename.ends_with(".flac") {
            AudioFormat::FLAC
        } else if result.primary.filename.ends_with(".opus") {
            AudioFormat::Opus
        } else {
            // Default to PCM if we can't determine format
            AudioFormat::PCM
        };
        
        aegis_core::resource::AudioResource {
            name: clip.name.clone(),
            format,
            data: result.primary.bytes.clone(),
            sample_rate: result.stats.sample_rate,
            channels: result.stats.channels as u8,
            duration_seconds: result.stats.duration_seconds as f32,
        }
    }
}

impl Drop for UnityArchive {
    fn drop(&mut self) {
        debug!("Closing Unity archive: {}", self.file_path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unity_detection() {
        // Test UnityFS detection
        let unityfs_header = [
            b'U', b'n', b'i', b't', b'y', b'F', b'S', 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert!(UnityArchive::detect(&unityfs_header));

        // Test UnityRaw detection
        let unityraw_header = [
            b'U', b'n', b'i', b't', b'y', b'R', b'a', b'w', 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert!(UnityArchive::detect(&unityraw_header));
        
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
        assert_eq!(info.name(), "Unity");
        // PluginInfo doesn't have compliance_verified field anymore
        // assert!(info.compliance_verified);
    }
}
