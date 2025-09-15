# Data Pipeline Spike - Asset Preprocessing for AI Tagging

## Spike Overview

Create preprocessing pipeline to convert Unity assets into AI-compatible formats (images, thumbnails, waveforms) for CLIP and cloud processing.

## Asset Type Processing

### 1. Texture2D → Image Processing

#### Input Formats (from Unity Plugin)
- RGBA32, ARGB32, BGRA32 (uncompressed)
- RGB24 (no alpha channel)
- Alpha8 (grayscale)
- DXT1, DXT5 (compressed - limited support)

#### Processing Pipeline
```rust
// In aegis-plugins/unity/src/ai_preprocessing.rs

use image::{ImageBuffer, RgbaImage, DynamicImage};
use anyhow::Result;

pub struct TexturePreprocessor;

impl TexturePreprocessor {
    /// Convert Unity Texture2D to standardized RGBA image for AI processing
    pub fn process_texture(texture_data: &[u8], format: UnityTextureFormat, width: u32, height: u32) -> Result<RgbaImage> {
        let rgba_data = match format {
            UnityTextureFormat::RGBA32 => texture_data.to_vec(),
            UnityTextureFormat::ARGB32 => convert_argb_to_rgba(texture_data),
            UnityTextureFormat::BGRA32 => convert_bgra_to_rgba(texture_data),
            UnityTextureFormat::RGB24 => add_alpha_channel(texture_data),
            UnityTextureFormat::Alpha8 => convert_alpha8_to_rgba(texture_data),
            _ => return Err(anyhow::anyhow!("Unsupported texture format: {:?}", format)),
        };
        
        // Create image buffer
        let image = ImageBuffer::from_raw(width, height, rgba_data)
            .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;
        
        // Resize for AI processing if needed (CLIP optimal: 224x224)
        let resized = if width != 224 || height != 224 {
            image::imageops::resize(&image, 224, 224, image::imageops::FilterType::Lanczos3)
        } else {
            image
        };
        
        Ok(resized)
    }
    
    /// Generate thumbnail for UI display (128x128)
    pub fn generate_thumbnail(image: &RgbaImage) -> Result<Vec<u8>> {
        let thumbnail = image::imageops::resize(image, 128, 128, image::imageops::FilterType::Triangle);
        
        let mut output = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut output);
        thumbnail.write_to(&mut cursor, image::ImageFormat::Png)?;
        
        Ok(output)
    }
}

// Helper functions for format conversion
fn convert_argb_to_rgba(argb_data: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(argb_data.len());
    for chunk in argb_data.chunks_exact(4) {
        rgba.push(chunk[1]); // R
        rgba.push(chunk[2]); // G  
        rgba.push(chunk[3]); // B
        rgba.push(chunk[0]); // A
    }
    rgba
}

fn convert_bgra_to_rgba(bgra_data: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(bgra_data.len());
    for chunk in bgra_data.chunks_exact(4) {
        rgba.push(chunk[2]); // R
        rgba.push(chunk[1]); // G
        rgba.push(chunk[0]); // B
        rgba.push(chunk[3]); // A
    }
    rgba
}

fn add_alpha_channel(rgb_data: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(rgb_data.len() * 4 / 3);
    for chunk in rgb_data.chunks_exact(3) {
        rgba.push(chunk[0]); // R
        rgba.push(chunk[1]); // G
        rgba.push(chunk[2]); // B
        rgba.push(255);      // A (opaque)
    }
    rgba
}
```

#### Output Specifications
- **AI Processing**: 224x224 RGBA PNG (CLIP optimal size)
- **Thumbnail**: 128x128 PNG for UI display  
- **Performance Target**: <300ms preprocessing per texture
- **Quality**: Preserve aspect ratio with letterboxing if needed

### 2. Mesh → Visual Representation

#### Challenge
Meshes don't have inherent visual representation - need to render thumbnails.

#### Solution: Wireframe + Silhouette Generation
```rust
// In aegis-plugins/unity/src/mesh_preview.rs

use nalgebra::{Vector3, Matrix4};
use image::{RgbaImage, Rgba};

pub struct MeshPreprocessor;

impl MeshPreprocessor {
    /// Generate wireframe preview of mesh for AI tagging
    pub fn generate_wireframe_preview(vertices: &[[f32; 3]], triangles: &[u32]) -> Result<RgbaImage> {
        let mut image = RgbaImage::new(224, 224);
        let white = Rgba([255, 255, 255, 255]);
        let black = Rgba([0, 0, 0, 255]);
        
        // Fill with black background
        for pixel in image.pixels_mut() {
            *pixel = black;
        }
        
        // Calculate bounding box and normalize coordinates
        let (min_bounds, max_bounds) = calculate_bounds(vertices);
        let scale = 200.0 / max_component(max_bounds - min_bounds); // Leave margin
        let center = Vector3::new(112.0, 112.0, 0.0); // Center of 224x224 image
        
        // Draw wireframe edges
        for triangle in triangles.chunks_exact(3) {
            let v1 = project_vertex(vertices[triangle[0] as usize], min_bounds, scale, center);
            let v2 = project_vertex(vertices[triangle[1] as usize], min_bounds, scale, center);
            let v3 = project_vertex(vertices[triangle[2] as usize], min_bounds, scale, center);
            
            // Draw triangle edges
            draw_line(&mut image, v1, v2, white);
            draw_line(&mut image, v2, v3, white);
            draw_line(&mut image, v3, v1, white);
        }
        
        Ok(image)
    }
    
    /// Extract mesh metadata for AI tagging
    pub fn extract_mesh_features(vertices: &[[f32; 3]], triangles: &[u32]) -> MeshFeatures {
        MeshFeatures {
            vertex_count: vertices.len(),
            triangle_count: triangles.len() / 3,
            bounding_box: calculate_bounds(vertices),
            surface_area: calculate_surface_area(vertices, triangles),
            is_watertight: check_watertight(vertices, triangles),
        }
    }
}

pub struct MeshFeatures {
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub bounding_box: (Vector3<f32>, Vector3<f32>), // (min, max)
    pub surface_area: f32,
    pub is_watertight: bool,
}

// Helper functions for mesh processing
fn project_vertex(vertex: [f32; 3], min_bounds: Vector3<f32>, scale: f32, center: Vector3<f32>) -> (i32, i32) {
    let normalized_x = (vertex[0] - min_bounds.x) * scale;
    let normalized_y = (vertex[1] - min_bounds.y) * scale;
    
    let screen_x = (center.x + normalized_x) as i32;
    let screen_y = (center.y + normalized_y) as i32;
    
    (screen_x, screen_y)
}

fn draw_line(image: &mut RgbaImage, start: (i32, i32), end: (i32, i32), color: Rgba<u8>) {
    // Bresenham's line algorithm (simplified)
    // Implementation would draw pixels between start and end points
}
```

#### Alternative: Silhouette Rendering
For more recognizable shapes, render filled silhouette:
- Project mesh to 2D plane (front view)
- Fill interior using scanline algorithm
- Generate multiple viewpoints (front, side, top) for better recognition

### 3. AudioClip → Waveform Visualization

#### Processing Pipeline
```rust
// In aegis-plugins/unity/src/audio_preview.rs

use image::{RgbaImage, Rgba};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

pub struct AudioPreprocessor;

impl AudioPreprocessor {
    /// Generate waveform visualization for audio clip
    pub fn generate_waveform(audio_data: &[u8], channels: u32, sample_rate: u32, bits_per_sample: u32) -> Result<RgbaImage> {
        let mut image = RgbaImage::new(224, 224);
        let background = Rgba([20, 20, 30, 255]); // Dark blue background
        let waveform_color = Rgba([0, 255, 100, 255]); // Bright green waveform
        let center_line = Rgba([100, 100, 100, 255]); // Gray center line
        
        // Fill background
        for pixel in image.pixels_mut() {
            *pixel = background;
        }
        
        // Draw center line
        for x in 0..224 {
            image.put_pixel(x, 112, center_line);
        }
        
        // Parse audio samples
        let samples = parse_audio_samples(audio_data, bits_per_sample)?;
        
        // Downsample to fit 224 pixels width
        let samples_per_pixel = samples.len() / 224;
        
        for (pixel_x, chunk) in samples.chunks(samples_per_pixel).enumerate() {
            if pixel_x >= 224 { break; }
            
            // Calculate RMS (root mean square) for this chunk
            let rms = calculate_rms(chunk);
            
            // Convert to pixel height (112 pixels above/below center)
            let amplitude_pixels = (rms * 112.0) as u32;
            
            // Draw waveform column
            for y_offset in 0..amplitude_pixels {
                if 112 + y_offset < 224 {
                    image.put_pixel(pixel_x as u32, 112 + y_offset, waveform_color);
                }
                if 112 >= y_offset {
                    image.put_pixel(pixel_x as u32, 112 - y_offset, waveform_color);
                }
            }
        }
        
        Ok(image)
    }
    
    /// Generate spectrogram for frequency analysis
    pub fn generate_spectrogram(audio_data: &[u8], sample_rate: u32) -> Result<RgbaImage> {
        // FFT-based spectrogram generation
        // Would use rustfft crate for frequency domain analysis
        todo!("Spectrogram generation - Phase 2 enhancement")
    }
    
    /// Extract audio features for AI tagging
    pub fn extract_audio_features(audio_data: &[u8], channels: u32, sample_rate: u32) -> AudioFeatures {
        let samples = parse_audio_samples(audio_data, 16).unwrap_or_default();
        
        AudioFeatures {
            duration_seconds: samples.len() as f32 / (sample_rate * channels) as f32,
            peak_amplitude: find_peak_amplitude(&samples),
            rms_amplitude: calculate_rms(&samples),
            zero_crossing_rate: calculate_zero_crossings(&samples),
            estimated_frequency: estimate_dominant_frequency(&samples, sample_rate),
        }
    }
}

pub struct AudioFeatures {
    pub duration_seconds: f32,
    pub peak_amplitude: f32,
    pub rms_amplitude: f32,
    pub zero_crossing_rate: f32,
    pub estimated_frequency: f32,
}

fn parse_audio_samples(data: &[u8], bits_per_sample: u32) -> Result<Vec<f32>> {
    let mut cursor = Cursor::new(data);
    let mut samples = Vec::new();
    
    match bits_per_sample {
        16 => {
            while let Ok(sample) = cursor.read_i16::<LittleEndian>() {
                // Normalize to [-1.0, 1.0] range
                samples.push(sample as f32 / 32768.0);
            }
        }
        8 => {
            for &byte in data {
                // Convert unsigned 8-bit to signed and normalize
                samples.push((byte as i16 - 128) as f32 / 128.0);
            }
        }
        _ => return Err(anyhow::anyhow!("Unsupported bit depth: {}", bits_per_sample)),
    }
    
    Ok(samples)
}

fn calculate_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() { return 0.0; }
    
    let sum_squares: f32 = samples.iter().map(|s| s * s).sum();
    (sum_squares / samples.len() as f32).sqrt()
}
```

## Performance Optimization

### Parallel Processing
```rust
// In aegis-plugins/unity/src/parallel_processing.rs

use rayon::prelude::*;
use std::sync::Arc;

pub struct BatchProcessor;

impl BatchProcessor {
    /// Process multiple assets in parallel
    pub async fn process_batch(assets: Vec<UnityAsset>) -> Result<Vec<ProcessedAsset>> {
        let results: Result<Vec<_>, _> = assets
            .into_par_iter()
            .map(|asset| Self::process_single_asset(asset))
            .collect();
            
        results
    }
    
    fn process_single_asset(asset: UnityAsset) -> Result<ProcessedAsset> {
        let start_time = std::time::Instant::now();
        
        let preview_image = match asset.asset_type {
            AssetType::Texture2D(texture) => {
                TexturePreprocessor::process_texture(&texture.data, texture.format, texture.width, texture.height)?
            },
            AssetType::Mesh(mesh) => {
                MeshPreprocessor::generate_wireframe_preview(&mesh.vertices, &mesh.triangles)?
            },
            AssetType::AudioClip(audio) => {
                AudioPreprocessor::generate_waveform(&audio.data, audio.channels, audio.sample_rate, audio.bits_per_sample)?
            },
        };
        
        let processing_time = start_time.elapsed().as_millis();
        
        Ok(ProcessedAsset {
            asset_id: asset.id,
            preview_image,
            processing_time_ms: processing_time as u32,
            features: extract_features(&asset)?,
        })
    }
}
```

### Caching Strategy
```rust
// In aegis-plugins/unity/src/preprocessing_cache.rs

use blake3::Hash;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct PreprocessingCache {
    cache_dir: PathBuf,
}

impl PreprocessingCache {
    /// Check if preprocessed version exists
    pub fn get_cached(&self, asset_hash: &str) -> Option<ProcessedAsset> {
        let cache_path = self.cache_dir.join(format!("{}.cache", asset_hash));
        
        if cache_path.exists() {
            // Load from cache (use bincode for performance)
            std::fs::read(&cache_path)
                .ok()
                .and_then(|data| bincode::deserialize(&data).ok())
        } else {
            None
        }
    }
    
    /// Store processed asset in cache
    pub fn store_cached(&self, asset_hash: &str, processed: &ProcessedAsset) -> Result<()> {
        let cache_path = self.cache_dir.join(format!("{}.cache", asset_hash));
        let serialized = bincode::serialize(processed)?;
        std::fs::write(cache_path, serialized)?;
        Ok(())
    }
    
    /// Calculate consistent hash for asset
    pub fn calculate_asset_hash(asset_data: &[u8], metadata: &AssetMetadata) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(asset_data);
        hasher.update(&metadata.width.to_le_bytes());
        hasher.update(&metadata.height.to_le_bytes());
        hasher.update(metadata.format.to_string().as_bytes());
        hasher.finalize().to_hex().to_string()
    }
}
```

## Integration with AI Pipeline

### API Endpoints
```rust
// In aegis-core/src/api/preprocessing.rs

#[post("/api/v1/assets/{asset_id}/preprocess")]
pub async fn preprocess_asset(
    asset_id: web::Path<String>,
    processing_options: web::Json<PreprocessingOptions>,
) -> Result<HttpResponse> {
    let asset = load_unity_asset(&asset_id).await?;
    
    // Check cache first
    let cache = PreprocessingCache::new("./cache/preprocessing")?;
    let asset_hash = cache.calculate_asset_hash(&asset.data, &asset.metadata);
    
    if let Some(cached_result) = cache.get_cached(&asset_hash) {
        return Ok(HttpResponse::Ok().json(cached_result));
    }
    
    // Process asset
    let processed = BatchProcessor::process_single_asset(asset)?;
    
    // Store in cache
    cache.store_cached(&asset_hash, &processed)?;
    
    Ok(HttpResponse::Ok().json(processed))
}

#[derive(Deserialize)]
pub struct PreprocessingOptions {
    pub output_size: Option<u32>, // Default: 224 for AI, 128 for thumbnails
    pub quality: Option<String>,  // "fast", "balanced", "high"
    pub cache_enabled: Option<bool>,
}
```

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_texture_preprocessing_rgba32() {
        let test_data = create_test_rgba_data(64, 64);
        let result = TexturePreprocessor::process_texture(
            &test_data, 
            UnityTextureFormat::RGBA32, 
            64, 
            64
        ).unwrap();
        
        assert_eq!(result.width(), 224);
        assert_eq!(result.height(), 224);
    }
    
    #[test]
    fn test_mesh_wireframe_generation() {
        let vertices = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.5, 1.0, 0.0],
        ];
        let triangles = vec![0, 1, 2];
        
        let result = MeshPreprocessor::generate_wireframe_preview(&vertices, &triangles).unwrap();
        
        assert_eq!(result.width(), 224);
        assert_eq!(result.height(), 224);
        // Verify that some pixels are white (wireframe) and some are black (background)
    }
    
    #[test]
    fn test_audio_waveform_generation() {
        let test_audio = create_test_sine_wave(44100, 1.0); // 1 second at 44.1kHz
        let result = AudioPreprocessor::generate_waveform(&test_audio, 1, 44100, 16).unwrap();
        
        assert_eq!(result.width(), 224);
        assert_eq!(result.height(), 224);
    }
    
    #[test]
    fn test_performance_target() {
        let test_texture = create_large_test_texture(512, 512);
        
        let start = std::time::Instant::now();
        let _result = TexturePreprocessor::process_texture(
            &test_texture,
            UnityTextureFormat::RGBA32,
            512,
            512
        ).unwrap();
        let duration = start.elapsed();
        
        assert!(duration.as_millis() < 300, "Processing took {}ms, target is <300ms", duration.as_millis());
    }
}
```

## Performance Benchmarks (Target)

| Asset Type | Input Size | Processing Time | Output Size | Memory Usage |
|------------|------------|-----------------|-------------|--------------|
| Texture2D | 512x512 RGBA | <200ms | 224x224 PNG | <50MB |
| Mesh | 10K vertices | <250ms | 224x224 PNG | <30MB |
| Audio | 30sec @ 44.1kHz | <150ms | 224x224 PNG | <20MB |

## Next Steps

### Sprint 1 Completion (Week 2)
- [ ] Implement texture preprocessing with format conversion
- [ ] Create basic mesh wireframe generation
- [ ] Implement audio waveform visualization
- [ ] Add caching system for processed assets
- [ ] Performance testing with real Unity assets

### Sprint 2 Integration (Week 3-4)
- [ ] Integrate preprocessing pipeline with AI tagging API
- [ ] Add batch processing capabilities
- [ ] Implement quality controls and error handling
- [ ] Add monitoring and metrics collection

---

**Status**: Implementation Ready  
**Performance Target**: <300ms per asset preprocessing  
**Dependencies**: Unity Plugin extraction functionality  
**Risk Level**: Low (well-defined image processing tasks)

**Critical Path**: Texture processing → AI integration → Batch optimization
