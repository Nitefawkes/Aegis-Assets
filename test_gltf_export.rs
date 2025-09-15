use aegis_core::export::{Exporter, ExportOptions};
use aegis_core::resource::{MeshResource, Vertex};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing glTF Mesh Export Functionality");

    // Create a simple cube mesh for testing
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Cube vertices (8 corners of a cube)
    let cube_positions = [
        [-0.5, -0.5,  0.5], // 0: front-bottom-left
        [ 0.5, -0.5,  0.5], // 1: front-bottom-right
        [ 0.5,  0.5,  0.5], // 2: front-top-right
        [-0.5,  0.5,  0.5], // 3: front-top-left
        [-0.5, -0.5, -0.5], // 4: back-bottom-left
        [ 0.5, -0.5, -0.5], // 5: back-bottom-right
        [ 0.5,  0.5, -0.5], // 6: back-top-right
        [-0.5,  0.5, -0.5], // 7: back-top-left
    ];

    // Cube UV coordinates
    let cube_uvs = [
        [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], // front face
        [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], // back face
    ];

    // Create vertices with position and UV data
    for (i, &pos) in cube_positions.iter().enumerate() {
        let uv_index = i % 4;
        vertices.push(Vertex {
            position: pos,
            normal: Some([0.0, 0.0, 1.0]), // Simple normal for front face
            uv: Some(cube_uvs[uv_index]),
            color: Some([1.0, 1.0, 1.0, 1.0]), // White color
        });
    }

    // Cube indices (12 triangles = 6 faces * 2 triangles per face)
    let cube_indices = [
        // Front face
        0, 1, 2,  0, 2, 3,
        // Back face
        5, 4, 7,  5, 7, 6,
        // Left face
        4, 0, 3,  4, 3, 7,
        // Right face
        1, 5, 6,  1, 6, 2,
        // Top face
        3, 2, 6,  3, 6, 7,
        // Bottom face
        4, 5, 1,  4, 1, 0,
    ];

    indices.extend_from_slice(&cube_indices);

    // Create the mesh resource
    let mesh = MeshResource {
        name: "TestCube".to_string(),
        vertices,
        indices,
        material_id: Some("DefaultMaterial".to_string()),
        bone_weights: None,
    };

    // Create exporter and export to glTF
    let exporter = Exporter::new(ExportOptions::default());
    let output_dir = Path::new("test_gltf");

    println!("📦 Creating test mesh with {} vertices and {} triangles",
             mesh.vertices.len(), mesh.indices.len() / 3);

    println!("🔄 Exporting to glTF format...");
    let exported_files = exporter.export_mesh(&mesh, output_dir, None)?;

    for file in &exported_files {
        println!("✅ Exported: {} ({})",
                 file.path.display(),
                 aegis_core::format_bytes(file.size_bytes));
    }

    println!("🎉 glTF export test completed successfully!");
    println!("📁 Output files are in: {}", output_dir.display());

    Ok(())
}
