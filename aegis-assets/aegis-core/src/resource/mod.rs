use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Different types of game assets that can be extracted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Resource {
    /// 3D model/mesh data
    Mesh(MeshResource),
    /// Texture/image data
    Texture(TextureResource),
    /// Material definitions
    Material(MaterialResource),
    /// Skeletal animation data
    Animation(AnimationResource),
    /// Audio clips
    Audio(AudioResource),
    /// Level/scene data
    Level(LevelResource),
    /// Generic binary data
    Binary(BinaryResource),
    /// Text-based data (JSON, XML, scripts)
    Text(TextResource),
}

/// 3D mesh/model resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshResource {
    pub name: String,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub materials: Vec<String>, // Material references
    pub bounding_box: Option<BoundingBox>,
    pub skeleton: Option<SkeletonResource>,
}

/// Vertex data for 3D meshes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: Option<[f32; 3]>,
    pub uv: Option<[f32; 2]>,
    pub color: Option<[f32; 4]>,
    pub bone_weights: Option<[f32; 4]>,
    pub bone_indices: Option<[u16; 4]>,
}

/// Bounding box for spatial optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

/// Skeletal data for animated meshes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkeletonResource {
    pub bones: Vec<Bone>,
    pub root_bone: Option<usize>,
}

/// Bone definition in skeleton
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    pub transform: Transform,
    pub inverse_bind_matrix: Option<[f32; 16]>,
}

/// Transform matrix
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transform {
    pub translation: [f32; 3],
    pub rotation: [f32; 4], // Quaternion
    pub scale: [f32; 3],
}

/// Texture/image resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureResource {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    pub data: Vec<u8>,
    pub mip_levels: u32,
    pub usage_hint: Option<TextureUsage>,
}

/// Supported texture formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextureFormat {
    RGBA8,
    RGB8,
    DXT1,
    DXT5,
    BC1,
    BC3,
    BC7,
    ASTC,
    ETC2,
    Unknown(String),
}

/// Texture usage hints for better material mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextureUsage {
    Albedo,
    Normal,
    Metallic,
    Roughness,
    AmbientOcclusion,
    Emission,
    Height,
    UI,
    Lightmap,
    Unknown,
}

/// Material definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialResource {
    pub name: String,
    pub shader: Option<String>,
    pub textures: HashMap<String, String>, // slot -> texture name
    pub properties: HashMap<String, MaterialProperty>,
    pub render_queue: Option<i32>,
    pub blend_mode: Option<BlendMode>,
}

/// Material property values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MaterialProperty {
    Float(f32),
    Vector2([f32; 2]),
    Vector3([f32; 3]),
    Vector4([f32; 4]),
    Color([f32; 4]),
    Bool(bool),
    Integer(i32),
    String(String),
}

/// Blend modes for transparency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlendMode {
    Opaque,
    AlphaBlend,
    Additive,
    Multiply,
    Screen,
}

/// Animation clip data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationResource {
    pub name: String,
    pub duration: f32,
    pub frame_rate: f32,
    pub tracks: Vec<AnimationTrack>,
    pub loop_mode: LoopMode,
}

/// Individual animation track (bone/property animation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationTrack {
    pub target: String, // Bone name or property path
    pub property: AnimationProperty,
    pub keyframes: Vec<Keyframe>,
    pub interpolation: InterpolationMode,
}

/// Properties that can be animated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnimationProperty {
    Translation,
    Rotation,
    Scale,
    Color,
    Float,
}

/// Animation keyframe
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyframe {
    pub time: f32,
    pub value: KeyframeValue,
    pub tangent_in: Option<[f32; 3]>,
    pub tangent_out: Option<[f32; 3]>,
}

/// Keyframe values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyframeValue {
    Float(f32),
    Vector3([f32; 3]),
    Quaternion([f32; 4]),
    Color([f32; 4]),
}

/// Animation loop behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoopMode {
    Once,
    Loop,
    PingPong,
    ClampForever,
}

/// Interpolation methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InterpolationMode {
    Linear,
    Step,
    Cubic,
    Bezier,
}

/// Audio clip resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioResource {
    pub name: String,
    pub format: AudioFormat,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration: f32,
    pub data: Vec<u8>,
    pub loop_points: Option<(u32, u32)>,
}

/// Supported audio formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioFormat {
    WAV,
    OGG,
    MP3,
    FLAC,
    Unknown(String),
}

/// Level/scene data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelResource {
    pub name: String,
    pub objects: Vec<LevelObject>,
    pub lighting: Option<LightingData>,
    pub terrain: Option<TerrainData>,
    pub bounds: Option<BoundingBox>,
}

/// Object instance in a level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelObject {
    pub name: String,
    pub mesh: Option<String>,
    pub material: Option<String>,
    pub transform: Transform,
    pub properties: HashMap<String, serde_json::Value>,
}

/// Lighting information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightingData {
    pub ambient_color: [f32; 3],
    pub lights: Vec<Light>,
    pub lightmaps: Vec<String>,
}

/// Light definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Light {
    pub light_type: LightType,
    pub position: [f32; 3],
    pub direction: Option<[f32; 3]>,
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: Option<f32>,
    pub spot_angle: Option<f32>,
}

/// Light types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LightType {
    Directional,
    Point,
    Spot,
    Area,
}

/// Terrain data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainData {
    pub heightmap: Vec<f32>,
    pub width: u32,
    pub height: u32,
    pub scale: [f32; 3],
    pub textures: Vec<String>,
}

/// Generic binary resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryResource {
    pub name: String,
    pub mime_type: Option<String>,
    pub data: Vec<u8>,
}

/// Text-based resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextResource {
    pub name: String,
    pub content_type: TextContentType,
    pub content: String,
}

/// Text content types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextContentType {
    JSON,
    XML,
    YAML,
    Script,
    Shader,
    Plain,
}

impl Resource {
    /// Get the name of this resource
    pub fn name(&self) -> &str {
        match self {
            Resource::Mesh(r) => &r.name,
            Resource::Texture(r) => &r.name,
            Resource::Material(r) => &r.name,
            Resource::Animation(r) => &r.name,
            Resource::Audio(r) => &r.name,
            Resource::Level(r) => &r.name,
            Resource::Binary(r) => &r.name,
            Resource::Text(r) => &r.name,
        }
    }

    /// Get the resource type as a string
    pub fn resource_type(&self) -> &'static str {
        match self {
            Resource::Mesh(_) => "mesh",
            Resource::Texture(_) => "texture",
            Resource::Material(_) => "material",
            Resource::Animation(_) => "animation",
            Resource::Audio(_) => "audio",
            Resource::Level(_) => "level",
            Resource::Binary(_) => "binary",
            Resource::Text(_) => "text",
        }
    }

    /// Estimate memory usage in bytes
    pub fn estimated_memory_usage(&self) -> usize {
        match self {
            Resource::Texture(t) => t.data.len(),
            Resource::Audio(a) => a.data.len(),
            Resource::Binary(b) => b.data.len(),
            Resource::Mesh(m) => {
                m.vertices.len() * std::mem::size_of::<Vertex>() +
                m.indices.len() * std::mem::size_of::<u32>()
            }
            _ => 1024, // Rough estimate for other types
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_name_extraction() {
        let texture = Resource::Texture(TextureResource {
            name: "test_texture.png".to_string(),
            width: 256,
            height: 256,
            format: TextureFormat::RGBA8,
            data: vec![0; 256 * 256 * 4],
            mip_levels: 1,
            usage_hint: Some(TextureUsage::Albedo),
        });

        assert_eq!(texture.name(), "test_texture.png");
        assert_eq!(texture.resource_type(), "texture");
    }

    #[test]
    fn test_memory_usage_calculation() {
        let texture = Resource::Texture(TextureResource {
            name: "test".to_string(),
            width: 256,
            height: 256,
            format: TextureFormat::RGBA8,
            data: vec![0; 256 * 256 * 4], // 256KB
            mip_levels: 1,
            usage_hint: None,
        });

        assert_eq!(texture.estimated_memory_usage(), 256 * 256 * 4);
    }
}
