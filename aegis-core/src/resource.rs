use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Type of resource extracted from a game archive
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceType {
    Texture,
    Mesh,
    Material,
    Animation,
    Audio,
    Level,
    Script,
    Generic,
}

/// A generic resource extracted from a game archive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub id: String,
    pub name: String,
    pub resource_type: ResourceType,
    pub size: u64,
    pub format: String,
    pub metadata: HashMap<String, String>,
    pub outputs: ResourceOutputs,
}

/// Physical outputs generated during extraction/conversion for a resource
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceOutputs {
    /// Path to the raw bytes written to disk
    pub raw: Option<PathBuf>,
    /// Paths to any converted representations written to disk
    pub converted: Vec<PathBuf>,
}

/// Texture resource with specific properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureResource {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    pub data: Vec<u8>,
    pub mip_levels: u8,
    pub usage_hint: Option<TextureUsage>,
}

/// Mesh resource with geometry data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshResource {
    pub name: String,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub material_id: Option<String>,
    pub bone_weights: Option<Vec<BoneWeight>>,
}

/// Material resource defining surface properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialResource {
    pub name: String,
    pub shader: String,
    pub textures: HashMap<String, String>, // slot_name -> texture_id
    pub properties: HashMap<String, MaterialProperty>,
    pub blend_mode: BlendMode,
}

/// Animation resource with keyframe data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationResource {
    pub name: String,
    pub duration_seconds: f32,
    pub bone_tracks: Vec<BoneTrack>,
    pub loop_mode: LoopMode,
}

/// Audio resource with sound data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioResource {
    pub name: String,
    pub format: AudioFormat,
    pub data: Vec<u8>,
    pub sample_rate: u32,
    pub channels: u8,
    pub duration_seconds: f32,
}

/// Level/Scene resource with world data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelResource {
    pub name: String,
    pub objects: Vec<GameObject>,
    pub lighting: LightingInfo,
    pub terrain: Option<TerrainData>,
}

/// Supported texture formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextureFormat {
    RGBA8,
    RGB8,
    RGBA16,
    DXT1,
    DXT3,
    DXT5,
    BC7,
    ETC2,
    ASTC,
}

/// Texture usage hints for optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextureUsage {
    Albedo,
    Normal,
    Roughness,
    Metallic,
    Emission,
    Occlusion,
    UI,
    Lightmap,
}

/// Material blend modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendMode {
    Opaque,
    AlphaBlend,
    Additive,
    Multiply,
}

/// Animation loop modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopMode {
    Once,
    Loop,
    PingPong,
}

/// Audio formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioFormat {
    PCM,
    MP3,
    OGG,
    WAV,
    FLAC,
}

/// Vertex data for mesh geometry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: Option<[f32; 3]>,
    pub uv: Option<[f32; 2]>,
    pub color: Option<[f32; 4]>,
}

/// Bone weight for skeletal animation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoneWeight {
    pub bone_index: u32,
    pub weight: f32,
}

/// Animation track for a single bone
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoneTrack {
    pub bone_name: String,
    pub position_keys: Vec<PositionKey>,
    pub rotation_keys: Vec<RotationKey>,
    pub scale_keys: Vec<ScaleKey>,
}

/// Position keyframe
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionKey {
    pub time: f32,
    pub position: [f32; 3],
}

/// Rotation keyframe
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationKey {
    pub time: f32,
    pub rotation: [f32; 4], // quaternion
}

/// Scale keyframe
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaleKey {
    pub time: f32,
    pub scale: [f32; 3],
}

/// Material property value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MaterialProperty {
    Float(f32),
    Vector2([f32; 2]),
    Vector3([f32; 3]),
    Vector4([f32; 4]),
    Color([f32; 4]),
    Texture(String),
}

/// Game object in a level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameObject {
    pub id: String,
    pub name: String,
    pub transform: Transform,
    pub mesh_id: Option<String>,
    pub material_ids: Vec<String>,
    pub components: Vec<Component>,
}

/// 3D transformation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transform {
    pub position: [f32; 3],
    pub rotation: [f32; 4], // quaternion
    pub scale: [f32; 3],
}

/// Generic component system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub type_name: String,
    pub properties: HashMap<String, serde_json::Value>,
}

/// Lighting information for a level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightingInfo {
    pub ambient_color: [f32; 3],
    pub lights: Vec<Light>,
    pub lightmaps: Vec<String>, // texture IDs
}

/// Light source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Light {
    pub light_type: LightType,
    pub position: [f32; 3],
    pub direction: Option<[f32; 3]>,
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: Option<f32>,
}

/// Types of lights
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightType {
    Directional,
    Point,
    Spot,
    Area,
}

/// Terrain height and texture data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainData {
    pub height_map: Vec<f32>,
    pub width: u32,
    pub height: u32,
    pub scale: [f32; 3],
    pub texture_layers: Vec<TerrainLayer>,
}

/// Terrain texture layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainLayer {
    pub texture_id: String,
    pub tiling: [f32; 2],
    pub blend_map: Option<Vec<u8>>,
}

impl Resource {
    /// Create a new generic resource
    pub fn new(
        id: String,
        name: String,
        resource_type: ResourceType,
        size: u64,
        format: String,
    ) -> Self {
        Self {
            id,
            name,
            resource_type,
            size,
            format,
            metadata: HashMap::new(),
            outputs: ResourceOutputs::default(),
        }
    }

    /// Add metadata to the resource
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Associate the raw output path for this resource
    pub fn set_raw_path<P: Into<PathBuf>>(&mut self, path: P) {
        self.outputs.raw = Some(path.into());
    }

    /// Register a converted output path for this resource
    pub fn add_converted_path<P: Into<PathBuf>>(&mut self, path: P) {
        self.outputs.converted.push(path.into());
    }

    /// Get the string representation of the resource type for display/logging
    pub fn resource_type(&self) -> String {
        format!("{:?}", self.resource_type)
    }

    /// Retrieve the raw output path if available
    pub fn raw_output_path(&self) -> Option<&Path> {
        self.outputs.raw.as_deref()
    }

    /// Retrieve any converted output paths written for this resource
    pub fn converted_output_paths(&self) -> &[PathBuf] {
        &self.outputs.converted
    }
}
