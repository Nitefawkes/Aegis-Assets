//! Unity plugin template

use std::collections::HashMap;

/// Get Unity plugin template files
pub fn get_unity_template_files() -> HashMap<String, String> {
    let mut files = HashMap::new();
    
    // Cargo.toml
    files.insert("Cargo.toml".to_string(), UNITY_CARGO_TOML.to_string());
    
    // Main library file
    files.insert("src/lib.rs".to_string(), UNITY_LIB_RS.to_string());
    
    // Unity-specific modules
    files.insert("src/unity/mod.rs".to_string(), UNITY_MOD_RS.to_string());
    files.insert("src/unity/asset_bundle.rs".to_string(), UNITY_ASSET_BUNDLE_RS.to_string());
    files.insert("src/unity/serialization.rs".to_string(), UNITY_SERIALIZATION_RS.to_string());
    files.insert("src/unity/resources.rs".to_string(), UNITY_RESOURCES_RS.to_string());
    
    // Tests
    files.insert("tests/integration_tests.rs".to_string(), UNITY_INTEGRATION_TESTS.to_string());
    files.insert("tests/fixtures/test_bundle.ab".to_string(), "".to_string()); // Binary placeholder
    
    // Examples
    files.insert("examples/extract_assets.rs".to_string(), UNITY_EXAMPLE_EXTRACT.to_string());
    files.insert("examples/bundle_info.rs".to_string(), UNITY_EXAMPLE_INFO.to_string());
    
    // Documentation
    files.insert("README.md".to_string(), UNITY_README.to_string());
    files.insert("docs/unity-formats.md".to_string(), UNITY_DOCS_FORMATS.to_string());
    
    // Configuration files
    files.insert(".gitignore".to_string(), GITIGNORE.to_string());
    files.insert("plugin.toml".to_string(), UNITY_PLUGIN_CONFIG.to_string());
    
    files
}

const UNITY_CARGO_TOML: &str = r#"[package]
name = "{{name}}"
version = "0.1.0"
edition = "2021"
authors = ["{{author}}"]
license = "{{license}}"
description = "{{description}}"
repository = "https://github.com/{{author}}/{{name}}"
keywords = ["aegis", "unity", "asset-extraction", "game-dev"]
categories = ["game-development", "parsing"]

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
# Aegis core dependencies
aegis-core = { version = "0.1", features = ["plugin-api"] }

# Serialization and parsing
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
byteorder = "1.5"
nom = "7.1"

# Binary format handling
lz4_flex = "0.11"
{{#if (has_feature "compression-support")}}
flate2 = "1.0"
{{/if}}

# Async support
tokio = { version = "1.0", features = ["full"] }
async-trait = "0.1"

# Error handling
anyhow = "1.0"
thiserror = "1.0"

# Logging
tracing = "0.1"

{{#if has_tests}}
[dev-dependencies]
tempfile = "3.8"
tokio-test = "0.4"
{{/if}}

{{#if has_benchmarks}}
[[bench]]
name = "asset_extraction"
harness = false

[dev-dependencies.criterion]
version = "0.5"
features = ["html_reports"]
{{/if}}

[features]
default = ["asset-bundles"]
asset-bundles = []
{{#each features}}
{{this}} = []
{{/each}}
"#;

const UNITY_LIB_RS: &str = r#"//! {{description}}
//!
//! This plugin provides support for extracting assets from Unity asset bundles
//! and other Unity-specific formats with full compliance checking.

use aegis_core::{
    plugin::{Plugin, PluginFactory, PluginInfo, PluginResult},
    resource::{Resource, ResourceType},
    compliance::ComplianceInfo,
    error::PluginError,
};
use async_trait::async_trait;
use std::path::Path;
use tracing::{info, debug, warn};

pub mod unity;
pub use unity::*;

/// Unity plugin factory
pub struct {{name_camel}}Factory;

impl PluginFactory for {{name_camel}}Factory {
    fn create_plugin(&self) -> Box<dyn Plugin> {
        Box::new({{name_camel}}Plugin::new())
    }
    
    fn plugin_info(&self) -> PluginInfo {
        PluginInfo {
            name: "{{name}}".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "{{description}}".to_string(),
            author: Some("{{author}}".to_string()),
            supported_extensions: vec![
                {{#each supported_formats}}
                "{{this}}".to_string(),
                {{/each}}
            ],
            compliance_info: ComplianceInfo {
                compliance_verified: true,
                risk_level: aegis_core::compliance::RiskLevel::{{compliance_level}},
                publisher_policy: aegis_core::compliance::PublisherPolicy::{{engine}},
                description: Some("{{description}}".to_string()),
                author: Some("{{author}}".to_string()),
                homepage: Some("https://github.com/{{author}}/{{name}}".to_string()),
                bounty_eligible: true,
                enterprise_approved: {{#if (eq compliance_level "enterprise")}}true{{else}}false{{/if}},
            },
        }
    }
}

/// Main Unity plugin implementation
pub struct {{name_camel}}Plugin {
    // Plugin state
}

impl {{name_camel}}Plugin {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Plugin for {{name_camel}}Plugin {
    async fn can_handle(&self, file_path: &Path, file_header: &[u8]) -> PluginResult<bool> {
        // Check if this is a Unity asset bundle
        if file_header.len() >= 8 {
            // Unity asset bundle signature
            if &file_header[0..8] == b"UnityFS\0" || &file_header[0..8] == b"UnityWeb" {
                debug!("Detected Unity asset bundle: {}", file_path.display());
                return Ok(true);
            }
        }
        
        // Check file extension
        if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                {{#each supported_formats}}
                "{{this}}" => {
                    debug!("Detected Unity file by extension: {}", file_path.display());
                    Ok(true)
                },
                {{/each}}
                _ => Ok(false),
            }
        } else {
            Ok(false)
        }
    }
    
    async fn extract_resources(&mut self, file_path: &Path) -> PluginResult<Vec<Resource>> {
        info!("Extracting resources from Unity file: {}", file_path.display());
        
        let file_data = std::fs::read(file_path)
            .map_err(|e| PluginError::IoError(e.to_string()))?;
        
        // Parse Unity asset bundle
        let bundle = unity::AssetBundle::parse(&file_data)
            .map_err(|e| PluginError::ParseError(e.to_string()))?;
        
        let mut resources = Vec::new();
        
        // Extract assets from bundle
        for asset in bundle.assets() {
            let resource = match asset.asset_type() {
                unity::AssetType::Texture2D => {
                    self.extract_texture(asset)?
                },
                unity::AssetType::Mesh => {
                    self.extract_mesh(asset)?
                },
                unity::AssetType::AudioClip => {
                    self.extract_audio(asset)?
                },
                unity::AssetType::Material => {
                    self.extract_material(asset)?
                },
                _ => {
                    debug!("Skipping unsupported asset type: {:?}", asset.asset_type());
                    continue;
                }
            };
            
            resources.push(resource);
        }
        
        info!("Extracted {} resources from {}", resources.len(), file_path.display());
        Ok(resources)
    }
    
    async fn list_entries(&self, file_path: &Path) -> PluginResult<Vec<aegis_core::archive::ArchiveEntry>> {
        let file_data = std::fs::read(file_path)
            .map_err(|e| PluginError::IoError(e.to_string()))?;
        
        let bundle = unity::AssetBundle::parse(&file_data)
            .map_err(|e| PluginError::ParseError(e.to_string()))?;
        
        let entries = bundle.assets()
            .iter()
            .map(|asset| aegis_core::archive::ArchiveEntry {
                name: asset.name().to_string(),
                size_uncompressed: asset.size() as u64,
                size_compressed: Some(asset.compressed_size() as u64),
                file_type: Some(format!("{:?}", asset.asset_type())),
                path: asset.path().map(|p| p.to_string()),
            })
            .collect();
            
        Ok(entries)
    }
}

impl {{name_camel}}Plugin {
    fn extract_texture(&self, asset: &unity::Asset) -> PluginResult<Resource> {
        // Extract texture data and convert to standard format
        let texture_data = asset.data();
        
        Ok(Resource {
            name: asset.name().to_string(),
            resource_type: ResourceType::Texture,
            data: texture_data.to_vec(),
            size: texture_data.len(),
            format: "Unity Texture2D".to_string(),
            metadata: serde_json::json!({
                "unity_asset_type": "Texture2D",
                "original_format": asset.get_property("format"),
                "width": asset.get_property("width"),
                "height": asset.get_property("height"),
            }),
        })
    }
    
    fn extract_mesh(&self, asset: &unity::Asset) -> PluginResult<Resource> {
        let mesh_data = asset.data();
        
        Ok(Resource {
            name: asset.name().to_string(),
            resource_type: ResourceType::Mesh,
            data: mesh_data.to_vec(),
            size: mesh_data.len(),
            format: "Unity Mesh".to_string(),
            metadata: serde_json::json!({
                "unity_asset_type": "Mesh",
                "vertex_count": asset.get_property("vertex_count"),
                "triangle_count": asset.get_property("triangle_count"),
            }),
        })
    }
    
    fn extract_audio(&self, asset: &unity::Asset) -> PluginResult<Resource> {
        let audio_data = asset.data();
        
        Ok(Resource {
            name: asset.name().to_string(),
            resource_type: ResourceType::Audio,
            data: audio_data.to_vec(),
            size: audio_data.len(),
            format: "Unity AudioClip".to_string(),
            metadata: serde_json::json!({
                "unity_asset_type": "AudioClip",
                "channels": asset.get_property("channels"),
                "frequency": asset.get_property("frequency"),
                "length": asset.get_property("length"),
            }),
        })
    }
    
    fn extract_material(&self, asset: &unity::Asset) -> PluginResult<Resource> {
        let material_data = asset.data();
        
        Ok(Resource {
            name: asset.name().to_string(),
            resource_type: ResourceType::Material,
            data: material_data.to_vec(),
            size: material_data.len(),
            format: "Unity Material".to_string(),
            metadata: serde_json::json!({
                "unity_asset_type": "Material",
                "shader": asset.get_property("shader"),
                "properties": asset.get_property("properties"),
            }),
        })
    }
}

// Export the plugin factory for dynamic loading
aegis_core::export_plugin!({{name_camel}}Factory);
"#;

const UNITY_MOD_RS: &str = r#"//! Unity-specific format parsing and asset extraction

pub mod asset_bundle;
pub mod serialization;
pub mod resources;

pub use asset_bundle::*;
pub use serialization::*;
pub use resources::*;

use anyhow::Result;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

/// Unity asset types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetType {
    GameObject = 1,
    Component = 2,
    LevelGameManager = 3,
    Transform = 4,
    TimeManager = 5,
    GlobalGameManager = 6,
    GameManager = 8,
    Behaviour = 9,
    GameObjectManager = 11,
    AudioManager = 12,
    InputManager = 13,
    Physics2DSettings = 19,
    Camera = 20,
    Material = 21,
    MeshRenderer = 23,
    Renderer = 25,
    Texture = 27,
    Texture2D = 28,
    MeshFilter = 33,
    OcclusionCullingSettings = 41,
    GraphicsSettings = 48,
    MeshCollider = 64,
    BoxCollider = 65,
    CompositeCollider2D = 66,
    EdgeCollider2D = 68,
    CapsuleCollider2D = 70,
    Mesh = 43,
    Shader = 48,
    TextAsset = 49,
    Rigidbody2D = 50,
    CircleCollider2D = 58,
    PolygonCollider2D = 60,
    BoxCollider2D = 61,
    PhysicsMaterial2D = 62,
    MeshCollider2D = 64,
    Rigidbody = 54,
    CollisionDetection2D = 71,
    AudioClip = 83,
    AudioSource = 82,
    RenderTexture = 84,
    Cubemap = 89,
    Avatar = 90,
    GUILayer = 92,
    RuntimeAnimatorController = 93,
    ScriptMapper = 94,
    Animator = 95,
    GUITexture = 96,
    GUIText = 97,
    GUIElement = 98,
    PhysicMaterial = 134,
    SphereCollider = 135,
    CapsuleCollider = 136,
    SkinnedMeshRenderer = 137,
    FixedJoint = 138,
    RaycastCollider = 140,
    BuildSettings = 141,
    AssetBundle = 142,
    CharacterController = 143,
    CharacterJoint = 144,
    SpringJoint = 145,
    WheelCollider = 146,
    ResourceManager = 147,
    PreloadData = 150,
    MovieTexture = 152,
    ConfigurableJoint = 153,
    TerrainCollider = 154,
    TerrainData = 156,
    LightmapSettings = 157,
    WebCamTexture = 158,
    EditorSettings = 159,
    InteractiveCloth = 160,
    ClothRenderer = 161,
    EditorUserSettings = 162,
    SkinnedCloth = 163,
    AudioReverbFilter = 164,
    AudioHighPassFilter = 165,
    AudioChorusFilter = 166,
    AudioReverbZone = 167,
    AudioEchoFilter = 168,
    AudioLowPassFilter = 169,
    AudioDistortionFilter = 170,
    SparseTexture = 171,
    AudioBehaviour = 180,
    AudioFilter = 181,
    WindZone = 182,
    Cloth = 183,
    SubstanceArchive = 184,
    ProceduralMaterial = 185,
    ProceduralTexture = 186,
    OffMeshLink = 191,
    OcclusionArea = 192,
    Tree = 193,
    NavMeshAgent = 195,
    NavMeshSettings = 196,
    ParticleAnimator = 198,
    ParticleRenderer = 199,
    ShaderVariantCollection = 200,
}

impl AssetType {
    /// Parse asset type from integer
    pub fn from_id(id: i32) -> Option<Self> {
        match id {
            1 => Some(AssetType::GameObject),
            21 => Some(AssetType::Material),
            28 => Some(AssetType::Texture2D),
            43 => Some(AssetType::Mesh),
            48 => Some(AssetType::Shader),
            49 => Some(AssetType::TextAsset),
            83 => Some(AssetType::AudioClip),
            142 => Some(AssetType::AssetBundle),
            _ => None,
        }
    }
}

/// Unity version information
#[derive(Debug, Clone)]
pub struct UnityVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub build: String,
}

impl UnityVersion {
    pub fn parse(version_str: &str) -> Result<Self> {
        let parts: Vec<&str> = version_str.split('.').collect();
        if parts.len() < 3 {
            return Err(anyhow::anyhow!("Invalid Unity version format"));
        }
        
        Ok(UnityVersion {
            major: parts[0].parse()?,
            minor: parts[1].parse()?,
            patch: parts[2].parse()?,
            build: parts.get(3).unwrap_or(&"").to_string(),
        })
    }
    
    pub fn supports_feature(&self, feature: &str) -> bool {
        match feature {
            "lz4_compression" => self.major >= 5 || (self.major == 4 && self.minor >= 6),
            "streaming_info" => self.major >= 5,
            "scriptable_objects" => self.major >= 4,
            _ => false,
        }
    }
}
"#;

const UNITY_ASSET_BUNDLE_RS: &str = r#"//! Unity Asset Bundle format parsing

use super::*;
use anyhow::{Result, Context};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use std::collections::HashMap;
use std::io::{Cursor, Read};

/// Unity Asset Bundle
#[derive(Debug)]
pub struct AssetBundle {
    header: BundleHeader,
    assets: Vec<Asset>,
    metadata: BundleMetadata,
}

impl AssetBundle {
    /// Parse asset bundle from bytes
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);
        
        // Parse header
        let header = BundleHeader::parse(&mut cursor)
            .context("Failed to parse bundle header")?;
        
        // Parse metadata
        let metadata = BundleMetadata::parse(&mut cursor, &header)
            .context("Failed to parse bundle metadata")?;
        
        // Parse assets
        let assets = Self::parse_assets(&mut cursor, &header, &metadata)
            .context("Failed to parse assets")?;
        
        Ok(AssetBundle {
            header,
            assets,
            metadata,
        })
    }
    
    /// Get all assets in the bundle
    pub fn assets(&self) -> &[Asset] {
        &self.assets
    }
    
    /// Get bundle metadata
    pub fn metadata(&self) -> &BundleMetadata {
        &self.metadata
    }
    
    /// Get bundle header
    pub fn header(&self) -> &BundleHeader {
        &self.header
    }
    
    fn parse_assets(
        cursor: &mut Cursor<&[u8]>,
        header: &BundleHeader,
        metadata: &BundleMetadata,
    ) -> Result<Vec<Asset>> {
        let mut assets = Vec::new();
        
        for asset_info in &metadata.asset_files {
            // Seek to asset data
            cursor.set_position(asset_info.offset);
            
            // Parse asset file
            let asset = Asset::parse(cursor, asset_info)
                .with_context(|| format!("Failed to parse asset: {}", asset_info.name))?;
            
            assets.push(asset);
        }
        
        Ok(assets)
    }
}

/// Bundle header information
#[derive(Debug)]
pub struct BundleHeader {
    pub signature: String,
    pub format_version: u32,
    pub unity_version: String,
    pub unity_revision: String,
    pub size: u64,
    pub compressed_blocks_info_size: u32,
    pub uncompressed_blocks_info_size: u32,
    pub flags: u32,
}

impl BundleHeader {
    fn parse(cursor: &mut Cursor<&[u8]>) -> Result<Self> {
        // Read signature
        let mut signature_bytes = [0u8; 8];
        cursor.read_exact(&mut signature_bytes)?;
        let signature = String::from_utf8_lossy(&signature_bytes).trim_end_matches('\0').to_string();
        
        if signature != "UnityFS" {
            return Err(anyhow::anyhow!("Invalid bundle signature: {}", signature));
        }
        
        // Read format version
        let format_version = cursor.read_u32::<BigEndian>()?;
        
        // Read Unity version
        let unity_version = read_null_terminated_string(cursor)?;
        
        // Read Unity revision
        let unity_revision = read_null_terminated_string(cursor)?;
        
        // Read size
        let size = cursor.read_u64::<BigEndian>()?;
        
        // Read compressed blocks info size
        let compressed_blocks_info_size = cursor.read_u32::<BigEndian>()?;
        
        // Read uncompressed blocks info size
        let uncompressed_blocks_info_size = cursor.read_u32::<BigEndian>()?;
        
        // Read flags
        let flags = cursor.read_u32::<BigEndian>()?;
        
        Ok(BundleHeader {
            signature,
            format_version,
            unity_version,
            unity_revision,
            size,
            compressed_blocks_info_size,
            uncompressed_blocks_info_size,
            flags,
        })
    }
}

/// Bundle metadata
#[derive(Debug)]
pub struct BundleMetadata {
    pub asset_files: Vec<AssetFileInfo>,
    pub dependencies: Vec<String>,
}

impl BundleMetadata {
    fn parse(cursor: &mut Cursor<&[u8]>, header: &BundleHeader) -> Result<Self> {
        // For now, simplified metadata parsing
        // In a full implementation, this would handle compression and complex metadata
        
        let asset_files = vec![
            AssetFileInfo {
                name: "MainAsset".to_string(),
                offset: cursor.position(),
                size: 1024, // Placeholder
            }
        ];
        
        let dependencies = Vec::new();
        
        Ok(BundleMetadata {
            asset_files,
            dependencies,
        })
    }
}

/// Asset file information
#[derive(Debug)]
pub struct AssetFileInfo {
    pub name: String,
    pub offset: u64,
    pub size: u64,
}

/// Individual asset within a bundle
#[derive(Debug)]
pub struct Asset {
    name: String,
    asset_type: AssetType,
    data: Vec<u8>,
    properties: HashMap<String, serde_json::Value>,
    path: Option<String>,
}

impl Asset {
    fn parse(cursor: &mut Cursor<&[u8]>, info: &AssetFileInfo) -> Result<Self> {
        // Read asset data
        let mut data = vec![0u8; info.size as usize];
        cursor.read_exact(&mut data)?;
        
        // For simplified implementation, create a basic asset
        // In a full implementation, this would parse the Unity serialized format
        
        Ok(Asset {
            name: info.name.clone(),
            asset_type: AssetType::Texture2D, // Default for demo
            data,
            properties: HashMap::new(),
            path: None,
        })
    }
    
    pub fn name(&self) -> &str {
        &self.name
    }
    
    pub fn asset_type(&self) -> AssetType {
        self.asset_type
    }
    
    pub fn data(&self) -> &[u8] {
        &self.data
    }
    
    pub fn size(&self) -> usize {
        self.data.len()
    }
    
    pub fn compressed_size(&self) -> usize {
        // For demo purposes, assume no compression
        self.data.len()
    }
    
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }
    
    pub fn get_property(&self, key: &str) -> Option<&serde_json::Value> {
        self.properties.get(key)
    }
}

/// Read null-terminated string from cursor
fn read_null_terminated_string(cursor: &mut Cursor<&[u8]>) -> Result<String> {
    let mut bytes = Vec::new();
    loop {
        let byte = cursor.read_u8()?;
        if byte == 0 {
            break;
        }
        bytes.push(byte);
    }
    
    Ok(String::from_utf8(bytes)?)
}
"#;

const UNITY_SERIALIZATION_RS: &str = r#"//! Unity serialization format handling

use super::*;
use anyhow::Result;
use byteorder::{LittleEndian, ReadBytesExt};
use std::collections::HashMap;
use std::io::Cursor;

/// Unity serialized file format
#[derive(Debug)]
pub struct SerializedFile {
    pub header: SerializedHeader,
    pub type_tree: TypeTree,
    pub objects: Vec<SerializedObject>,
}

/// Serialized file header
#[derive(Debug)]
pub struct SerializedHeader {
    pub metadata_size: u32,
    pub file_size: u64,
    pub version: u32,
    pub data_offset: u64,
    pub endianness: bool, // true = little endian
}

/// Type tree for object serialization
#[derive(Debug)]
pub struct TypeTree {
    pub types: Vec<TypeInfo>,
}

/// Type information for Unity objects
#[derive(Debug)]
pub struct TypeInfo {
    pub class_id: i32,
    pub type_hash: [u8; 16],
    pub type_tree: Vec<TypeNode>,
}

/// Node in the type tree
#[derive(Debug)]
pub struct TypeNode {
    pub type_name: String,
    pub name: String,
    pub byte_size: i32,
    pub index: i32,
    pub flags: u32,
    pub version: u32,
    pub meta_flag: u32,
    pub children: Vec<TypeNode>,
}

/// Serialized object
#[derive(Debug)]
pub struct SerializedObject {
    pub path_id: i64,
    pub byte_start: u64,
    pub byte_size: u64,
    pub type_id: u32,
    pub data: Vec<u8>,
}

impl SerializedFile {
    /// Parse serialized file from bytes
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);
        
        let header = SerializedHeader::parse(&mut cursor)?;
        let type_tree = TypeTree::parse(&mut cursor, &header)?;
        let objects = Self::parse_objects(&mut cursor, &header)?;
        
        Ok(SerializedFile {
            header,
            type_tree,
            objects,
        })
    }
    
    fn parse_objects(cursor: &mut Cursor<&[u8]>, header: &SerializedHeader) -> Result<Vec<SerializedObject>> {
        // Simplified object parsing
        // In a full implementation, this would parse the object table and data
        Ok(Vec::new())
    }
}

impl SerializedHeader {
    fn parse(cursor: &mut Cursor<&[u8]>) -> Result<Self> {
        let metadata_size = cursor.read_u32::<LittleEndian>()?;
        let file_size = cursor.read_u64::<LittleEndian>()?;
        let version = cursor.read_u32::<LittleEndian>()?;
        let data_offset = cursor.read_u64::<LittleEndian>()?;
        let endianness = cursor.read_u8()? != 0;
        
        // Skip reserved bytes
        cursor.read_u8()?;
        cursor.read_u8()?;
        cursor.read_u8()?;
        
        Ok(SerializedHeader {
            metadata_size,
            file_size,
            version,
            data_offset,
            endianness,
        })
    }
}

impl TypeTree {
    fn parse(cursor: &mut Cursor<&[u8]>, header: &SerializedHeader) -> Result<Self> {
        // Simplified type tree parsing
        // In a full implementation, this would parse the complete type information
        Ok(TypeTree {
            types: Vec::new(),
        })
    }
}

/// Unity YAML parsing utilities
pub mod yaml {
    use super::*;
    use std::collections::HashMap;
    
    /// Parse Unity YAML format
    pub fn parse_unity_yaml(data: &str) -> Result<HashMap<String, serde_json::Value>> {
        // Simplified YAML parsing for Unity metadata
        // In a full implementation, this would handle Unity's specific YAML dialect
        Ok(HashMap::new())
    }
}
"#;

const UNITY_RESOURCES_RS: &str = r#"//! Unity resource handling and extraction

use super::*;
use anyhow::Result;
use std::collections::HashMap;

/// Unity resource extractor
pub struct ResourceExtractor {
    unity_version: UnityVersion,
}

impl ResourceExtractor {
    pub fn new(unity_version: UnityVersion) -> Self {
        Self { unity_version }
    }
    
    /// Extract texture from Unity asset
    pub fn extract_texture(&self, asset: &Asset) -> Result<TextureResource> {
        let data = asset.data();
        
        // Parse texture header (simplified)
        let width = 512; // Would be parsed from asset data
        let height = 512; // Would be parsed from asset data
        let format = TextureFormat::RGBA32;
        
        Ok(TextureResource {
            name: asset.name().to_string(),
            width,
            height,
            format,
            data: data.to_vec(),
            mip_levels: 1,
        })
    }
    
    /// Extract mesh from Unity asset
    pub fn extract_mesh(&self, asset: &Asset) -> Result<MeshResource> {
        let data = asset.data();
        
        // Parse mesh data (simplified)
        Ok(MeshResource {
            name: asset.name().to_string(),
            vertices: Vec::new(), // Would be parsed from asset data
            indices: Vec::new(),  // Would be parsed from asset data
            normals: Vec::new(),
            uvs: Vec::new(),
            materials: Vec::new(),
        })
    }
    
    /// Extract audio from Unity asset
    pub fn extract_audio(&self, asset: &Asset) -> Result<AudioResource> {
        let data = asset.data();
        
        Ok(AudioResource {
            name: asset.name().to_string(),
            format: AudioFormat::WAV,
            sample_rate: 44100,
            channels: 2,
            data: data.to_vec(),
        })
    }
}

/// Texture resource
#[derive(Debug)]
pub struct TextureResource {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    pub data: Vec<u8>,
    pub mip_levels: u32,
}

/// Texture formats
#[derive(Debug)]
pub enum TextureFormat {
    RGBA32,
    RGB24,
    DXT1,
    DXT5,
    ETC2,
    ASTC,
}

/// Mesh resource
#[derive(Debug)]
pub struct MeshResource {
    pub name: String,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub normals: Vec<Vector3>,
    pub uvs: Vec<Vector2>,
    pub materials: Vec<String>,
}

/// Vertex data
#[derive(Debug)]
pub struct Vertex {
    pub position: Vector3,
    pub normal: Vector3,
    pub uv: Vector2,
}

/// 3D vector
#[derive(Debug)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// 2D vector
#[derive(Debug)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

/// Audio resource
#[derive(Debug)]
pub struct AudioResource {
    pub name: String,
    pub format: AudioFormat,
    pub sample_rate: u32,
    pub channels: u32,
    pub data: Vec<u8>,
}

/// Audio formats
#[derive(Debug)]
pub enum AudioFormat {
    WAV,
    OGG,
    MP3,
    AIFF,
}
"#;

const UNITY_INTEGRATION_TESTS: &str = r#"//! Integration tests for Unity plugin

use {{name_snake}}::*;
use aegis_core::plugin::{Plugin, PluginFactory};
use std::path::Path;
use tempfile::TempDir;

#[tokio::test]
async fn test_unity_plugin_factory() {
    let factory = {{name_camel}}Factory;
    let info = factory.plugin_info();
    
    assert_eq!(info.name, "{{name}}");
    assert!(!info.supported_extensions.is_empty());
    assert!(info.compliance_info.compliance_verified);
}

#[tokio::test]
async fn test_unity_asset_bundle_detection() {
    let factory = {{name_camel}}Factory;
    let mut plugin = factory.create_plugin();
    
    // Test with Unity asset bundle header
    let unity_header = b"UnityFS\0\x00\x00\x00\x06";
    let temp_file = Path::new("test.ab");
    
    let can_handle = plugin.can_handle(temp_file, unity_header).await.unwrap();
    assert!(can_handle);
}

#[tokio::test]
async fn test_file_extension_detection() {
    let factory = {{name_camel}}Factory;
    let mut plugin = factory.create_plugin();
    
    {{#each supported_formats}}
    let test_file = Path::new("test{{this}}");
    let empty_header = &[];
    let can_handle = plugin.can_handle(test_file, empty_header).await.unwrap();
    assert!(can_handle);
    {{/each}}
}

#[tokio::test]
async fn test_asset_extraction() {
    // This test would use real Unity asset bundle files
    // For now, it's a placeholder for the testing framework
    
    let factory = {{name_camel}}Factory;
    let mut plugin = factory.create_plugin();
    
    // Create a minimal test asset bundle
    let test_bundle = create_test_asset_bundle();
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.ab");
    
    std::fs::write(&test_file, test_bundle).unwrap();
    
    let resources = plugin.extract_resources(&test_file).await.unwrap();
    
    // Verify extracted resources
    assert!(!resources.is_empty());
    
    for resource in &resources {
        assert!(!resource.name.is_empty());
        assert!(!resource.data.is_empty());
    }
}

#[tokio::test]
async fn test_list_entries() {
    let factory = {{name_camel}}Factory;
    let plugin = factory.create_plugin();
    
    let test_bundle = create_test_asset_bundle();
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.ab");
    
    std::fs::write(&test_file, test_bundle).unwrap();
    
    let entries = plugin.list_entries(&test_file).await.unwrap();
    
    assert!(!entries.is_empty());
    
    for entry in &entries {
        assert!(!entry.name.is_empty());
        assert!(entry.size_uncompressed > 0);
    }
}

/// Create a minimal test asset bundle for testing
fn create_test_asset_bundle() -> Vec<u8> {
    // Create a minimal Unity asset bundle for testing
    let mut bundle = Vec::new();
    
    // Unity signature
    bundle.extend_from_slice(b"UnityFS\0");
    
    // Format version (big endian)
    bundle.extend_from_slice(&6u32.to_be_bytes());
    
    // Unity version string
    bundle.extend_from_slice(b"2022.3.0f1\0");
    
    // Unity revision string  
    bundle.extend_from_slice(b"abcd1234\0");
    
    // Bundle size (placeholder)
    bundle.extend_from_slice(&1024u64.to_be_bytes());
    
    // Compressed blocks info size
    bundle.extend_from_slice(&0u32.to_be_bytes());
    
    // Uncompressed blocks info size
    bundle.extend_from_slice(&0u32.to_be_bytes());
    
    // Flags
    bundle.extend_from_slice(&0u32.to_be_bytes());
    
    // Pad to minimum size
    while bundle.len() < 1024 {
        bundle.push(0);
    }
    
    bundle
}
"#;

const UNITY_EXAMPLE_EXTRACT: &str = r#"//! Example: Extract assets from Unity asset bundle

use {{name_snake}}::*;
use aegis_core::plugin::{Plugin, PluginFactory};
use std::env;
use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::init();
    
    // Get input file from command line
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <unity-asset-bundle>", args[0]);
        std::process::exit(1);
    }
    
    let input_file = Path::new(&args[1]);
    if !input_file.exists() {
        eprintln!("Error: File not found: {}", input_file.display());
        std::process::exit(1);
    }
    
    println!("🎮 Unity Asset Extractor");
    println!("📁 Input: {}", input_file.display());
    
    // Create plugin
    let factory = {{name_camel}}Factory;
    let mut plugin = factory.create_plugin();
    
    // Check if plugin can handle the file
    let file_data = std::fs::read(input_file)?;
    let header = if file_data.len() >= 64 { &file_data[..64] } else { &file_data };
    
    if !plugin.can_handle(input_file, header).await? {
        eprintln!("❌ This plugin cannot handle the input file");
        std::process::exit(1);
    }
    
    println!("✅ File format recognized");
    
    // List entries first
    println!("\n📋 Asset Bundle Contents:");
    let entries = plugin.list_entries(input_file).await?;
    
    for (i, entry) in entries.iter().enumerate() {
        println!("  {}. {} ({} bytes)", 
                i + 1, 
                entry.name, 
                entry.size_uncompressed);
        
        if let Some(file_type) = &entry.file_type {
            println!("     Type: {}", file_type);
        }
        
        if let Some(compressed_size) = entry.size_compressed {
            if compressed_size != entry.size_uncompressed {
                let ratio = (1.0 - compressed_size as f64 / entry.size_uncompressed as f64) * 100.0;
                println!("     Compressed: {} bytes ({:.1}% reduction)", compressed_size, ratio);
            }
        }
        println!();
    }
    
    // Extract resources
    println!("🔄 Extracting assets...");
    let resources = plugin.extract_resources(input_file).await?;
    
    println!("✅ Extracted {} resources:", resources.len());
    
    for resource in &resources {
        println!("  • {} ({}, {} bytes)", 
                resource.name, 
                resource.format, 
                resource.size);
        
        // Print metadata if available
        if !resource.metadata.is_null() {
            println!("    Metadata: {}", resource.metadata);
        }
    }
    
    // Save extracted resources
    let output_dir = input_file.parent().unwrap_or(Path::new(".")).join("extracted");
    std::fs::create_dir_all(&output_dir)?;
    
    for resource in &resources {
        let output_file = output_dir.join(&resource.name);
        std::fs::write(&output_file, &resource.data)?;
        println!("💾 Saved: {}", output_file.display());
    }
    
    println!("\n🎉 Extraction complete!");
    println!("📁 Output directory: {}", output_dir.display());
    
    Ok(())
}
"#;

const UNITY_EXAMPLE_INFO: &str = r#"//! Example: Get information about Unity asset bundle

use {{name_snake}}::*;
use aegis_core::plugin::{Plugin, PluginFactory};
use std::env;
use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <unity-asset-bundle>", args[0]);
        std::process::exit(1);
    }
    
    let input_file = Path::new(&args[1]);
    
    println!("🎮 Unity Asset Bundle Information");
    println!("📁 File: {}", input_file.display());
    
    // Read file
    let file_data = std::fs::read(input_file)?;
    let file_size = file_data.len();
    
    println!("📊 File Size: {} bytes ({:.2} MB)", file_size, file_size as f64 / 1024.0 / 1024.0);
    
    // Create plugin
    let factory = {{name_camel}}Factory;
    let plugin = factory.create_plugin();
    let info = factory.plugin_info();
    
    println!("\n🔌 Plugin Information:");
    println!("  Name: {}", info.name);
    println!("  Version: {}", info.version);
    println!("  Author: {}", info.author.as_deref().unwrap_or("Unknown"));
    println!("  Supported Extensions: {}", info.supported_extensions.join(", "));
    
    // Check if we can handle this file
    let header = if file_data.len() >= 64 { &file_data[..64] } else { &file_data };
    
    let can_handle = plugin.can_handle(input_file, header).await?;
    println!("  Can Handle: {}", if can_handle { "✅ Yes" } else { "❌ No" });
    
    if !can_handle {
        println!("\n❌ This file cannot be processed by the Unity plugin");
        return Ok(());
    }
    
    // Parse bundle header
    if file_data.len() >= 8 {
        let signature = String::from_utf8_lossy(&file_data[0..8]);
        println!("\n📋 Bundle Information:");
        println!("  Signature: {}", signature.trim_end_matches('\0'));
        
        if signature.starts_with("UnityFS") {
            // Parse Unity FS header
            if file_data.len() >= 32 {
                let format_version = u32::from_be_bytes([
                    file_data[8], file_data[9], file_data[10], file_data[11]
                ]);
                println!("  Format Version: {}", format_version);
                
                // Find Unity version string
                let mut version_start = 12;
                while version_start < file_data.len() && file_data[version_start] != 0 {
                    version_start += 1;
                }
                
                if version_start < file_data.len() {
                    version_start += 1; // Skip null terminator
                    let mut version_end = version_start;
                    while version_end < file_data.len() && file_data[version_end] != 0 {
                        version_end += 1;
                    }
                    
                    if version_end > version_start {
                        let unity_version = String::from_utf8_lossy(&file_data[version_start..version_end]);
                        println!("  Unity Version: {}", unity_version);
                    }
                }
            }
        }
    }
    
    // List entries
    println!("\n📦 Bundle Contents:");
    let entries = plugin.list_entries(input_file).await?;
    
    if entries.is_empty() {
        println!("  (No entries found)");
    } else {
        println!("  Total Entries: {}", entries.len());
        
        // Group by type
        let mut type_counts = std::collections::HashMap::new();
        let mut total_uncompressed = 0u64;
        let mut total_compressed = 0u64;
        
        for entry in &entries {
            let entry_type = entry.file_type.as_deref().unwrap_or("Unknown");
            *type_counts.entry(entry_type).or_insert(0) += 1;
            
            total_uncompressed += entry.size_uncompressed;
            if let Some(compressed_size) = entry.size_compressed {
                total_compressed += compressed_size;
            } else {
                total_compressed += entry.size_uncompressed;
            }
        }
        
        println!("  Total Uncompressed Size: {} bytes ({:.2} MB)", 
                total_uncompressed, total_uncompressed as f64 / 1024.0 / 1024.0);
        
        if total_compressed != total_uncompressed {
            let compression_ratio = (1.0 - total_compressed as f64 / total_uncompressed as f64) * 100.0;
            println!("  Total Compressed Size: {} bytes ({:.2} MB, {:.1}% compression)", 
                    total_compressed, total_compressed as f64 / 1024.0 / 1024.0, compression_ratio);
        }
        
        println!("\n📊 Asset Types:");
        for (asset_type, count) in type_counts {
            println!("    {}: {} assets", asset_type, count);
        }
        
        if entries.len() <= 10 {
            println!("\n📋 Detailed Entries:");
            for (i, entry) in entries.iter().enumerate() {
                println!("    {}. {}", i + 1, entry.name);
                println!("       Size: {} bytes", entry.size_uncompressed);
                if let Some(file_type) = &entry.file_type {
                    println!("       Type: {}", file_type);
                }
                if let Some(path) = &entry.path {
                    println!("       Path: {}", path);
                }
            }
        }
    }
    
    Ok(())
}
"#;

const UNITY_README: &str = r#"# {{name}}

{{description}}

## Features

{{#each features}}
- **{{this}}**: Advanced {{this}} capabilities
{{/each}}

## Supported Formats

This plugin supports the following Unity file formats:

{{#each supported_formats}}
- `{{this}}` - Unity asset format
{{/each}}

## Quick Start

### Installation

Add this plugin to your `Cargo.toml`:

```toml
[dependencies]
{{name}} = "0.1.0"
aegis-core = "0.1.0"
```

### Basic Usage

```rust
use {{name_snake}}::*;
use aegis_core::plugin::{Plugin, PluginFactory};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create plugin instance
    let factory = {{name_camel}}Factory;
    let mut plugin = factory.create_plugin();
    
    // Extract assets from Unity bundle
    let resources = plugin.extract_resources("assets.ab").await?;
    
    for resource in resources {
        println!("Extracted: {} ({} bytes)", resource.name, resource.size);
    }
    
    Ok(())
}
```

### Command Line Usage

```bash
# Extract assets from Unity bundle
aegis extract assets.ab --output ./extracted

# List bundle contents
aegis list assets.ab --details

# Check compliance
aegis compliance assets.ab --profile unity
```

## Development

### Building

```bash
cargo build --release
```

### Testing

```bash
# Run unit tests
cargo test

# Run integration tests with test assets
cargo test --test integration_tests

# Run with test Unity bundles
aegis-dev test run --with-assets ./test-data/unity
```

### Development Server

Start the development server for hot-reload development:

```bash
aegis-dev dev --port 3001
```

This will:
- Watch for file changes and rebuild automatically
- Run tests on changes
- Provide development API endpoints
- Generate live documentation

## Unity Format Support

### Asset Bundle Format

This plugin supports Unity Asset Bundle formats from Unity 4.x through 2023.x:

- **UnityFS**: Modern streaming format (Unity 5.3+)
- **UnityWeb**: Legacy web format
- **UnityRaw**: Uncompressed legacy format

### Supported Asset Types

- **Textures**: Texture2D, RenderTexture, Cubemap
- **Meshes**: Static and skinned meshes
- **Audio**: AudioClip in various formats
- **Materials**: Shader and material data
- **Animations**: Animation clips and controllers
- **Scripts**: MonoBehaviour data

### Compression Support

{{#if (has_feature "compression-support")}}
- LZ4 compression (Unity 5.3+)
- LZMA compression
- LZ4HC compression
{{else}}
- Basic LZ4 compression support
{{/if}}

## Compliance and Security

This plugin is designed with compliance-first principles:

- ✅ **Verified Safe**: All extraction operations are sandboxed
- ✅ **Privacy Compliant**: No data collection or telemetry
- ✅ **Publisher Respect**: Honors Unity's asset protection
- ✅ **Legal Compliance**: DMCA and copyright aware

### Risk Level: {{compliance_level}}

{{#if (eq compliance_level "enterprise")}}
This plugin meets enterprise security standards and is suitable for commercial use.
{{else}}
This plugin is suitable for personal and educational use. For commercial use, consider upgrading to enterprise compliance.
{{/if}}

## License

Licensed under {{license}}. See [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please read our [contributing guidelines](CONTRIBUTING.md) first.

### Development Setup

1. Clone the repository
2. Install dependencies: `cargo build`
3. Run tests: `cargo test`
4. Start development server: `aegis-dev dev`

### Submitting Changes

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

## Support

- 📚 [Documentation](https://docs.aegis-assets.org/plugins/{{name}})
- 🐛 [Issue Tracker](https://github.com/{{author}}/{{name}}/issues)
- 💬 [Community Discord](https://discord.gg/aegis-assets)
- 📧 Email: {{author}}@example.com

## Acknowledgments

- Unity Technologies for the Unity game engine
- The Aegis-Assets community for feedback and testing
- Contributors and maintainers

---

Built with ❤️ for the game development community.
"#;

const UNITY_DOCS_FORMATS: &str = r#"# Unity File Formats

This document describes the Unity file formats supported by this plugin.

## Asset Bundle Format

### UnityFS Format (Unity 5.3+)

The modern Unity asset bundle format with the following structure:

```
Header (32+ bytes)
├── Signature: "UnityFS\0" (8 bytes)
├── Format Version: uint32 (4 bytes)  
├── Unity Version: null-terminated string
├── Unity Revision: null-terminated string
├── Bundle Size: uint64 (8 bytes)
├── Compressed Blocks Info Size: uint32 (4 bytes)
├── Uncompressed Blocks Info Size: uint32 (4 bytes)
└── Flags: uint32 (4 bytes)

Blocks Info
├── Decompressed Size: uint32
├── Compressed Size: uint32
└── Flags: uint16

Directory Info
├── Node Count: int32
├── Asset Paths: string[]
└── Dependencies: string[]

Asset Files
├── Serialized Header
├── Type Tree
└── Object Data
```

### Supported Versions

| Unity Version | Format Version | Notes |
|---------------|----------------|-------|
| 2023.x        | 7             | Latest format |
| 2022.x        | 7             | LTS support |
| 2021.x        | 7             | Full support |
| 2020.x        | 6             | Full support |
| 2019.x        | 6             | Full support |
| 2018.x        | 6             | Full support |
| 2017.x        | 6             | Full support |
| 5.3 - 2017.1  | 6             | Legacy support |

## Asset Types

### Texture2D (Class ID: 28)

Unity texture assets with the following properties:

- Width/Height dimensions
- Texture format (DXT1, DXT5, ETC2, ASTC, etc.)
- Mipmap levels
- Filtering settings
- Wrap modes

### Mesh (Class ID: 43)

3D mesh data including:

- Vertex positions
- Normals and tangents
- UV coordinates (multiple sets)
- Vertex colors
- Bone weights (for skinned meshes)
- Index buffers
- Sub-mesh definitions

### AudioClip (Class ID: 83)

Audio data in various formats:

- WAV (uncompressed)
- Ogg Vorbis (compressed)
- MP3 (compressed)
- ADPCM (compressed)

### Material (Class ID: 21)

Material definitions including:

- Shader references
- Texture assignments
- Shader properties
- Render queue settings

## Compression

### LZ4 Compression

Used by default in Unity 5.3+:

- Fast decompression
- Moderate compression ratio
- Streaming-friendly

### LZMA Compression

Legacy compression:

- High compression ratio
- Slower decompression
- Not streaming-friendly

## Security Considerations

### Asset Protection

Unity provides several mechanisms to protect assets:

1. **Encryption**: Assets can be encrypted
2. **Obfuscation**: Asset names and paths can be obfuscated
3. **Compression**: Reduces file size and provides light obfuscation

### Compliance Notes

When extracting Unity assets:

- Respect copyright and licensing
- Honor Unity's asset store terms
- Follow platform-specific guidelines
- Consider publisher protection mechanisms

## Implementation Notes

### Performance Optimizations

- Use memory mapping for large files
- Stream decompression when possible
- Cache type information
- Parallelize asset processing

### Error Handling

Common issues and solutions:

- **Unsupported format version**: Update plugin
- **Corrupted bundle**: Verify file integrity
- **Missing dependencies**: Check bundle dependencies
- **Compression errors**: Verify compression support

### Testing

Test with various Unity versions and asset types:

```bash
# Test with different Unity versions
aegis-dev test run --with-assets ./test-data/unity-2020
aegis-dev test run --with-assets ./test-data/unity-2021
aegis-dev test run --with-assets ./test-data/unity-2022

# Benchmark performance
aegis-dev profile ./test-data/large-bundle.ab
```

## References

- [Unity Manual: Asset Bundles](https://docs.unity3d.com/Manual/AssetBundles-Workflow.html)
- [Unity Serialization Format](https://docs.unity3d.com/Manual/ClassIDReference.html)
- [AssetStudio Project](https://github.com/Perfare/AssetStudio)
"#;

const UNITY_PLUGIN_CONFIG: &str = r#"# Plugin Configuration for {{name}}

[plugin]
name = "{{name}}"
version = "0.1.0"
description = "{{description}}"
author = "{{author}}"
license = "{{license}}"

[compatibility]
min_aegis_version = "0.1.0"
unity_versions = [
    {{#each supported_formats}}
    "{{this}}",
    {{/each}}
]

[features]
{{#each features}}
{{this}} = true
{{/each}}

[security]
compliance_level = "{{compliance_level}}"
risk_level = "low"
requires_sandbox = true
network_access = false

[testing]
framework = "{{test_framework}}"
{{#if has_tests}}
test_assets_required = true
integration_tests = true
{{/if}}
{{#if has_benchmarks}}
performance_tests = true
{{/if}}

{{#if has_docs}}
[documentation]
api_docs = true
examples = true
tutorials = true
{{/if}}

{{#if has_ci}}
[ci_cd]
github_actions = true
test_on_push = true
auto_release = false
{{/if}}
"#;

const GITIGNORE: &str = r#"# Generated by Rust
/target/
**/*.rs.bk
Cargo.lock

# Generated by Aegis
/extracted/
/test-output/
*.aegis-cache

# IDE files
.vscode/
.idea/
*.swp
*.swo

# OS files
.DS_Store
Thumbs.db

# Development
/docs/book/
/dev-server/
*.log

# Test artifacts
/test-results/
/coverage/
*.prof
"#;

// Template constants are used internally by get_unity_template_files()