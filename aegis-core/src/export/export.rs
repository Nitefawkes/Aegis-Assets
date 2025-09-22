use crate::{archive::Provenance, resource::{Resource, TextureFormat}};
use anyhow::{Result, Context, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use image::{ImageBuffer, Rgba};

/// Supported export formats
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportFormat {
    /// glTF 2.0 for 3D assets
    GltF2,
    /// KTX2 for textures with basis compression
    Ktx2,
    /// PNG for uncompressed textures
    Png,
    /// OGG for audio
    Ogg,
    /// WAV for uncompressed audio
    Wav,
    /// JSON for metadata and materials
    Json,
}

/// Export configuration options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    /// Target export formats (automatic selection if empty)
    pub formats: Vec<ExportFormat>,
    /// Include provenance metadata
    pub include_provenance: bool,
    /// Compress textures using Basis Universal
    pub compress_textures: bool,
    /// Texture compression quality (0-255)
    pub texture_quality: u8,
    /// Export only specific resource types
    pub resource_filter: Option<Vec<String>>,
    /// Custom output naming pattern
    pub naming_pattern: Option<String>,
    /// Generate index/manifest file
    pub generate_manifest: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            formats: vec![], // Auto-detect
            include_provenance: true,
            compress_textures: true,
            texture_quality: 128, // Medium quality
            resource_filter: None,
            naming_pattern: None,
            generate_manifest: true,
        }
    }
}

/// Export result information
#[derive(Debug, Clone)]
pub struct ExportResult {
    /// Exported files
    pub files: Vec<ExportedFile>,
    /// Total bytes written
    pub total_bytes: u64,
    /// Export duration
    pub duration_ms: u64,
    /// Warnings encountered
    pub warnings: Vec<String>,
}

/// Information about an exported file
#[derive(Debug, Clone)]
pub struct ExportedFile {
    /// Output file path
    pub path: PathBuf,
    /// File size in bytes
    pub size_bytes: u64,
    /// Export format used
    pub format: ExportFormat,
    /// Original resource name
    pub source_resource: String,
}

/// Main exporter for converting resources to standard formats
pub struct Exporter {
    options: ExportOptions,
}

impl Exporter {
    /// Create a new exporter with default options
    pub fn new() -> Self {
        Self {
            options: ExportOptions::default(),
        }
    }
    
    /// Create exporter with custom options
    pub fn with_options(options: ExportOptions) -> Self {
        Self { options }
    }
    
    /// Export resources to the specified directory
    pub fn export_resources(
        &self,
        resources: &[Resource],
        output_dir: &Path,
        provenance: Option<&Provenance>,
    ) -> Result<ExportResult> {
        let start_time = std::time::Instant::now();
        let mut exported_files = Vec::new();
        let mut total_bytes = 0u64;
        let mut warnings = Vec::new();
        
        // Create output directory
        std::fs::create_dir_all(output_dir)
            .context("Failed to create output directory")?;
        
        // Export each resource
        for resource in resources {
            match self.export_single_resource(resource, output_dir, provenance) {
                Ok(mut files) => {
                    for file in &files {
                        total_bytes += file.size_bytes;
                    }
                    exported_files.append(&mut files);
                }
                Err(e) => {
                    let warning = format!(
                        "Failed to export resource '{}': {}",
                        &resource.name,
                        e
                    );
                    warnings.push(warning);
                }
            }
        }
        
        // Generate manifest if requested
        if self.options.generate_manifest {
            match self.generate_manifest(&exported_files, output_dir, provenance) {
                Ok(manifest_file) => {
                    total_bytes += manifest_file.size_bytes;
                    exported_files.push(manifest_file);
                }
                Err(e) => {
                    warnings.push(format!("Failed to generate manifest: {}", e));
                }
            }
        }
        
        let duration = start_time.elapsed();
        
        Ok(ExportResult {
            files: exported_files,
            total_bytes,
            duration_ms: duration.as_millis() as u64,
            warnings,
        })
    }
    
    /// Export a single resource
    fn export_single_resource(
        &self,
        resource: &Resource,
        output_dir: &Path,
        provenance: Option<&Provenance>,
    ) -> Result<Vec<ExportedFile>> {
        let mut files = Vec::new();
        
        match &resource.resource_type {
            crate::resource::ResourceType::Texture => {
                // For now, just export as generic JSON since we don't have texture data access
                files.extend(self.export_generic_json(resource, output_dir, provenance)?);
            }
            crate::resource::ResourceType::Mesh => {
                // For now, just export as generic JSON since we don't have mesh data access
                files.extend(self.export_generic_json(resource, output_dir, provenance)?);
            }
            crate::resource::ResourceType::Material => {
                // For now, just export as generic JSON since we don't have material data access
                files.extend(self.export_generic_json(resource, output_dir, provenance)?);
            }
            crate::resource::ResourceType::Audio => {
                // For now, just export as generic JSON since we don't have audio data access
                files.extend(self.export_generic_json(resource, output_dir, provenance)?);
            }
            crate::resource::ResourceType::Animation => {
                // For now, just export as generic JSON since we don't have animation data access
                files.extend(self.export_generic_json(resource, output_dir, provenance)?);
            }
            _ => {
                // Generic JSON export for other resource types
                files.extend(self.export_generic_json(resource, output_dir, provenance)?);
            }
        }
        
        Ok(files)
    }

    fn decode_block_compressed_texture(
        data: &[u8],
        width: u32,
        height: u32,
        block_size: usize,
        decoder: fn(&[u8], &mut [u8], usize),
    ) -> Result<Vec<u8>> {
        if width == 0 || height == 0 {
            return Ok(Vec::new());
        }

        let blocks_x = ((width + 3) / 4) as usize;
        let blocks_y = ((height + 3) / 4) as usize;
        let required_len = blocks_x * blocks_y * block_size;

        if data.len() < required_len {
            bail!(
                "Compressed texture data is too small: expected at least {} bytes, found {}",
                required_len,
                data.len()
            );
        }

        let mut rgba = vec![0u8; width as usize * height as usize * 4];
        let mut block_rgba = [0u8; 4 * 4 * 4];
        let row_pitch = width as usize * 4;

        for block_y in 0..blocks_y {
            for block_x in 0..blocks_x {
                let offset = (block_y * blocks_x + block_x) * block_size;
                let block = &data[offset..offset + block_size];
                block_rgba.fill(0);
                decoder(block, &mut block_rgba, 4 * 4);

                for row in 0..4 {
                    let dest_y = block_y * 4 + row;
                    if dest_y >= height as usize {
                        continue;
                    }

                    let dest_x = block_x * 4;
                    if dest_x >= width as usize {
                        continue;
                    }

                    let pixels_in_row = std::cmp::min(4, width as usize - dest_x);
                    let dest_start = dest_y * row_pitch + dest_x * 4;
                    let src_start = row * 4 * 4;
                    let src_end = src_start + pixels_in_row * 4;
                    rgba[dest_start..dest_start + pixels_in_row * 4]
                        .copy_from_slice(&block_rgba[src_start..src_end]);
                }
            }
        }

        Ok(rgba)
    }
    
    /// Export texture resource
    pub fn export_texture(
        &self,
        texture: &crate::resource::TextureResource,
        output_dir: &Path,
        _provenance: Option<&Provenance>,
    ) -> Result<Vec<ExportedFile>> {
        let mut files = Vec::new();
        
        // Determine export format
        let format = if self.options.compress_textures && texture.data.len() > 64 * 1024 {
            ExportFormat::Ktx2
        } else {
            ExportFormat::Png
        };
        
        let file_extension = match format {
            ExportFormat::Ktx2 => "ktx2",
            ExportFormat::Png => "png",
            _ => "bin",
        };
        
        let output_path = output_dir.join(format!("{}.{}", texture.name, file_extension));
        
        // Convert texture data to PNG
        let png_data = match texture.format {
            TextureFormat::RGBA8 => {
                // Direct RGBA8 - just use as-is
                ImageBuffer::<Rgba<u8>, _>::from_raw(texture.width, texture.height, texture.data.clone())
                    .context("Failed to create RGBA8 image from texture data")?
            },
            TextureFormat::RGB8 => {
                // RGB8 - convert to RGBA8 by adding alpha channel
                let mut rgba_data = Vec::with_capacity(texture.width as usize * texture.height as usize * 4);
                for chunk in texture.data.chunks(3) {
                    rgba_data.extend_from_slice(chunk);
                    rgba_data.push(255); // Add alpha channel
                }
                ImageBuffer::<Rgba<u8>, _>::from_raw(texture.width, texture.height, rgba_data)
                    .context("Failed to create RGBA8 image from RGB8 texture data")?
            },
            TextureFormat::RGBA16 => {
                // RGBA16 - convert to RGBA8 (16-bit to 8-bit)
                let mut rgba8_data = Vec::with_capacity(texture.width as usize * texture.height as usize * 4);
                for chunk in texture.data.chunks(2) {
                    if chunk.len() == 2 {
                        // Convert 16-bit to 8-bit (simple approach - take upper 8 bits)
                        rgba8_data.push(chunk[1]); // Upper 8 bits
                        rgba8_data.push(chunk[1]); // Duplicate for both channels if needed
                        rgba8_data.push(chunk[1]);
                        rgba8_data.push(chunk[1]);
                    }
                }
                ImageBuffer::<Rgba<u8>, _>::from_raw(texture.width, texture.height, rgba8_data)
                    .context("Failed to create RGBA8 image from RGBA16 texture data")?
            },
            TextureFormat::DXT1 => {
                let rgba = Self::decode_block_compressed_texture(
                    &texture.data,
                    texture.width,
                    texture.height,
                    8,
                    bcdec_rs::bc1,
                )?;
                ImageBuffer::<Rgba<u8>, _>::from_raw(texture.width, texture.height, rgba)
                    .context("Failed to create RGBA8 image from DXT1 texture data")?
            }
            TextureFormat::DXT3 => {
                let rgba = Self::decode_block_compressed_texture(
                    &texture.data,
                    texture.width,
                    texture.height,
                    16,
                    bcdec_rs::bc2,
                )?;
                ImageBuffer::<Rgba<u8>, _>::from_raw(texture.width, texture.height, rgba)
                    .context("Failed to create RGBA8 image from DXT3 texture data")?
            }
            TextureFormat::DXT5 => {
                let rgba = Self::decode_block_compressed_texture(
                    &texture.data,
                    texture.width,
                    texture.height,
                    16,
                    bcdec_rs::bc3,
                )?;
                ImageBuffer::<Rgba<u8>, _>::from_raw(texture.width, texture.height, rgba)
                    .context("Failed to create RGBA8 image from DXT5 texture data")?
            }
            TextureFormat::BC7 => {
                let rgba = Self::decode_block_compressed_texture(
                    &texture.data,
                    texture.width,
                    texture.height,
                    16,
                    bcdec_rs::bc7,
                )?;
                ImageBuffer::<Rgba<u8>, _>::from_raw(texture.width, texture.height, rgba)
                    .context("Failed to create RGBA8 image from BC7 texture data")?
            }
            TextureFormat::ETC2 | TextureFormat::ASTC => {
                bail!("Decoding {:?} textures is not supported yet", texture.format)
            }
        };

        // Save as PNG
        png_data.save_with_format(&output_path, image::ImageFormat::Png)
            .context("Failed to save texture as PNG")?;
        
        let metadata = std::fs::metadata(&output_path)?;
        files.push(ExportedFile {
            path: output_path,
            size_bytes: metadata.len(),
            format,
            source_resource: texture.name.clone(),
        });
        
        Ok(files)
    }
    
    /// Export mesh resource to glTF
    pub fn export_mesh(
        &self,
        mesh: &crate::resource::MeshResource,
        output_dir: &Path,
        provenance: Option<&Provenance>,
    ) -> Result<Vec<ExportedFile>> {
        let output_path = output_dir.join(format!("{}.gltf", mesh.name));
        
        // Create basic glTF structure (mock implementation)
        let gltf_data = self.create_gltf_from_mesh(mesh, provenance)?;
        
        std::fs::write(&output_path, &gltf_data)
            .context("Failed to write glTF file")?;
        
        let metadata = std::fs::metadata(&output_path)?;
        Ok(vec![ExportedFile {
            path: output_path,
            size_bytes: metadata.len(),
            format: ExportFormat::GltF2,
            source_resource: mesh.name.clone(),
        }])
    }
    
    /// Create glTF 2.0 JSON from mesh with binary data
    fn create_gltf_from_mesh(
        &self,
        mesh: &crate::resource::MeshResource,
        provenance: Option<&Provenance>,
    ) -> Result<Vec<u8>> {
        let mut gltf = serde_json::Map::new();
        
        // glTF asset information
        gltf.insert("asset".to_string(), serde_json::json!({
            "version": "2.0",
            "generator": "Aegis-Assets Core v0.1.0",
            "copyright": "Extracted for personal use only - check original game licensing"
        }));
        
        // Add provenance information
        if let Some(prov) = provenance {
            gltf.insert("extras".to_string(), serde_json::json!({
                "aegis_provenance": {
                    "session_id": prov.session_id,
                    "source_hash": prov.source_hash,
                    "extraction_time": prov.extraction_time.to_rfc3339(),
                    "aegis_version": prov.aegis_version
                },
                "extraction_method": "aegis_assets_unity_plugin",
                "legal_status": "personal_use_only",
                "compliance_profile": prov.compliance_profile.publisher
            }));
        }
        
        // Create binary data for vertices and indices
        let (binary_data, accessors) = self.create_mesh_binary_data(mesh)?;

        // Create buffer view for the binary data
        gltf.insert("bufferViews".to_string(), serde_json::json!([{
            "buffer": 0,
            "byteOffset": 0,
            "byteLength": binary_data.len(),
            "target": 34962  // ARRAY_BUFFER
        }]));

        // Create buffer
        gltf.insert("buffers".to_string(), serde_json::json!([{
            "byteLength": binary_data.len(),
            "uri": format!("{}.bin", mesh.name)
        }]));

        // Scene setup
        gltf.insert("scene".to_string(), serde_json::Value::Number(0.into()));
        gltf.insert("scenes".to_string(), serde_json::json!([{
            "name": "Scene",
            "nodes": [0]
        }]));
        
        // Node with mesh
        gltf.insert("nodes".to_string(), serde_json::json!([{
            "name": mesh.name,
            "mesh": 0
        }]));

        // Mesh with primitives
        let mut primitives = Vec::new();
        let mut attributes = serde_json::Map::new();

        // Position attribute (always present)
        attributes.insert("POSITION".to_string(), serde_json::Value::Number(0.into()));

        // Normal attribute (if available)
        if mesh.vertices.iter().any(|v| v.normal.is_some()) {
            attributes.insert("NORMAL".to_string(), serde_json::Value::Number(1.into()));
        }

        // UV attribute (if available)
        if mesh.vertices.iter().any(|v| v.uv.is_some()) {
            attributes.insert("TEXCOORD_0".to_string(), serde_json::Value::Number(2.into()));
        }

        // Color attribute (if available)
        if mesh.vertices.iter().any(|v| v.color.is_some()) {
            attributes.insert("COLOR_0".to_string(), serde_json::Value::Number(3.into()));
        }

        primitives.push(serde_json::json!({
            "attributes": attributes,
            "indices": 4,
            "mode": 4  // TRIANGLES
        }));
        
        gltf.insert("meshes".to_string(), serde_json::json!([{
            "name": mesh.name,
            "primitives": primitives
        }]));
        
        // Add accessors
        gltf.insert("accessors".to_string(), serde_json::Value::Array(accessors));

        // Create the complete glTF JSON
        let json_value = serde_json::Value::Object(gltf);
        let json_bytes = serde_json::to_vec_pretty(&json_value)
            .context("Failed to serialize glTF JSON")?;

        Ok(json_bytes)
    }

    /// Create binary data and accessors for mesh geometry
    fn create_mesh_binary_data(&self, mesh: &crate::resource::MeshResource) -> Result<(Vec<u8>, Vec<serde_json::Value>)> {
        let mut binary_data = Vec::new();
        let mut accessors = Vec::new();

        // Calculate data sizes and offsets
        let vertex_count = mesh.vertices.len();
        let index_count = mesh.indices.len();

        // POSITION accessor (required)
        let position_min = self.calculate_position_bounds(&mesh.vertices, 0);
        let position_max = self.calculate_position_bounds(&mesh.vertices, 1);
        accessors.push(serde_json::json!({
            "bufferView": 0,
            "byteOffset": binary_data.len(),
            "componentType": 5126,  // FLOAT
            "count": vertex_count,
            "type": "VEC3",
            "min": position_min,
            "max": position_max
        }));

        // Write vertex positions to binary data
        for vertex in &mesh.vertices {
            binary_data.extend_from_slice(&vertex.position[0].to_le_bytes());
            binary_data.extend_from_slice(&vertex.position[1].to_le_bytes());
            binary_data.extend_from_slice(&vertex.position[2].to_le_bytes());
        }

        // NORMAL accessor (optional)
        if mesh.vertices.iter().any(|v| v.normal.is_some()) {
            accessors.push(serde_json::json!({
                "bufferView": 0,
                "byteOffset": binary_data.len(),
                "componentType": 5126,  // FLOAT
                "count": vertex_count,
                "type": "VEC3"
            }));

            // Write normals to binary data
            for vertex in &mesh.vertices {
                if let Some(normal) = vertex.normal {
                    binary_data.extend_from_slice(&normal[0].to_le_bytes());
                    binary_data.extend_from_slice(&normal[1].to_le_bytes());
                    binary_data.extend_from_slice(&normal[2].to_le_bytes());
                } else {
                    // Default normal if not available
                    binary_data.extend_from_slice(&[0.0f32.to_le_bytes(), 0.0f32.to_le_bytes(), 1.0f32.to_le_bytes()].concat());
                }
            }
        }

        // TEXCOORD_0 accessor (optional)
        if mesh.vertices.iter().any(|v| v.uv.is_some()) {
            accessors.push(serde_json::json!({
                "bufferView": 0,
                "byteOffset": binary_data.len(),
                "componentType": 5126,  // FLOAT
                "count": vertex_count,
                "type": "VEC2"
            }));

            // Write UVs to binary data
            for vertex in &mesh.vertices {
                if let Some(uv) = vertex.uv {
                    binary_data.extend_from_slice(&uv[0].to_le_bytes());
                    binary_data.extend_from_slice(&uv[1].to_le_bytes());
                } else {
                    // Default UV if not available
                    binary_data.extend_from_slice(&[0.0f32.to_le_bytes(), 0.0f32.to_le_bytes()].concat());
                }
            }
        }

        // COLOR_0 accessor (optional)
        if mesh.vertices.iter().any(|v| v.color.is_some()) {
            accessors.push(serde_json::json!({
                "bufferView": 0,
                "byteOffset": binary_data.len(),
                "componentType": 5126,  // FLOAT
                "count": vertex_count,
                "type": "VEC4"
            }));

            // Write colors to binary data
            for vertex in &mesh.vertices {
                if let Some(color) = vertex.color {
                    binary_data.extend_from_slice(&color[0].to_le_bytes());
                    binary_data.extend_from_slice(&color[1].to_le_bytes());
                    binary_data.extend_from_slice(&color[2].to_le_bytes());
                    binary_data.extend_from_slice(&color[3].to_le_bytes());
                } else {
                    // Default white color if not available
                    binary_data.extend_from_slice(&[1.0f32.to_le_bytes(), 1.0f32.to_le_bytes(), 1.0f32.to_le_bytes(), 1.0f32.to_le_bytes()].concat());
                }
            }
        }

        // INDICES accessor
        accessors.push(serde_json::json!({
            "bufferView": 0,
            "byteOffset": binary_data.len(),
            "componentType": 5123,  // UNSIGNED_SHORT (if indices fit, otherwise UNSIGNED_INT)
            "count": index_count,
            "type": "SCALAR"
        }));

        // Write indices to binary data
        for index in &mesh.indices {
            binary_data.extend_from_slice(&(*index as u16).to_le_bytes());  // Convert to u16 for smaller file size
        }

        Ok((binary_data, accessors))
    }

    /// Calculate position bounds for glTF accessor
    fn calculate_position_bounds(&self, vertices: &[crate::resource::Vertex], bound_type: usize) -> Vec<f32> {
        let mut bounds = [0.0f32; 3];

        for vertex in vertices {
            for i in 0..3 {
                if bound_type == 0 {  // min
                    if vertex.position[i] < bounds[i] {
                        bounds[i] = vertex.position[i];
                    }
                } else {  // max
                    if vertex.position[i] > bounds[i] {
                        bounds[i] = vertex.position[i];
                    }
                }
            }
        }

        bounds.to_vec()
    }
    
    /// Export material resource
    fn export_material(
        &self,
        material: &crate::resource::MaterialResource,
        output_dir: &Path,
        _provenance: Option<&Provenance>,
    ) -> Result<Vec<ExportedFile>> {
        let output_path = output_dir.join(format!("{}.material.json", material.name));
        
        let json = serde_json::to_vec_pretty(material)
            .context("Failed to serialize material")?;
        
        std::fs::write(&output_path, &json)
            .context("Failed to write material file")?;
        
        let metadata = std::fs::metadata(&output_path)?;
        Ok(vec![ExportedFile {
            path: output_path,
            size_bytes: metadata.len(),
            format: ExportFormat::Json,
            source_resource: material.name.clone(),
        }])
    }
    
    /// Export audio resource
    fn export_audio(
        &self,
        audio: &crate::resource::AudioResource,
        output_dir: &Path,
        _provenance: Option<&Provenance>,
    ) -> Result<Vec<ExportedFile>> {
        let format = ExportFormat::Ogg; // Default to OGG
        let extension = "ogg";
        
        let output_path = output_dir.join(format!("{}.{}", audio.name, extension));
        
        // Mock export - would convert audio data to target format
        std::fs::write(&output_path, &audio.data)
            .context("Failed to write audio file")?;
        
        let metadata = std::fs::metadata(&output_path)?;
        Ok(vec![ExportedFile {
            path: output_path,
            size_bytes: metadata.len(),
            format,
            source_resource: audio.name.clone(),
        }])
    }
    
    /// Export animation resource
    fn export_animation(
        &self,
        animation: &crate::resource::AnimationResource,
        output_dir: &Path,
        _provenance: Option<&Provenance>,
    ) -> Result<Vec<ExportedFile>> {
        let output_path = output_dir.join(format!("{}.animation.json", animation.name));
        
        let json = serde_json::to_vec_pretty(animation)
            .context("Failed to serialize animation")?;
        
        std::fs::write(&output_path, &json)
            .context("Failed to write animation file")?;
        
        let metadata = std::fs::metadata(&output_path)?;
        Ok(vec![ExportedFile {
            path: output_path,
            size_bytes: metadata.len(),
            format: ExportFormat::Json,
            source_resource: animation.name.clone(),
        }])
    }
    
    /// Export resource as generic JSON
    fn export_generic_json(
        &self,
        resource: &Resource,
        output_dir: &Path,
        _provenance: Option<&Provenance>,
    ) -> Result<Vec<ExportedFile>> {
        let output_path = output_dir.join(format!("{}.{:?}.json", 
            &resource.name, &resource.resource_type));
        
        let json = serde_json::to_vec_pretty(resource)
            .context("Failed to serialize resource")?;
        
        std::fs::write(&output_path, &json)
            .context("Failed to write resource file")?;
        
        let metadata = std::fs::metadata(&output_path)?;
        Ok(vec![ExportedFile {
            path: output_path,
            size_bytes: metadata.len(),
            format: ExportFormat::Json,
            source_resource: resource.name.clone(),
        }])
    }
    
    /// Generate manifest file
    fn generate_manifest(
        &self,
        exported_files: &[ExportedFile],
        output_dir: &Path,
        provenance: Option<&Provenance>,
    ) -> Result<ExportedFile> {
        let manifest_path = output_dir.join("manifest.json");
        
        let mut manifest = serde_json::Map::new();
        manifest.insert("version".to_string(), serde_json::Value::String("1.0".to_string()));
        manifest.insert("generator".to_string(), serde_json::Value::String("Aegis-Assets".to_string()));
        manifest.insert("exported_at".to_string(), serde_json::Value::String(
            chrono::Utc::now().to_rfc3339()
        ));
        
        if let Some(prov) = provenance {
            manifest.insert("provenance".to_string(), serde_json::to_value(prov)
                .context("Failed to serialize provenance")?);
        }
        
        let files_info: Vec<_> = exported_files.iter().map(|f| {
            serde_json::json!({
                "path": f.path.file_name().unwrap().to_str(),
                "size_bytes": f.size_bytes,
                "format": f.format,
                "source_resource": f.source_resource
            })
        }).collect();
        
        manifest.insert("files".to_string(), serde_json::Value::Array(files_info));
        
        let json = serde_json::to_vec_pretty(&serde_json::Value::Object(manifest))
            .context("Failed to serialize manifest")?;
        
        std::fs::write(&manifest_path, &json)
            .context("Failed to write manifest file")?;
        
        let metadata = std::fs::metadata(&manifest_path)?;
        Ok(ExportedFile {
            path: manifest_path,
            size_bytes: metadata.len(),
            format: ExportFormat::Json,
            source_resource: "manifest".to_string(),
        })
    }
}

impl Default for Exporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{TextureResource, TextureFormat, TextureUsage};
    use tempfile::TempDir;

    const DXT1_SAMPLE: [u8; 8] = [139, 37, 139, 37, 0, 0, 0, 0];
    const BC7_SAMPLE: [u8; 16] = [
        32, 145, 72, 54, 219, 106, 253, 255, 175, 170, 170, 170, 86, 85, 85, 85,
    ];
    const DXT_EXPECTED_PIXEL: [u8; 4] = [33, 178, 90, 255];
    const BC7_EXPECTED_PIXEL: [u8; 4] = [34, 179, 90, 255];

    #[test]
    fn test_exporter_creation() {
        let exporter = Exporter::new();
        assert!(exporter.options.include_provenance);
        assert!(exporter.options.compress_textures);
    }
    
    #[test]
    fn test_texture_export() {
        let temp_dir = TempDir::new().unwrap();
        let exporter = Exporter::new();
        
        let texture = TextureResource {
            name: "test_texture".to_string(),
            width: 64,
            height: 64,
            format: TextureFormat::RGBA8,
            data: vec![255u8; 64 * 64 * 4],
            mip_levels: 1,
            usage_hint: Some(TextureUsage::Albedo),
        };

        let files = exporter
            .export_texture(&texture, temp_dir.path(), None)
            .expect("export rgba8");

        assert!(!files.is_empty());
        assert!(files[0].size_bytes > 0);

        let image = image::open(&files[0].path).expect("open png").to_rgba8();
        assert_eq!(image.dimensions(), (64, 64));
        for pixel in image.pixels() {
            assert_eq!(pixel.0, [255, 255, 255, 255]);
        }
    }

    #[test]
    fn test_compressed_dxt1_texture_export() {
        let temp_dir = TempDir::new().unwrap();
        let exporter = Exporter::new();

        let texture = TextureResource {
            name: "compressed_dxt1".to_string(),
            width: 4,
            height: 4,
            format: TextureFormat::DXT1,
            data: DXT1_SAMPLE.to_vec(),
            mip_levels: 1,
            usage_hint: None,
        };

        let files = exporter
            .export_texture(&texture, temp_dir.path(), None)
            .expect("export dxt1");

        assert_eq!(files.len(), 1);
        let image = image::open(&files[0].path).expect("open png").to_rgba8();
        assert_eq!(image.dimensions(), (4, 4));

        for pixel in image.pixels() {
            assert_eq!(pixel.0, DXT_EXPECTED_PIXEL);
        }
    }

    #[test]
    fn test_compressed_bc7_texture_export() {
        let temp_dir = TempDir::new().unwrap();
        let exporter = Exporter::new();

        let texture = TextureResource {
            name: "compressed_bc7".to_string(),
            width: 4,
            height: 4,
            format: TextureFormat::BC7,
            data: BC7_SAMPLE.to_vec(),
            mip_levels: 1,
            usage_hint: None,
        };

        let files = exporter
            .export_texture(&texture, temp_dir.path(), None)
            .expect("export bc7");

        assert_eq!(files.len(), 1);
        let image = image::open(&files[0].path).expect("open png").to_rgba8();
        assert_eq!(image.dimensions(), (4, 4));

        for pixel in image.pixels() {
            assert_eq!(pixel.0, BC7_EXPECTED_PIXEL);
        }
    }
}
