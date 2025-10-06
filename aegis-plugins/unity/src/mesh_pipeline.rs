use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde_json::json;
use std::fmt;

use crate::converters::UnityMesh;

/// Options controlling mesh conversion output.
#[derive(Debug, Clone)]
pub struct MeshPipelineOptions {
    /// Embed the binary buffers as base64 URIs inside the glTF JSON.
    pub embed_buffers: bool,
    /// Produce an OBJ fallback alongside the primary glTF artifact.
    pub generate_obj_fallback: bool,
    /// Attempt to run validation after generation (e.g. gltf-validator CLI).
    pub validate_output: bool,
}

impl Default for MeshPipelineOptions {
    fn default() -> Self {
        Self {
            embed_buffers: true,
            generate_obj_fallback: true,
            validate_output: false,
        }
    }
}

/// Primary conversion entry point returning artifacts and statistics.
#[derive(Debug, Clone)]
pub struct MeshConversionResult {
    pub primary: MeshArtifact,
    pub fallback: Option<MeshArtifact>,
    pub stats: MeshConversionStats,
    pub validation: MeshValidationReport,
}

/// Artifact produced by the mesh pipeline (e.g. glTF, OBJ, or binary buffer).
#[derive(Debug, Clone)]
pub struct MeshArtifact {
    pub filename: String,
    pub media_type: &'static str,
    pub bytes: Vec<u8>,
}

/// Basic mesh statistics captured during export for validation/telemetry.
#[derive(Debug, Clone, Default)]
pub struct MeshConversionStats {
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub has_normals: bool,
    pub has_uvs: bool,
    pub has_tangents: bool,
}

impl fmt::Display for MeshConversionStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "vertices: {}, triangles: {}, normals: {}, uvs: {}, tangents: {}",
            self.vertex_count,
            self.triangle_count,
            self.has_normals,
            self.has_uvs,
            self.has_tangents
        )
    }
}

/// Validation metadata describing whether post-processing checks ran.
#[derive(Debug, Clone)]
pub struct MeshValidationReport {
    pub status: MeshValidationStatus,
    pub details: Option<String>,
}

impl MeshValidationReport {
    pub fn not_run() -> Self {
        Self {
            status: MeshValidationStatus::NotRun,
            details: None,
        }
    }
}

/// Result of running optional validation step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshValidationStatus {
    NotRun,
    Passed,
    Failed,
}

/// Convert a Unity mesh into an interchange-friendly representation.
pub(crate) fn convert_unity_mesh(
    mesh: &UnityMesh,
    options: &MeshPipelineOptions,
) -> Result<MeshConversionResult> {
    let (gltf_bytes, stats) = build_embedded_gltf(mesh, options)?;

    let primary = MeshArtifact {
        filename: format!("{}.gltf", mesh.name),
        media_type: "model/gltf+json",
        bytes: gltf_bytes,
    };

    let fallback = if options.generate_obj_fallback {
        Some(MeshArtifact {
            filename: format!("{}.obj", mesh.name),
            media_type: "text/plain",
            bytes: build_wavefront_obj(mesh),
        })
    } else {
        None
    };

    let validation = if options.validate_output {
        MeshValidationReport {
            status: MeshValidationStatus::Failed,
            details: Some("External validator integration not implemented".to_string()),
        }
    } else {
        MeshValidationReport::not_run()
    };

    Ok(MeshConversionResult {
        primary,
        fallback,
        stats,
        validation,
    })
}

fn build_embedded_gltf(
    mesh: &UnityMesh,
    options: &MeshPipelineOptions,
) -> Result<(Vec<u8>, MeshConversionStats)> {
    let stats = MeshConversionStats {
        vertex_count: mesh.vertices.len(),
        triangle_count: mesh.triangles.len() / 3,
        has_normals: !mesh.normals.is_empty(),
        has_uvs: !mesh.uvs.is_empty(),
        has_tangents: !mesh.tangents.is_empty(),
    };

    if mesh.vertices.is_empty() {
        let gltf = json!({
            "asset": {
                "generator": "Aegis-Assets Unity Plugin",
                "version": "2.0"
            },
            "meshes": [],
            "nodes": [],
            "scenes": [{ "nodes": [] }],
            "scene": 0,
        });

        let bytes = serde_json::to_vec_pretty(&gltf).context("Failed to serialize glTF")?;
        return Ok((bytes, stats));
    }

    let mut mesh_json = json!({
        "name": mesh.name,
        "primitives": [
            {
                "mode": 4,
                "attributes": {
                    "POSITION": 0
                }
            }
        ]
    });

    if !mesh.triangles.is_empty() {
        mesh_json["primitives"][0]["indices"] = json!(1);
    }
    if !mesh.normals.is_empty() {
        mesh_json["primitives"][0]["attributes"]["NORMAL"] = json!(2);
    }
    if !mesh.uvs.is_empty() {
        mesh_json["primitives"][0]["attributes"]["TEXCOORD_0"] = json!(3);
    }
    if !mesh.tangents.is_empty() {
        mesh_json["primitives"][0]["attributes"]["TANGENT"] = json!(4);
    }

    let mut accessors = Vec::new();
    let mut buffer_views = Vec::new();
    let mut byte_offset = 0usize;

    accessors.push(json!({
        "bufferView": 0,
        "byteOffset": 0,
        "componentType": 5126,
        "count": mesh.vertices.len(),
        "type": "VEC3",
        "min": mesh.get_position_bounds().0,
        "max": mesh.get_position_bounds().1,
    }));

    buffer_views.push(json!({
        "buffer": 0,
        "byteOffset": byte_offset,
        "byteLength": mesh.vertices.len() * 12,
        "target": 34962,
    }));
    byte_offset += mesh.vertices.len() * 12;

    if !mesh.triangles.is_empty() {
        accessors.push(json!({
            "bufferView": 1,
            "byteOffset": 0,
            "componentType": 5125,
            "count": mesh.triangles.len(),
            "type": "SCALAR",
        }));

        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": byte_offset,
            "byteLength": mesh.triangles.len() * 4,
            "target": 34963,
        }));
        byte_offset += mesh.triangles.len() * 4;
    }

    if !mesh.normals.is_empty() {
        accessors.push(json!({
            "bufferView": buffer_views.len(),
            "byteOffset": 0,
            "componentType": 5126,
            "count": mesh.normals.len(),
            "type": "VEC3",
        }));

        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": byte_offset,
            "byteLength": mesh.normals.len() * 12,
            "target": 34962,
        }));
        byte_offset += mesh.normals.len() * 12;
    }

    if !mesh.uvs.is_empty() {
        accessors.push(json!({
            "bufferView": buffer_views.len(),
            "byteOffset": 0,
            "componentType": 5126,
            "count": mesh.uvs.len(),
            "type": "VEC2",
        }));

        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": byte_offset,
            "byteLength": mesh.uvs.len() * 8,
            "target": 34962,
        }));
        byte_offset += mesh.uvs.len() * 8;
    }

    if !mesh.tangents.is_empty() {
        accessors.push(json!({
            "bufferView": buffer_views.len(),
            "byteOffset": 0,
            "componentType": 5126,
            "count": mesh.tangents.len(),
            "type": "VEC4",
        }));

        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": byte_offset,
            "byteLength": mesh.tangents.len() * 16,
            "target": 34962,
        }));
        byte_offset += mesh.tangents.len() * 16;
    }

    let mut gltf = json!({
        "asset": {
            "generator": "Aegis-Assets Unity Plugin",
            "version": "2.0"
        },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{
            "mesh": 0,
            "name": mesh.name,
        }],
        "meshes": [mesh_json],
        "bufferViews": buffer_views,
        "accessors": accessors,
    });

    if options.embed_buffers {
        let buffer_bytes = serialize_vertex_data(mesh);
        gltf["buffers"] = json!([{
            "byteLength": buffer_bytes.len(),
            "uri": format!(
                "data:application/octet-stream;base64,{}",
                BASE64_STANDARD.encode(buffer_bytes)
            ),
        }]);
    } else {
        gltf["buffers"] = json!([{
            "byteLength": byte_offset,
            "uri": format!("{}.bin", mesh.name),
        }]);
    }

    let bytes = serde_json::to_vec_pretty(&gltf).context("Failed to serialize glTF")?;
    Ok((bytes, stats))
}

fn serialize_vertex_data(mesh: &UnityMesh) -> Vec<u8> {
    let mut data = Vec::new();

    for vertex in &mesh.vertices {
        for component in vertex {
            data.extend_from_slice(&component.to_le_bytes());
        }
    }

    for index in &mesh.triangles {
        data.extend_from_slice(&index.to_le_bytes());
    }

    for normal in &mesh.normals {
        for component in normal {
            data.extend_from_slice(&component.to_le_bytes());
        }
    }

    for uv in &mesh.uvs {
        for component in uv {
            data.extend_from_slice(&component.to_le_bytes());
        }
    }

    for tangent in &mesh.tangents {
        for component in tangent {
            data.extend_from_slice(&component.to_le_bytes());
        }
    }

    data
}

fn build_wavefront_obj(mesh: &UnityMesh) -> Vec<u8> {
    let mut obj = String::new();

    obj.push_str(&format!("o {}\n", mesh.name));

    for vertex in &mesh.vertices {
        obj.push_str(&format!("v {} {} {}\n", vertex[0], vertex[1], vertex[2]));
    }

    for uv in &mesh.uvs {
        obj.push_str(&format!("vt {} {}\n", uv[0], uv[1]));
    }

    for normal in &mesh.normals {
        obj.push_str(&format!("vn {} {} {}\n", normal[0], normal[1], normal[2]));
    }

    // Triangles use 1-based indexing in OBJ.
    for chunk in mesh.triangles.chunks(3) {
        if chunk.len() == 3 {
            let indices: Vec<String> = chunk
                .iter()
                .map(|idx| {
                    // Subtracting/adding 1 handles the conversion from 0-based to 1-based indices.
                    // Include UV/normal references when available.
                    let vertex_index = idx + 1;
                    if mesh.uvs.is_empty() && mesh.normals.is_empty() {
                        format!("{}", vertex_index)
                    } else if mesh.uvs.is_empty() {
                        format!("{}//{}", vertex_index, vertex_index)
                    } else if mesh.normals.is_empty() {
                        format!("{}/{}", vertex_index, vertex_index)
                    } else {
                        format!("{}/{}/{}", vertex_index, vertex_index, vertex_index)
                    }
                })
                .collect();

            obj.push_str(&format!("f {} {} {}\n", indices[0], indices[1], indices[2]));
        }
    }

    obj.into_bytes()
}
