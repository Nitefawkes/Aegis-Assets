use crate::{archive::Provenance, resource::Resource};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
                        resource.name(),
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
        
        match resource {
            Resource::Texture(texture) => {
                files.extend(self.export_texture(texture, output_dir, provenance)?);
            }
            Resource::Mesh(mesh) => {
                files.extend(self.export_mesh(mesh, output_dir, provenance)?);
            }
            Resource::Material(material) => {
                files.extend(self.export_material(material, output_dir, provenance)?);
            }
            Resource::Audio(audio) => {
                files.extend(self.export_audio(audio, output_dir, provenance)?);
            }
            Resource::Animation(animation) => {
                files.extend(self.export_animation(animation, output_dir, provenance)?);
            }
            _ => {
                // Generic JSON export for other resource types
                files.extend(self.export_generic_json(resource, output_dir, provenance)?);
            }
        }
        
        Ok(files)
    }
    
    /// Export texture resource
    fn export_texture(
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
        
        // Mock export - in real implementation, would convert texture data
        std::fs::write(&output_path, &texture.data)
            .context("Failed to write texture file")?;
        
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
    fn export_mesh(
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
    
    /// Create glTF JSON from mesh (simplified mock)
    fn create_gltf_from_mesh(
        &self,
        mesh: &crate::resource::MeshResource,
        provenance: Option<&Provenance>,
    ) -> Result<Vec<u8>> {
        let mut gltf = serde_json::Map::new();
        
        // Basic glTF structure
        gltf.insert("asset".to_string(), serde_json::json!({
            "version": "2.0",
            "generator": "Aegis-Assets Core"
        }));
        
        // Add provenance if available
        if let Some(prov) = provenance {
            gltf.insert("extras".to_string(), serde_json::json!({
                "aegis_provenance": prov,
                "extraction_method": "patch_recipe",
                "legal_status": "personal_use_only"
            }));
        }
        
        // Mock scene with single mesh
        gltf.insert("scenes".to_string(), serde_json::json!([{
            "nodes": [0]
        }]));
        
        gltf.insert("nodes".to_string(), serde_json::json!([{
            "name": mesh.name,
            "mesh": 0
        }]));
        
        gltf.insert("meshes".to_string(), serde_json::json!([{
            "name": mesh.name,
            "primitives": [{
                "attributes": {
                    "POSITION": 0
                },
                "indices": 1
            }]
        }]));
        
        let json_value = serde_json::Value::Object(gltf);
        serde_json::to_vec_pretty(&json_value)
            .context("Failed to serialize glTF JSON")
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
        let output_path = output_dir.join(format!("{}.{}.json", 
            resource.name(), resource.resource_type()));
        
        let json = serde_json::to_vec_pretty(resource)
            .context("Failed to serialize resource")?;
        
        std::fs::write(&output_path, &json)
            .context("Failed to write resource file")?;
        
        let metadata = std::fs::metadata(&output_path)?;
        Ok(vec![ExportedFile {
            path: output_path,
            size_bytes: metadata.len(),
            format: ExportFormat::Json,
            source_resource: resource.name().to_string(),
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
        
        let texture = Resource::Texture(TextureResource {
            name: "test_texture".to_string(),
            width: 64,
            height: 64,
            format: TextureFormat::RGBA8,
            data: vec![255u8; 64 * 64 * 4],
            mip_levels: 1,
            usage_hint: Some(TextureUsage::Albedo),
        });
        
        let result = exporter.export_resources(
            &[texture],
            temp_dir.path(),
            None,
        );
        
        assert!(result.is_ok());
        let export_result = result.unwrap();
        assert!(!export_result.files.is_empty());
        assert!(export_result.total_bytes > 0);
    }
}
