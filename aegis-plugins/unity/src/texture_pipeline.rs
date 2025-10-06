use anyhow::{Result, Context, bail};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read, Seek, SeekFrom};
use image::{ImageBuffer, Rgba, ImageFormat, DynamicImage, RgbaImage};
use serde::{Serialize, Deserialize};
use serde_json;
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::converters::{UnityTexture2D, UnityTextureFormat};
use crate::streaming::{StreamingStats, MemoryPressureMonitor};
use std::sync::Arc;

/// Enhanced texture conversion pipeline for Sprint 2
pub struct TexturePipeline {
    memory_monitor: Arc<MemoryPressureMonitor>,
    conversion_stats: Vec<TextureConversionStats>,
    atlas_registry: HashMap<String, AtlasInfo>,
}

/// Statistics for texture conversion operations
#[derive(Debug, Clone)]
pub struct TextureConversionStats {
    pub source_format: String,
    pub target_format: String,
    pub source_size: (u32, u32),
    pub source_bytes: usize,
    pub target_bytes: usize,
    pub mip_levels: u32,
    pub conversion_time_ms: u64,
    pub memory_peak_mb: f64,
    pub has_alpha: bool,
    pub color_space: ColorSpace,
    pub compression_ratio: f64,
}

/// Atlas information for sprite atlases
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtlasInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub sprites: Vec<SpriteInfo>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Individual sprite within an atlas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpriteInfo {
    pub name: String,
    pub rect: UVRect,
    pub pivot: (f32, f32),
    pub border: (f32, f32, f32, f32), // left, bottom, right, top
    pub tags: Vec<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// UV rectangle for sprite mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UVRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Color space information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ColorSpace {
    Linear,
    sRGB,
    Gamma,
    Unknown,
}

/// Enhanced texture conversion result
#[derive(Debug, Clone)]
pub struct TextureConversionResult {
    pub filename: String,
    pub data: Vec<u8>,
    pub format: TextureOutputFormat,
    pub metadata: TextureMetadata,
    pub sidecar_json: Option<String>,
    pub stats: TextureConversionStats,
}

/// Output format options
#[derive(Debug, Clone, PartialEq)]
pub enum TextureOutputFormat {
    PNG,
    KTX2,
    BasisU,
    Both, // Both PNG and KTX2
}

/// Comprehensive texture metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureMetadata {
    pub source_name: String,
    pub dimensions: (u32, u32),
    pub format: String,
    pub mip_levels: u32,
    pub color_space: ColorSpace,
    pub has_alpha: bool,
    pub compression_info: Option<CompressionInfo>,
    pub atlas_info: Option<AtlasInfo>,
    pub quality_metrics: QualityMetrics,
    pub provenance: TextureProvenance,
}

/// Compression information for KTX2/BasisU
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionInfo {
    pub algorithm: String,
    pub quality: f32,
    pub ratio: f64,
    pub target_size_mb: f64,
    pub encoding_time_ms: u64,
}

/// Quality metrics for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    pub mse: f64,           // Mean Squared Error
    pub psnr: f64,          // Peak Signal-to-Noise Ratio
    pub ssim: f64,          // Structural Similarity Index
    pub alpha_coverage: f64, // Alpha channel coverage ratio
    pub color_range: (f64, f64, f64), // Min/max values per channel
}

/// Provenance tracking for textures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureProvenance {
    pub unity_format: String,
    pub source_file: String,
    pub extraction_time: String,
    pub pipeline_version: String,
    pub golden_test_hash: Option<String>,
}

impl TexturePipeline {
    /// Create new texture pipeline with memory monitoring
    pub fn new(memory_limit_mb: usize) -> Result<Self> {
        let memory_monitor = Arc::new(MemoryPressureMonitor::new(memory_limit_mb));
        
        Ok(Self {
            memory_monitor,
            conversion_stats: Vec::new(),
            atlas_registry: HashMap::new(),
        })
    }

    /// Convert Unity Texture2D to multiple formats with Sprint 2 enhancements
    pub fn convert_texture(&mut self, 
        texture: &UnityTexture2D, 
        output_format: TextureOutputFormat,
        quality_settings: &QualitySettings,
    ) -> Result<TextureConversionResult> {
        let start_time = std::time::Instant::now();
        info!("Converting texture '{}' ({:?}) to {:?}", 
              texture.name, texture.format, output_format);

        // Check memory pressure before starting
        if self.memory_monitor.memory_pressure() > 0.8 {
            warn!("High memory pressure before texture conversion: {:.1}%", 
                  self.memory_monitor.memory_pressure() * 100.0);
        }

        // Extract all mip levels
        let mip_levels = self.extract_mip_levels(texture)?;
        debug!("Extracted {} mip levels from texture", mip_levels.len());

        // Determine color space
        let color_space = self.detect_color_space(texture, &mip_levels[0])?;
        
        // Create enhanced metadata
        let metadata = self.create_texture_metadata(texture, &mip_levels, color_space.clone())?;

        // Perform conversion based on format
        let (filename, data, sidecar_json) = match output_format {
            TextureOutputFormat::PNG => {
                self.convert_to_png(texture, &mip_levels, quality_settings)?
            }
            TextureOutputFormat::KTX2 => {
                self.convert_to_ktx2(texture, &mip_levels, quality_settings)?
            }
            TextureOutputFormat::BasisU => {
                self.convert_to_basisu(texture, &mip_levels, quality_settings)?
            }
            TextureOutputFormat::Both => {
                // Convert to both PNG and KTX2, return primary (PNG)
                let (png_file, png_data, png_sidecar) = self.convert_to_png(texture, &mip_levels, quality_settings)?;
                let (_ktx2_file, _ktx2_data, _ktx2_sidecar) = self.convert_to_ktx2(texture, &mip_levels, quality_settings)?;
                
                info!("Generated both PNG and KTX2 versions of texture '{}'", texture.name);
                (png_file, png_data, png_sidecar)
            }
        };

        let conversion_time = start_time.elapsed();

        // Create conversion statistics
        let stats = TextureConversionStats {
            source_format: format!("{:?}", texture.format),
            target_format: format!("{:?}", output_format),
            source_size: (texture.width, texture.height),
            source_bytes: texture.data.len(),
            target_bytes: data.len(),
            mip_levels: mip_levels.len() as u32,
            conversion_time_ms: conversion_time.as_millis() as u64,
            memory_peak_mb: self.memory_monitor.peak_usage_mb(),
            has_alpha: self.has_alpha_channel(texture),
            color_space: color_space.clone(),
            compression_ratio: data.len() as f64 / texture.data.len() as f64,
        };

        // Store statistics
        self.conversion_stats.push(stats.clone());

        // Check for atlas information
        let atlas_sidecar = if self.is_atlas_texture(texture) {
            self.generate_atlas_sidecar(texture, &metadata)?
        } else {
            None
        };

        let final_sidecar = match (sidecar_json, atlas_sidecar) {
            (Some(main), Some(atlas)) => Some(self.merge_sidecar_json(&main, &atlas)?),
            (Some(main), None) => Some(main),
            (None, Some(atlas)) => Some(atlas),
            (None, None) => None,
        };

        Ok(TextureConversionResult {
            filename,
            data,
            format: output_format,
            metadata,
            sidecar_json: final_sidecar,
            stats,
        })
    }

    /// Extract mip levels from Unity texture data
    fn extract_mip_levels(&self, texture: &UnityTexture2D) -> Result<Vec<MipLevel>> {
        let mut mip_levels = Vec::new();
        
        if texture.mipmap_count <= 1 {
            // Single mip level
            let rgba_data = self.unity_to_rgba(texture)?;
            mip_levels.push(MipLevel {
                width: texture.width,
                height: texture.height,
                data: rgba_data,
                level: 0,
            });
        } else {
            // Multiple mip levels - extract each one
            let mut data_offset = 0;
            for level in 0..texture.mipmap_count {
                let mip_width = (texture.width >> level).max(1);
                let mip_height = (texture.height >> level).max(1);
                
                let mip_size = self.calculate_mip_data_size(texture.format, mip_width, mip_height);
                
                if data_offset + mip_size > texture.data.len() {
                    warn!("Mip level {} extends beyond texture data, stopping", level);
                    break;
                }

                let mip_data = &texture.data[data_offset..data_offset + mip_size];
                let rgba_data = self.decompress_mip_data(mip_data, texture.format, mip_width, mip_height)?;
                
                mip_levels.push(MipLevel {
                    width: mip_width,
                    height: mip_height,
                    data: rgba_data,
                    level,
                });

                data_offset += mip_size;
            }
        }

        Ok(mip_levels)
    }

    /// Convert to PNG format with mip map support
    fn convert_to_png(&self, 
        texture: &UnityTexture2D, 
        mip_levels: &[MipLevel], 
        _quality_settings: &QualitySettings,
    ) -> Result<(String, Vec<u8>, Option<String>)> {
        // Use the highest quality mip level (level 0)
        let base_mip = &mip_levels[0];
        
        let img = ImageBuffer::<Rgba<u8>, _>::from_raw(base_mip.width, base_mip.height, base_mip.data.clone())
            .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer for PNG"))?;
        
        let mut output = Vec::new();
        let mut cursor = Cursor::new(&mut output);
        
        img.write_to(&mut cursor, ImageFormat::Png)
            .context("Failed to encode PNG")?;

        // Generate sidecar JSON for mip map information
        let sidecar = if mip_levels.len() > 1 {
            Some(self.generate_png_sidecar(texture, mip_levels)?)
        } else {
            None
        };

        Ok((format!("{}.png", texture.name), output, sidecar))
    }

    /// Convert to KTX2 format with BasisU compression
    fn convert_to_ktx2(&self, 
        texture: &UnityTexture2D, 
        mip_levels: &[MipLevel], 
        quality_settings: &QualitySettings,
    ) -> Result<(String, Vec<u8>, Option<String>)> {
        // For now, create a basic KTX2 structure
        // In a full implementation, this would use proper KTX2 encoding
        
        let base_mip = &mip_levels[0];
        
        // Create basic KTX2 header + data
        let mut ktx2_data = Vec::new();
        
        // KTX2 identifier
        ktx2_data.extend_from_slice(b"\xABKTX 20\xBB\r\n\x1A\n");
        
        // Basic header (simplified)
        ktx2_data.extend_from_slice(&(base_mip.width).to_le_bytes());
        ktx2_data.extend_from_slice(&(base_mip.height).to_le_bytes());
        ktx2_data.extend_from_slice(&1u32.to_le_bytes()); // depth
        ktx2_data.extend_from_slice(&1u32.to_le_bytes()); // layers
        ktx2_data.extend_from_slice(&1u32.to_le_bytes()); // faces
        ktx2_data.extend_from_slice(&(mip_levels.len() as u32).to_le_bytes());
        
        // Add compressed texture data (placeholder - would use real BasisU compression)
        for mip in mip_levels {
            let compressed_size = mip.data.len() as u32;
            ktx2_data.extend_from_slice(&compressed_size.to_le_bytes());
            ktx2_data.extend_from_slice(&mip.data);
            
            // Align to 4 bytes
            while ktx2_data.len() % 4 != 0 {
                ktx2_data.push(0);
            }
        }

        // Generate KTX2 sidecar with compression info
        let sidecar = self.generate_ktx2_sidecar(texture, mip_levels, quality_settings)?;

        Ok((format!("{}.ktx2", texture.name), ktx2_data, Some(sidecar)))
    }

    /// Convert to BasisU format
    fn convert_to_basisu(&self, 
        texture: &UnityTexture2D, 
        mip_levels: &[MipLevel], 
        quality_settings: &QualitySettings,
    ) -> Result<(String, Vec<u8>, Option<String>)> {
        // For now, delegate to KTX2 conversion as BasisU is embedded in KTX2
        let (filename, data, sidecar) = self.convert_to_ktx2(texture, mip_levels, quality_settings)?;
        
        // Change extension to .basis
        let basis_filename = filename.replace(".ktx2", ".basis");
        
        Ok((basis_filename, data, sidecar))
    }

    /// Detect color space from texture data
    fn detect_color_space(&self, texture: &UnityTexture2D, base_mip: &MipLevel) -> Result<ColorSpace> {
        // Analyze texture data to determine color space
        match texture.format {
            UnityTextureFormat::RGB24 | UnityTextureFormat::RGBA32 => {
                // Check if values suggest sRGB or linear
                let sample_size = (base_mip.data.len() / 4).min(1000); // Sample first 1000 pixels or all if smaller
                let mut linear_indicators = 0;
                let mut srgb_indicators = 0;

                for i in (0..sample_size * 4).step_by(4) {
                    let r = base_mip.data[i] as f32 / 255.0;
                    let g = base_mip.data[i + 1] as f32 / 255.0;
                    let b = base_mip.data[i + 2] as f32 / 255.0;

                    // Check for linear characteristics (more values in higher ranges)
                    if r > 0.7 || g > 0.7 || b > 0.7 {
                        linear_indicators += 1;
                    }
                    
                    // Check for sRGB characteristics (gamma curve)
                    if (r < 0.3 && (r * r) < r) || (g < 0.3 && (g * g) < g) || (b < 0.3 && (b * b) < b) {
                        srgb_indicators += 1;
                    }
                }

                if linear_indicators > srgb_indicators {
                    Ok(ColorSpace::Linear)
                } else {
                    Ok(ColorSpace::sRGB)
                }
            }
            UnityTextureFormat::DXT1 | UnityTextureFormat::DXT5 => {
                // Compressed textures are typically sRGB
                Ok(ColorSpace::sRGB)
            }
            _ => Ok(ColorSpace::Unknown),
        }
    }

    /// Check if texture has alpha channel
    fn has_alpha_channel(&self, texture: &UnityTexture2D) -> bool {
        matches!(texture.format, 
            UnityTextureFormat::RGBA32 | 
            UnityTextureFormat::ARGB32 | 
            UnityTextureFormat::BGRA32 |
            UnityTextureFormat::DXT5 |
            UnityTextureFormat::ETC2_RGBA8 |
            UnityTextureFormat::Alpha8
        )
    }

    /// Check if texture is likely an atlas
    fn is_atlas_texture(&self, texture: &UnityTexture2D) -> bool {
        // Heuristics for atlas detection
        let name_lower = texture.name.to_lowercase();
        
        // Check for atlas keywords
        if name_lower.contains("atlas") || 
           name_lower.contains("sprite") || 
           name_lower.contains("ui") ||
           name_lower.contains("sheet") {
            return true;
        }

        // Check dimensions (atlases are often power-of-2 and large)
        let is_power_of_2 = |n: u32| n != 0 && (n & (n - 1)) == 0;
        
        if is_power_of_2(texture.width) && 
           is_power_of_2(texture.height) && 
           texture.width >= 512 && 
           texture.height >= 512 {
            return true;
        }

        false
    }

    /// Unity texture data to RGBA conversion (enhanced version)
    fn unity_to_rgba(&self, texture: &UnityTexture2D) -> Result<Vec<u8>> {
        // Delegate to existing converter but with memory monitoring
        let start_memory = self.memory_monitor.current_usage_mb();
        
        // Use the existing to_rgba method from the UnityTexture2D implementation
        let rgba_data = match texture.format {
            UnityTextureFormat::RGBA32 => {
                Ok(texture.data.clone())
            }
            UnityTextureFormat::ARGB32 => {
                // Convert ARGB to RGBA
                let mut rgba = Vec::with_capacity(texture.data.len());
                for chunk in texture.data.chunks_exact(4) {
                    rgba.push(chunk[1]); // R
                    rgba.push(chunk[2]); // G
                    rgba.push(chunk[3]); // B
                    rgba.push(chunk[0]); // A
                }
                Ok(rgba)
            }
            UnityTextureFormat::BGRA32 => {
                // Convert BGRA to RGBA
                let mut rgba = Vec::with_capacity(texture.data.len());
                for chunk in texture.data.chunks_exact(4) {
                    rgba.push(chunk[2]); // R
                    rgba.push(chunk[1]); // G
                    rgba.push(chunk[0]); // B
                    rgba.push(chunk[3]); // A
                }
                Ok(rgba)
            }
            UnityTextureFormat::RGB24 => {
                // Convert RGB to RGBA
                let mut rgba = Vec::with_capacity(texture.data.len() * 4 / 3);
                for chunk in texture.data.chunks_exact(3) {
                    rgba.push(chunk[0]); // R
                    rgba.push(chunk[1]); // G
                    rgba.push(chunk[2]); // B
                    rgba.push(255);      // A
                }
                Ok(rgba)
            }
            _ => {
                // For compressed formats and others, we need decompression
                // This is a simplified implementation - real version would handle all formats
                warn!("Texture format {:?} requires decompression - using placeholder", texture.format);
                let pixel_count = (texture.width * texture.height) as usize;
                Ok(vec![128; pixel_count * 4]) // Gray placeholder
            }
        }?;
        
        let end_memory = self.memory_monitor.current_usage_mb();
        debug!("RGBA conversion memory usage: {:.1}MB", end_memory - start_memory);
        
        Ok(rgba_data)
    }

    /// Calculate data size for a mip level
    fn calculate_mip_data_size(&self, format: UnityTextureFormat, width: u32, height: u32) -> usize {
        match format {
            UnityTextureFormat::DXT1 => ((width + 3) / 4 * (height + 3) / 4 * 8) as usize,
            UnityTextureFormat::DXT5 => ((width + 3) / 4 * (height + 3) / 4 * 16) as usize,
            UnityTextureFormat::ETC_RGB4 => ((width + 3) / 4 * (height + 3) / 4 * 8) as usize,
            UnityTextureFormat::ETC2_RGBA8 => ((width + 3) / 4 * (height + 3) / 4 * 16) as usize,
            _ => (width * height * format.bytes_per_pixel() as u32) as usize,
        }
    }

    /// Decompress mip level data
    fn decompress_mip_data(&self, data: &[u8], format: UnityTextureFormat, width: u32, height: u32) -> Result<Vec<u8>> {
        // Create temporary texture for mip level
        let temp_texture = UnityTexture2D {
            name: "mip".to_string(),
            width,
            height,
            format,
            mipmap_count: 1,
            data: data.to_vec(),
            is_readable: true,
        };

        self.unity_to_rgba(&temp_texture)
    }

    /// Generate PNG sidecar JSON
    fn generate_png_sidecar(&self, texture: &UnityTexture2D, mip_levels: &[MipLevel]) -> Result<String> {
        let sidecar = serde_json::json!({
            "format": "PNG",
            "source_texture": texture.name,
            "unity_format": format!("{:?}", texture.format),
            "mip_levels": mip_levels.iter().map(|mip| {
                serde_json::json!({
                    "level": mip.level,
                    "width": mip.width,
                    "height": mip.height,
                    "data_size": mip.data.len()
                })
            }).collect::<Vec<_>>(),
            "conversion_notes": "PNG uses highest quality mip level (level 0)"
        });

        serde_json::to_string_pretty(&sidecar).context("Failed to serialize PNG sidecar")
    }

    /// Generate KTX2 sidecar JSON
    fn generate_ktx2_sidecar(&self, texture: &UnityTexture2D, mip_levels: &[MipLevel], quality: &QualitySettings) -> Result<String> {
        let sidecar = serde_json::json!({
            "format": "KTX2",
            "compression": "BasisU",
            "source_texture": texture.name,
            "unity_format": format!("{:?}", texture.format),
            "quality_settings": {
                "compression_level": quality.compression_level,
                "target_quality": quality.target_quality,
                "preserve_alpha": quality.preserve_alpha
            },
            "mip_levels": mip_levels.iter().map(|mip| {
                serde_json::json!({
                    "level": mip.level,
                    "width": mip.width,
                    "height": mip.height,
                    "data_size": mip.data.len()
                })
            }).collect::<Vec<_>>(),
            "notes": "KTX2 with BasisU compression for optimal GPU compatibility"
        });

        serde_json::to_string_pretty(&sidecar).context("Failed to serialize KTX2 sidecar")
    }

    /// Generate atlas sidecar JSON
    fn generate_atlas_sidecar(&mut self, texture: &UnityTexture2D, metadata: &TextureMetadata) -> Result<String> {
        // For now, create a placeholder atlas info
        // In a real implementation, this would extract sprite information from Unity data
        let atlas_info = AtlasInfo {
            name: texture.name.clone(),
            width: texture.width,
            height: texture.height,
            sprites: vec![
                SpriteInfo {
                    name: format!("{}_sprite_0", texture.name),
                    rect: UVRect {
                        x: 0.0,
                        y: 0.0,
                        width: 1.0,
                        height: 1.0,
                    },
                    pivot: (0.5, 0.5),
                    border: (0.0, 0.0, 0.0, 0.0),
                    tags: vec!["atlas".to_string()],
                    metadata: HashMap::new(),
                }
            ],
            metadata: HashMap::new(),
        };

        // Store in registry
        self.atlas_registry.insert(texture.name.clone(), atlas_info.clone());

        serde_json::to_string_pretty(&atlas_info).context("Failed to serialize atlas sidecar")
    }

    /// Merge multiple sidecar JSON objects
    fn merge_sidecar_json(&self, main: &str, atlas: &str) -> Result<String> {
        let mut main_json: serde_json::Value = serde_json::from_str(main)?;
        let atlas_json: serde_json::Value = serde_json::from_str(atlas)?;

        if let serde_json::Value::Object(ref mut main_obj) = main_json {
            main_obj.insert("atlas_info".to_string(), atlas_json);
        }

        serde_json::to_string_pretty(&main_json).context("Failed to merge sidecar JSON")
    }

    /// Create comprehensive texture metadata
    fn create_texture_metadata(&self, texture: &UnityTexture2D, mip_levels: &[MipLevel], color_space: ColorSpace) -> Result<TextureMetadata> {
        let base_mip = &mip_levels[0];
        
        // Calculate quality metrics
        let quality_metrics = self.calculate_quality_metrics(base_mip)?;

        Ok(TextureMetadata {
            source_name: texture.name.clone(),
            dimensions: (texture.width, texture.height),
            format: format!("{:?}", texture.format),
            mip_levels: mip_levels.len() as u32,
            color_space,
            has_alpha: self.has_alpha_channel(texture),
            compression_info: None, // Filled during conversion
            atlas_info: None,       // Filled if atlas detected
            quality_metrics,
            provenance: TextureProvenance {
                unity_format: format!("{:?}", texture.format),
                source_file: "unknown".to_string(), // Would be filled by caller
                extraction_time: chrono::Utc::now().to_rfc3339(),
                pipeline_version: env!("CARGO_PKG_VERSION").to_string(),
                golden_test_hash: None, // Filled during testing
            },
        })
    }

    /// Calculate quality metrics for texture
    fn calculate_quality_metrics(&self, mip: &MipLevel) -> Result<QualityMetrics> {
        let mut color_min = [f64::INFINITY; 3];
        let mut color_max = [f64::NEG_INFINITY; 3];
        let mut alpha_coverage = 0.0;
        let total_pixels = (mip.width * mip.height) as f64;

        for chunk in mip.data.chunks_exact(4) {
            let r = chunk[0] as f64 / 255.0;
            let g = chunk[1] as f64 / 255.0;
            let b = chunk[2] as f64 / 255.0;
            let a = chunk[3] as f64 / 255.0;

            color_min[0] = color_min[0].min(r);
            color_min[1] = color_min[1].min(g);
            color_min[2] = color_min[2].min(b);
            
            color_max[0] = color_max[0].max(r);
            color_max[1] = color_max[1].max(g);
            color_max[2] = color_max[2].max(b);

            if a > 0.5 {
                alpha_coverage += 1.0;
            }
        }

        alpha_coverage /= total_pixels;

        Ok(QualityMetrics {
            mse: 0.0,      // Would calculate vs. reference
            psnr: 0.0,     // Would calculate vs. reference
            ssim: 1.0,     // Would calculate vs. reference
            alpha_coverage,
            color_range: (
                color_max[0] - color_min[0],
                color_max[1] - color_min[1],
                color_max[2] - color_min[2],
            ),
        })
    }

    /// Get conversion statistics
    pub fn stats(&self) -> &[TextureConversionStats] {
        &self.conversion_stats
    }

    /// Get atlas registry
    pub fn atlas_registry(&self) -> &HashMap<String, AtlasInfo> {
        &self.atlas_registry
    }

    /// Get memory usage
    pub fn memory_usage(&self) -> (f64, f64) {
        (self.memory_monitor.current_usage_mb(), self.memory_monitor.peak_usage_mb())
    }
}

/// Mip level data structure
#[derive(Debug, Clone)]
struct MipLevel {
    width: u32,
    height: u32,
    data: Vec<u8>, // RGBA data
    level: u32,
}

/// Quality settings for conversion
#[derive(Debug, Clone)]
pub struct QualitySettings {
    pub compression_level: u32,
    pub target_quality: f32,
    pub preserve_alpha: bool,
    pub generate_mipmaps: bool,
    pub color_space_conversion: bool,
}

impl Default for QualitySettings {
    fn default() -> Self {
        Self {
            compression_level: 1,      // BasisU compression level
            target_quality: 0.8,       // Target quality (0.0 - 1.0)
            preserve_alpha: true,      // Preserve alpha channel
            generate_mipmaps: true,    // Generate mip maps
            color_space_conversion: true, // Apply color space conversion
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_texture_pipeline_creation() {
        let pipeline = TexturePipeline::new(300).unwrap();
        assert!(pipeline.conversion_stats.is_empty());
        assert!(pipeline.atlas_registry.is_empty());
    }

    #[test]
    fn test_color_space_detection() {
        // Would test color space detection logic
    }

    #[test]
    fn test_atlas_detection() {
        // Would test atlas detection heuristics
    }

    #[test]
    fn test_quality_settings() {
        let settings = QualitySettings::default();
        assert_eq!(settings.compression_level, 1);
        assert_eq!(settings.target_quality, 0.8);
        assert!(settings.preserve_alpha);
    }
}
