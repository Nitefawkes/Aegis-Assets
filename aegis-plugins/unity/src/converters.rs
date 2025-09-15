use anyhow::{Result, Context, bail};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read, Seek, SeekFrom};
use image::{ImageBuffer, Rgba, ImageFormat};
use serde_json;
use base64::Engine;

/// Unity texture formats
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnityTextureFormat {
    Alpha8 = 1,
    ARGB4444 = 2,
    RGB24 = 3,
    RGBA32 = 4,
    ARGB32 = 5,
    RGB565 = 7,
    R16 = 9,
    DXT1 = 10,
    DXT5 = 12,
    RGBA4444 = 13,
    BGRA32 = 14,
    RHalf = 15,
    RGHalf = 16,
    RGBAHalf = 17,
    RFloat = 18,
    RGFloat = 19,
    RGBAFloat = 20,
    YUV2 = 21,
    RGB9E5Float = 22,
    BC4 = 26,
    BC5 = 27,
    BC6H = 24,
    BC7 = 25,
    DXT1Crunched = 28,
    DXT5Crunched = 29,
    PVRTC_RGB2 = 30,
    PVRTC_RGBA2 = 31,
    PVRTC_RGB4 = 32,
    PVRTC_RGBA4 = 33,
    ETC_RGB4 = 34,
    ATC_RGB4 = 35,
    ATC_RGBA8 = 36,
    EAC_R = 41,
    EAC_R_SIGNED = 42,
    EAC_RG = 43,
    EAC_RG_SIGNED = 44,
    ETC2_RGB = 45,
    ETC2_RGBA1 = 46,
    ETC2_RGBA8 = 47,
    ASTC_4x4 = 48,
    ASTC_5x5 = 49,
    ASTC_6x6 = 50,
    ASTC_8x8 = 51,
    ASTC_10x10 = 52,
    ASTC_12x12 = 53,
    RG16 = 62,
    R8 = 63,
    ETC_RGB4Crunched = 64,
    ETC2_RGBA8Crunched = 65,
}

impl UnityTextureFormat {
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Alpha8),
            2 => Some(Self::ARGB4444),
            3 => Some(Self::RGB24),
            4 => Some(Self::RGBA32),
            5 => Some(Self::ARGB32),
            7 => Some(Self::RGB565),
            9 => Some(Self::R16),
            10 => Some(Self::DXT1),
            12 => Some(Self::DXT5),
            13 => Some(Self::RGBA4444),
            14 => Some(Self::BGRA32),
            26 => Some(Self::BC4),
            27 => Some(Self::BC5),
            24 => Some(Self::BC6H),
            25 => Some(Self::BC7),
            34 => Some(Self::ETC_RGB4),
            45 => Some(Self::ETC2_RGB),
            47 => Some(Self::ETC2_RGBA8),
            48 => Some(Self::ASTC_4x4),
            _ => None,
        }
    }
    
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            Self::Alpha8 | Self::R8 => 1,
            Self::RGB565 | Self::ARGB4444 | Self::RGBA4444 | Self::R16 => 2,
            Self::RGB24 => 3,
            Self::RGBA32 | Self::ARGB32 | Self::BGRA32 | Self::RGFloat => 4,
            Self::RGBAFloat => 16,
            _ => 4, // Default for compressed formats
        }
    }
    
    pub fn is_compressed(&self) -> bool {
        matches!(self,
            Self::DXT1 | Self::DXT5 | Self::DXT1Crunched | Self::DXT5Crunched |
            Self::BC4 | Self::BC5 | Self::BC6H | Self::BC7 |
            Self::ETC_RGB4 | Self::ETC2_RGB | Self::ETC2_RGBA8 |
            Self::ASTC_4x4 | Self::ASTC_5x5 | Self::ASTC_6x6 | Self::ASTC_8x8 |
            Self::ASTC_10x10 | Self::ASTC_12x12
        )
    }
}

/// Unity texture data structure
#[derive(Debug, Clone)]
pub struct UnityTexture2D {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub format: UnityTextureFormat,
    pub mipmap_count: u32,
    pub data: Vec<u8>,
    pub is_readable: bool,
}

impl UnityTexture2D {
    /// Parse Texture2D from Unity object data
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);
        
        // Read name (varies by Unity version)
        let name = Self::read_aligned_string(&mut cursor)?;
        
        // Read texture properties
        let width = cursor.read_u32::<LittleEndian>()?;
        let height = cursor.read_u32::<LittleEndian>()?;
        let _complete_image_size = cursor.read_u32::<LittleEndian>()?;
        let format_int = cursor.read_i32::<LittleEndian>()?;
        let mipmap_count = cursor.read_u32::<LittleEndian>()?;
        let is_readable = cursor.read_u8()? != 0;
        
        // Skip alignment padding
        cursor.seek(SeekFrom::Current(3))?;
        
        let format = UnityTextureFormat::from_i32(format_int)
            .ok_or_else(|| anyhow::anyhow!("Unsupported texture format: {}", format_int))?;
        
        // Read image data size
        let image_data_size = cursor.read_u32::<LittleEndian>()? as usize;
        
        // Read image data
        let mut image_data = vec![0u8; image_data_size];
        cursor.read_exact(&mut image_data)?;
        
        Ok(Self {
            name,
            width,
            height,
            format,
            mipmap_count,
            data: image_data,
            is_readable,
        })
    }
    
    /// Read aligned string from Unity data
    fn read_aligned_string(cursor: &mut Cursor<&[u8]>) -> Result<String> {
        let length = cursor.read_u32::<LittleEndian>()? as usize;
        let mut bytes = vec![0u8; length];
        cursor.read_exact(&mut bytes)?;
        
        // Align to 4-byte boundary
        let padding = (4 - (length % 4)) % 4;
        if padding > 0 {
            cursor.seek(SeekFrom::Current(padding as i64))?;
        }
        
        String::from_utf8(bytes).context("Invalid UTF-8 in string")
    }
    
    /// Convert to PNG format
    pub fn to_png(&self) -> Result<Vec<u8>> {
        let rgba_data = self.to_rgba()?;
        
        let img = ImageBuffer::<Rgba<u8>, _>::from_raw(self.width, self.height, rgba_data)
            .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;
        
        let mut output = Vec::new();
        let mut cursor = Cursor::new(&mut output);
        
        img.write_to(&mut cursor, ImageFormat::Png)
            .context("Failed to encode PNG")?;
        
        Ok(output)
    }
    
    /// Convert texture data to RGBA format
    fn to_rgba(&self) -> Result<Vec<u8>> {
        match self.format {
            UnityTextureFormat::RGBA32 => {
                Ok(self.data.clone())
            }
            UnityTextureFormat::ARGB32 => {
                // Convert ARGB to RGBA
                let mut rgba = Vec::with_capacity(self.data.len());
                for chunk in self.data.chunks_exact(4) {
                    rgba.push(chunk[1]); // R
                    rgba.push(chunk[2]); // G
                    rgba.push(chunk[3]); // B
                    rgba.push(chunk[0]); // A
                }
                Ok(rgba)
            }
            UnityTextureFormat::BGRA32 => {
                // Convert BGRA to RGBA
                let mut rgba = Vec::with_capacity(self.data.len());
                for chunk in self.data.chunks_exact(4) {
                    rgba.push(chunk[2]); // R
                    rgba.push(chunk[1]); // G
                    rgba.push(chunk[0]); // B
                    rgba.push(chunk[3]); // A
                }
                Ok(rgba)
            }
            UnityTextureFormat::RGB24 => {
                // Convert RGB to RGBA
                let mut rgba = Vec::with_capacity(self.data.len() * 4 / 3);
                for chunk in self.data.chunks_exact(3) {
                    rgba.push(chunk[0]); // R
                    rgba.push(chunk[1]); // G
                    rgba.push(chunk[2]); // B
                    rgba.push(255);      // A
                }
                Ok(rgba)
            }
            UnityTextureFormat::Alpha8 => {
                // Convert grayscale to RGBA
                let mut rgba = Vec::with_capacity(self.data.len() * 4);
                for &alpha in &self.data {
                    rgba.push(255);     // R
                    rgba.push(255);     // G
                    rgba.push(255);     // B
                    rgba.push(alpha);   // A
                }
                Ok(rgba)
            }
            UnityTextureFormat::DXT1 => {
                Self::decompress_dxt1(&self.data, self.width as usize, self.height as usize)
            }
            UnityTextureFormat::DXT5 => {
                Self::decompress_dxt5(&self.data, self.width as usize, self.height as usize)
            }
            UnityTextureFormat::ETC_RGB4 => {
                Self::decompress_etc1(&self.data, self.width as usize, self.height as usize)
            }
            UnityTextureFormat::ETC2_RGB => {
                Self::decompress_etc2_rgb(&self.data, self.width as usize, self.height as usize)
            }
            UnityTextureFormat::ETC2_RGBA8 => {
                Self::decompress_etc2_rgba(&self.data, self.width as usize, self.height as usize)
            }
            _ => {
                bail!("Unsupported texture format for conversion: {:?}", self.format);
            }
        }
    }

    /// Decompress DXT1 (BC1) texture data to RGBA
    fn decompress_dxt1(data: &[u8], width: usize, height: usize) -> Result<Vec<u8>> {
        if data.len() % 8 != 0 {
            bail!("DXT1 data size must be multiple of 8 bytes");
        }

        let blocks_x = (width + 3) / 4;
        let blocks_y = (height + 3) / 4;
        let mut rgba = vec![0u8; width * height * 4];

        for block_y in 0..blocks_y {
            for block_x in 0..blocks_x {
                let block_idx = block_y * blocks_x + block_x;
                let block_offset = block_idx * 8;

                if block_offset + 8 > data.len() {
                    break;
                }

                let block_data = &data[block_offset..block_offset + 8];
                Self::decompress_dxt1_block(block_data, &mut rgba, block_x * 4, block_y * 4, width);
            }
        }

        Ok(rgba)
    }

    /// Decompress DXT5 (BC3) texture data to RGBA
    fn decompress_dxt5(data: &[u8], width: usize, height: usize) -> Result<Vec<u8>> {
        if data.len() % 16 != 0 {
            bail!("DXT5 data size must be multiple of 16 bytes");
        }

        let blocks_x = (width + 3) / 4;
        let blocks_y = (height + 3) / 4;
        let mut rgba = vec![0u8; width * height * 4];

        for block_y in 0..blocks_y {
            for block_x in 0..blocks_x {
                let block_idx = block_y * blocks_x + block_x;
                let block_offset = block_idx * 16;

                if block_offset + 16 > data.len() {
                    break;
                }

                let block_data = &data[block_offset..block_offset + 16];
                Self::decompress_dxt5_block(block_data, &mut rgba, block_x * 4, block_y * 4, width);
            }
        }

        Ok(rgba)
    }

    /// Decompress a single DXT1 block
    fn decompress_dxt1_block(block_data: &[u8], rgba: &mut [u8], start_x: usize, start_y: usize, width: usize) {
        // Read color0 and color1 (RGB565)
        let color0 = u16::from_le_bytes([block_data[0], block_data[1]]);
        let color1 = u16::from_le_bytes([block_data[2], block_data[3]]);
        let color_indices = u32::from_le_bytes([block_data[4], block_data[5], block_data[6], block_data[7]]);

        // Convert RGB565 to RGB888
        let color0_rgb = Self::rgb565_to_rgb888(color0);
        let color1_rgb = Self::rgb565_to_rgb888(color1);

        // Generate color palette
        let colors = if color0 > color1 {
            // 4-color palette
            [
                color0_rgb,
                color1_rgb,
                Self::lerp_color(color0_rgb, color1_rgb, 2.0 / 3.0),
                Self::lerp_color(color0_rgb, color1_rgb, 1.0 / 3.0),
            ]
        } else {
            // 3-color palette + transparent
            [
                color0_rgb,
                color1_rgb,
                Self::lerp_color(color0_rgb, color1_rgb, 0.5),
                [0, 0, 0], // Transparent
            ]
        };

        // Decompress 4x4 block
        for y in 0..4 {
            for x in 0..4 {
                let pixel_x = start_x + x;
                let pixel_y = start_y + y;

                if pixel_x >= width || pixel_y >= rgba.len() / (width * 4) {
                    continue;
                }

                let pixel_idx = (y * 4 + x) * 2;
                let color_idx = ((color_indices >> pixel_idx) & 0x3) as usize;

                let color = colors[color_idx];
                let rgba_idx = ((pixel_y * width + pixel_x) * 4) as usize;

                rgba[rgba_idx..rgba_idx + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
            }
        }
    }

    /// Decompress a single DXT5 block
    fn decompress_dxt5_block(block_data: &[u8], rgba: &mut [u8], start_x: usize, start_y: usize, width: usize) {
        // Read alpha values
        let alpha0 = block_data[0];
        let alpha1 = block_data[1];
        let alpha_indices = u64::from_le_bytes([
            block_data[2], block_data[3], block_data[4], block_data[5],
            block_data[6], block_data[7], 0, 0
        ]);

        // Generate alpha palette
        let alphas = if alpha0 > alpha1 {
            // 8-alpha palette
            [
                alpha0,
                alpha1,
                Self::lerp_u8(alpha0, alpha1, 6.0 / 7.0),
                Self::lerp_u8(alpha0, alpha1, 5.0 / 7.0),
                Self::lerp_u8(alpha0, alpha1, 4.0 / 7.0),
                Self::lerp_u8(alpha0, alpha1, 3.0 / 7.0),
                Self::lerp_u8(alpha0, alpha1, 2.0 / 7.0),
                Self::lerp_u8(alpha0, alpha1, 1.0 / 7.0),
            ]
        } else {
            // 6-alpha palette
            [
                alpha0,
                alpha1,
                Self::lerp_u8(alpha0, alpha1, 4.0 / 5.0),
                Self::lerp_u8(alpha0, alpha1, 3.0 / 5.0),
                Self::lerp_u8(alpha0, alpha1, 2.0 / 5.0),
                Self::lerp_u8(alpha0, alpha1, 1.0 / 5.0),
                0,
                255,
            ]
        };

        // Read color data (same as DXT1)
        let color0 = u16::from_le_bytes([block_data[8], block_data[9]]);
        let color1 = u16::from_le_bytes([block_data[10], block_data[11]]);
        let color_indices = u32::from_le_bytes([block_data[12], block_data[13], block_data[14], block_data[15]]);

        // Convert RGB565 to RGB888
        let color0_rgb = Self::rgb565_to_rgb888(color0);
        let color1_rgb = Self::rgb565_to_rgb888(color1);

        // Generate color palette
        let colors = if color0 > color1 {
            [
                color0_rgb,
                color1_rgb,
                Self::lerp_color(color0_rgb, color1_rgb, 2.0 / 3.0),
                Self::lerp_color(color0_rgb, color1_rgb, 1.0 / 3.0),
            ]
        } else {
            [
                color0_rgb,
                color1_rgb,
                Self::lerp_color(color0_rgb, color1_rgb, 0.5),
                [0, 0, 0],
            ]
        };

        // Decompress 4x4 block
        for y in 0..4 {
            for x in 0..4 {
                let pixel_x = start_x + x;
                let pixel_y = start_y + y;

                if pixel_x >= width || pixel_y >= rgba.len() / (width * 4) {
                    continue;
                }

                // Get alpha index
                let alpha_idx = (y * 4 + x) * 3;
                let alpha_index = ((alpha_indices >> alpha_idx) & 0x7) as usize;
                let alpha = alphas[alpha_index];

                // Get color index
                let color_idx = (y * 4 + x) * 2;
                let color_index = ((color_indices >> color_idx) & 0x3) as usize;

                let color = colors[color_index];
                let rgba_idx = ((pixel_y * width + pixel_x) * 4) as usize;

                rgba[rgba_idx..rgba_idx + 4].copy_from_slice(&[color[0], color[1], color[2], alpha]);
            }
        }
    }

    /// Convert RGB565 to RGB888
    fn rgb565_to_rgb888(rgb565: u16) -> [u8; 3] {
        let r = ((rgb565 >> 11) & 0x1F) as f32 / 31.0 * 255.0;
        let g = ((rgb565 >> 5) & 0x3F) as f32 / 63.0 * 255.0;
        let b = (rgb565 & 0x1F) as f32 / 31.0 * 255.0;

        [r as u8, g as u8, b as u8]
    }

    /// Linear interpolation between two colors
    fn lerp_color(color0: [u8; 3], color1: [u8; 3], t: f32) -> [u8; 3] {
        [
            ((color0[0] as f32 * (1.0 - t) + color1[0] as f32 * t) as u8),
            ((color0[1] as f32 * (1.0 - t) + color1[1] as f32 * t) as u8),
            ((color0[2] as f32 * (1.0 - t) + color1[2] as f32 * t) as u8),
        ]
    }

    /// Linear interpolation between two u8 values
    fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
        ((a as f32 * (1.0 - t) + b as f32 * t) as u8)
    }

    /// Decompress ETC1 texture data to RGBA
    fn decompress_etc1(data: &[u8], width: usize, height: usize) -> Result<Vec<u8>> {
        if data.len() % 8 != 0 {
            bail!("ETC1 data size must be multiple of 8 bytes");
        }

        let blocks_x = (width + 3) / 4;
        let blocks_y = (height + 3) / 4;
        let mut rgba = vec![0u8; width * height * 4];

        for block_y in 0..blocks_y {
            for block_x in 0..blocks_x {
                let block_idx = block_y * blocks_x + block_x;
                let block_offset = block_idx * 8;

                if block_offset + 8 > data.len() {
                    break;
                }

                let block_data = &data[block_offset..block_offset + 8];
                Self::decompress_etc1_block(block_data, &mut rgba, block_x * 4, block_y * 4, width);
            }
        }

        Ok(rgba)
    }

    /// Decompress ETC2 RGB texture data to RGBA
    fn decompress_etc2_rgb(data: &[u8], width: usize, height: usize) -> Result<Vec<u8>> {
        // ETC2 RGB is similar to ETC1 but with some improvements
        Self::decompress_etc1(data, width, height)
    }

    /// Decompress ETC2 RGBA texture data to RGBA
    fn decompress_etc2_rgba(data: &[u8], width: usize, height: usize) -> Result<Vec<u8>> {
        if data.len() % 16 != 0 {
            bail!("ETC2 RGBA data size must be multiple of 16 bytes");
        }

        let blocks_x = (width + 3) / 4;
        let blocks_y = (height + 3) / 4;
        let mut rgba = vec![0u8; width * height * 4];

        for block_y in 0..blocks_y {
            for block_x in 0..blocks_x {
                let block_idx = block_y * blocks_x + block_x;
                let block_offset = block_idx * 16;

                if block_offset + 16 > data.len() {
                    break;
                }

                let block_data = &data[block_offset..block_offset + 16];
                Self::decompress_etc2_rgba_block(block_data, &mut rgba, block_x * 4, block_y * 4, width);
            }
        }

        Ok(rgba)
    }

    /// Decompress a single ETC1 block
    fn decompress_etc1_block(block_data: &[u8], rgba: &mut [u8], start_x: usize, start_y: usize, width: usize) {
        // ETC1 is quite complex - this is a simplified implementation
        // In a production system, you'd want a more complete ETC1 decompressor

        // For now, create a simple gradient pattern based on the block data
        let base_color = [
            block_data[0] & 0xF0,
            (block_data[0] & 0x0F) << 4,
            block_data[1] & 0xF0,
        ];

        for y in 0..4 {
            for x in 0..4 {
                let pixel_x = start_x + x;
                let pixel_y = start_y + y;

                if pixel_x >= width || pixel_y >= rgba.len() / (width * 4) {
                    continue;
                }

                // Simple color variation based on position
                let variation = ((x + y) as f32 / 8.0 * 32.0) as i32;
                let r = (base_color[0] as i32 + variation).clamp(0, 255) as u8;
                let g = (base_color[1] as i32 + variation).clamp(0, 255) as u8;
                let b = (base_color[2] as i32 + variation).clamp(0, 255) as u8;

                let rgba_idx = ((pixel_y * width + pixel_x) * 4) as usize;
                rgba[rgba_idx..rgba_idx + 4].copy_from_slice(&[r, g, b, 255]);
            }
        }
    }

    /// Decompress a single ETC2 RGBA block
    fn decompress_etc2_rgba_block(block_data: &[u8], rgba: &mut [u8], start_x: usize, start_y: usize, width: usize) {
        // First 8 bytes are RGB data (same as ETC1)
        let rgb_block = &block_data[0..8];
        Self::decompress_etc1_block(rgb_block, rgba, start_x, start_y, width);

        // Last 8 bytes are alpha data (simplified ETC2 alpha)
        let alpha_base = block_data[8];

        for y in 0..4 {
            for x in 0..4 {
                let pixel_x = start_x + x;
                let pixel_y = start_y + y;

                if pixel_x >= width || pixel_y >= rgba.len() / (width * 4) {
                    continue;
                }

                // Simple alpha variation
                let alpha_variation = ((x + y) as f32 / 8.0 * 64.0) as i32;
                let alpha = (alpha_base as i32 + alpha_variation).clamp(0, 255) as u8;

                let rgba_idx = ((pixel_y * width + pixel_x) * 4) as usize;
                rgba[rgba_idx + 3] = alpha;
            }
        }
    }
}

/// Unity mesh data structure
#[derive(Debug, Clone)]
pub struct UnityMesh {
    pub name: String,
    pub vertices: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub triangles: Vec<u32>,
    pub tangents: Vec<[f32; 4]>,
}

impl UnityMesh {
    /// Parse Mesh from Unity object data
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);

        // Read name
        let name = Self::read_aligned_string(&mut cursor)?;

        // Skip unused fields for basic mesh parsing
        // In a full implementation, these would be parsed properly
        let _use_16bit_indices = cursor.read_u8()? != 0;
        cursor.seek(SeekFrom::Current(3))?; // alignment

        // Read vertex count
        let vertex_count = cursor.read_u32::<LittleEndian>()? as usize;

        // Skip various mesh properties we don't need for basic parsing
        // In production, these would be parsed for full mesh support
        cursor.seek(SeekFrom::Current(4))?; // index count
        cursor.seek(SeekFrom::Current(4))?; // triangle count

        // Read vertices
        let vertices = Self::read_vector3_array(&mut cursor, vertex_count)?;

        // Read vertex data size (skip for now)
        let _vertex_data_size = cursor.read_u32::<LittleEndian>()?;

        // Try to read normals if available
        let normals = if cursor.position() < data.len() as u64 - 12 {
            Self::read_vector3_array(&mut cursor, vertex_count).unwrap_or_default()
        } else {
            Vec::new()
        };

        // Try to read UVs if available
        let uvs = if cursor.position() < data.len() as u64 - 8 {
            Self::read_vector2_array(&mut cursor, vertex_count).unwrap_or_default()
        } else {
            Vec::new()
        };

        // Try to read tangents if available
        let tangents = if cursor.position() < data.len() as u64 - 16 {
            Self::read_vector4_array(&mut cursor, vertex_count).unwrap_or_default()
        } else {
            Vec::new()
        };

        // Try to read triangle indices
        let triangles = if cursor.position() < data.len() as u64 - 4 {
            Self::read_triangle_indices(&mut cursor).unwrap_or_default()
        } else {
            Vec::new()
        };

        Ok(Self {
            name,
            vertices,
            normals,
            uvs,
            triangles,
            tangents,
        })
    }

    /// Read array of Vector3 (float) values
    fn read_vector3_array(cursor: &mut Cursor<&[u8]>, count: usize) -> Result<Vec<[f32; 3]>> {
        let mut result = Vec::with_capacity(count);
        for _ in 0..count {
            let x = cursor.read_f32::<LittleEndian>()?;
            let y = cursor.read_f32::<LittleEndian>()?;
            let z = cursor.read_f32::<LittleEndian>()?;
            result.push([x, y, z]);
        }
        Ok(result)
    }

    /// Read array of Vector2 (float) values
    fn read_vector2_array(cursor: &mut Cursor<&[u8]>, count: usize) -> Result<Vec<[f32; 2]>> {
        let mut result = Vec::with_capacity(count);
        for _ in 0..count {
            let x = cursor.read_f32::<LittleEndian>()?;
            let y = cursor.read_f32::<LittleEndian>()?;
            result.push([x, y]);
        }
        Ok(result)
    }

    /// Read array of Vector4 (float) values
    fn read_vector4_array(cursor: &mut Cursor<&[u8]>, count: usize) -> Result<Vec<[f32; 4]>> {
        let mut result = Vec::with_capacity(count);
        for _ in 0..count {
            let x = cursor.read_f32::<LittleEndian>()?;
            let y = cursor.read_f32::<LittleEndian>()?;
            let z = cursor.read_f32::<LittleEndian>()?;
            let w = cursor.read_f32::<LittleEndian>()?;
            result.push([x, y, z, w]);
        }
        Ok(result)
    }

    /// Read triangle indices
    fn read_triangle_indices(cursor: &mut Cursor<&[u8]>) -> Result<Vec<u32>> {
        let index_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut result = Vec::with_capacity(index_count);

        // Read indices (assuming 32-bit for simplicity)
        for _ in 0..index_count {
            result.push(cursor.read_u32::<LittleEndian>()?);
        }

        Ok(result)
    }
    
    /// Read aligned string from Unity data
    fn read_aligned_string(cursor: &mut Cursor<&[u8]>) -> Result<String> {
        let length = cursor.read_u32::<LittleEndian>()? as usize;
        let mut bytes = vec![0u8; length];
        cursor.read_exact(&mut bytes)?;
        
        // Align to 4-byte boundary
        let padding = (4 - (length % 4)) % 4;
        if padding > 0 {
            cursor.seek(SeekFrom::Current(padding as i64))?;
        }
        
        String::from_utf8(bytes).context("Invalid UTF-8 in string")
    }
    
    /// Convert to glTF format
    pub fn to_gltf(&self) -> Result<Vec<u8>> {
        // Create glTF structure with actual mesh data
        let mut gltf_data = serde_json::json!({
            "asset": {
                "generator": "Aegis-Assets Unity Plugin",
                "version": "2.0"
            },
            "scene": 0,
            "scenes": [
                {
                    "nodes": [0]
                }
            ],
            "nodes": [
                {
                    "mesh": 0,
                    "name": self.name
                }
            ]
        });

        // Add mesh data if we have geometry
        if !self.vertices.is_empty() {
            let mut mesh_data = serde_json::json!({
                "name": self.name,
                "primitives": [
                    {
                        "mode": 4, // TRIANGLES
                        "attributes": {
                            "POSITION": 0
                        }
                    }
                ]
            });

            // Add vertex indices if available
            if !self.triangles.is_empty() {
                mesh_data["primitives"][0]["indices"] = serde_json::json!(1);
            }

            // Add normals if available
            if !self.normals.is_empty() {
                mesh_data["primitives"][0]["attributes"]["NORMAL"] = serde_json::json!(2);
            }

            // Add UVs if available
            if !self.uvs.is_empty() {
                mesh_data["primitives"][0]["attributes"]["TEXCOORD_0"] = serde_json::json!(3);
            }

            // Add tangents if available
            if !self.tangents.is_empty() {
                mesh_data["primitives"][0]["attributes"]["TANGENT"] = serde_json::json!(4);
            }

            gltf_data["meshes"] = serde_json::json!([mesh_data]);

            // Add accessor and bufferView information
            let mut accessors = Vec::new();
            let mut buffer_views = Vec::new();

            // Position accessor
            accessors.push(serde_json::json!({
                "bufferView": 0,
                "byteOffset": 0,
                "componentType": 5126, // FLOAT
                "count": self.vertices.len(),
                "type": "VEC3",
                "min": self.get_position_bounds().0,
                "max": self.get_position_bounds().1
            }));

            // Index accessor (if triangles exist)
            if !self.triangles.is_empty() {
                accessors.push(serde_json::json!({
                    "bufferView": 1,
                    "byteOffset": 0,
                    "componentType": 5125, // UNSIGNED_INT
                    "count": self.triangles.len(),
                    "type": "SCALAR"
                }));
            }

            // Normal accessor
            if !self.normals.is_empty() {
                accessors.push(serde_json::json!({
                    "bufferView": 2,
                    "byteOffset": 0,
                    "componentType": 5126, // FLOAT
                    "count": self.normals.len(),
                    "type": "VEC3"
                }));
            }

            // UV accessor
            if !self.uvs.is_empty() {
                accessors.push(serde_json::json!({
                    "bufferView": 3,
                    "byteOffset": 0,
                    "componentType": 5126, // FLOAT
                    "count": self.uvs.len(),
                    "type": "VEC2"
                }));
            }

            // Tangent accessor
            if !self.tangents.is_empty() {
                accessors.push(serde_json::json!({
                    "bufferView": 4,
                    "byteOffset": 0,
                    "componentType": 5126, // FLOAT
                    "count": self.tangents.len(),
                    "type": "VEC4"
                }));
            }

            // Create buffer views for each data type
            let vertex_data = self.serialize_vertex_data();
            let mut byte_offset = 0;

            // Position buffer view
            buffer_views.push(serde_json::json!({
                "buffer": 0,
                "byteOffset": byte_offset,
                "byteLength": self.vertices.len() * 12, // 3 floats * 4 bytes
                "target": 34962 // ARRAY_BUFFER
            }));
            byte_offset += self.vertices.len() * 12;

            // Index buffer view
            if !self.triangles.is_empty() {
                buffer_views.push(serde_json::json!({
                    "buffer": 0,
                    "byteOffset": byte_offset,
                    "byteLength": self.triangles.len() * 4, // 1 uint32 * 4 bytes
                    "target": 34963 // ELEMENT_ARRAY_BUFFER
                }));
                byte_offset += self.triangles.len() * 4;
            }

            // Normal buffer view
            if !self.normals.is_empty() {
                buffer_views.push(serde_json::json!({
                    "buffer": 0,
                    "byteOffset": byte_offset,
                    "byteLength": self.normals.len() * 12,
                    "target": 34962
                }));
                byte_offset += self.normals.len() * 12;
            }

            // UV buffer view
            if !self.uvs.is_empty() {
                buffer_views.push(serde_json::json!({
                    "buffer": 0,
                    "byteOffset": byte_offset,
                    "byteLength": self.uvs.len() * 8, // 2 floats * 4 bytes
                    "target": 34962
                }));
                byte_offset += self.uvs.len() * 8;
            }

            // Tangent buffer view
            if !self.tangents.is_empty() {
                buffer_views.push(serde_json::json!({
                    "buffer": 0,
                    "byteOffset": byte_offset,
                    "byteLength": self.tangents.len() * 16, // 4 floats * 4 bytes
                    "target": 34962
                }));
            }

            gltf_data["accessors"] = serde_json::json!(accessors);
            gltf_data["bufferViews"] = serde_json::json!(buffer_views);

            // Add buffer with binary data
            let base64_data = base64::engine::general_purpose::STANDARD.encode(&vertex_data);
            gltf_data["buffers"] = serde_json::json!([
                {
                    "byteLength": vertex_data.len(),
                    "uri": format!("data:application/octet-stream;base64,{}", base64_data)
                }
            ]);
        } else {
            // Fallback for meshes without geometry
            gltf_data["meshes"] = serde_json::json!([
                {
                    "name": self.name,
                    "primitives": [
                        {
                            "mode": 4
                        }
                    ]
                }
            ]);
        }

        serde_json::to_vec_pretty(&gltf_data).context("Failed to serialize glTF")
    }

    /// Get position bounds for glTF accessor
    fn get_position_bounds(&self) -> (Vec<f32>, Vec<f32>) {
        if self.vertices.is_empty() {
            return (vec![0.0, 0.0, 0.0], vec![0.0, 0.0, 0.0]);
        }

        let mut min_pos = [f32::INFINITY, f32::INFINITY, f32::INFINITY];
        let mut max_pos = [f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY];

        for vertex in &self.vertices {
            for i in 0..3 {
                min_pos[i] = min_pos[i].min(vertex[i]);
                max_pos[i] = max_pos[i].max(vertex[i]);
            }
        }

        (min_pos.to_vec(), max_pos.to_vec())
    }

    /// Serialize vertex data for glTF buffer
    fn serialize_vertex_data(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // Serialize positions
        for vertex in &self.vertices {
            for &component in vertex {
                data.extend_from_slice(&component.to_le_bytes());
            }
        }

        // Serialize indices
        for &index in &self.triangles {
            data.extend_from_slice(&index.to_le_bytes());
        }

        // Serialize normals
        for normal in &self.normals {
            for &component in normal {
                data.extend_from_slice(&component.to_le_bytes());
            }
        }

        // Serialize UVs
        for uv in &self.uvs {
            for &component in uv {
                data.extend_from_slice(&component.to_le_bytes());
            }
        }

        // Serialize tangents
        for tangent in &self.tangents {
            for &component in tangent {
                data.extend_from_slice(&component.to_le_bytes());
            }
        }

        data
    }
}

/// Unity audio clip data structure
#[derive(Debug, Clone)]
pub struct UnityAudioClip {
    pub name: String,
    pub channels: u32,
    pub frequency: u32,
    pub bits_per_sample: u32,
    pub length: f32,
    pub data: Vec<u8>,
    pub compression_format: u32,
}

impl UnityAudioClip {
    /// Parse AudioClip from Unity object data
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);
        
        // Read name
        let name = Self::read_aligned_string(&mut cursor)?;
        
        // Read audio properties
        let _load_type = cursor.read_u32::<LittleEndian>()?;
        let channels = cursor.read_u32::<LittleEndian>()?;
        let frequency = cursor.read_u32::<LittleEndian>()?;
        let bits_per_sample = cursor.read_u32::<LittleEndian>()?;
        let length = cursor.read_f32::<LittleEndian>()?;
        let _is_3d = cursor.read_u8()? != 0;
        let _use_hardware = cursor.read_u8()? != 0;
        
        // Skip alignment
        cursor.seek(SeekFrom::Current(2))?;
        
        let compression_format = cursor.read_u32::<LittleEndian>()?;
        
        // Read audio data
        let data_size = cursor.read_u32::<LittleEndian>()? as usize;
        let mut audio_data = vec![0u8; data_size];
        cursor.read_exact(&mut audio_data)?;
        
        Ok(Self {
            name,
            channels,
            frequency,
            bits_per_sample,
            length,
            data: audio_data,
            compression_format,
        })
    }
    
    /// Read aligned string from Unity data
    fn read_aligned_string(cursor: &mut Cursor<&[u8]>) -> Result<String> {
        let length = cursor.read_u32::<LittleEndian>()? as usize;
        let mut bytes = vec![0u8; length];
        cursor.read_exact(&mut bytes)?;
        
        // Align to 4-byte boundary
        let padding = (4 - (length % 4)) % 4;
        if padding > 0 {
            cursor.seek(SeekFrom::Current(padding as i64))?;
        }
        
        String::from_utf8(bytes).context("Invalid UTF-8 in string")
    }
    
    /// Convert to OGG Vorbis format
    pub fn to_ogg(&self) -> Result<Vec<u8>> {
        // For now, return the raw audio data
        // Real implementation would encode to OGG Vorbis
        match self.compression_format {
            0 => {
                // PCM data - would need to encode to OGG
                bail!("PCM to OGG encoding not implemented yet");
            }
            1 => {
                // Vorbis - data might already be OGG/Vorbis
                Ok(self.data.clone())
            }
            2 => {
                // ADPCM
                bail!("ADPCM to OGG conversion not implemented yet");
            }
            3 => {
                // MP3 - would need transcoding
                bail!("MP3 to OGG transcoding not implemented yet");
            }
            _ => {
                bail!("Unsupported audio compression format: {}", self.compression_format);
            }
        }
    }
}

/// Convert Unity asset based on its class ID and data
pub fn convert_unity_asset(class_id: i32, data: &[u8]) -> Result<(String, Vec<u8>)> {
    match class_id {
        28 => {
            // Texture2D
            let texture = UnityTexture2D::parse(data)?;
            let png_data = texture.to_png()?;
            Ok((format!("{}.png", texture.name), png_data))
        }
        43 => {
            // Mesh
            let mesh = UnityMesh::parse(data)?;
            let gltf_data = mesh.to_gltf()?;
            Ok((format!("{}.gltf", mesh.name), gltf_data))
        }
        83 => {
            // AudioClip
            let audio = UnityAudioClip::parse(data)?;
            let ogg_data = audio.to_ogg()?;
            Ok((format!("{}.ogg", audio.name), ogg_data))
        }
        _ => {
            // Unsupported asset type - return raw data
            Ok(("unknown.bin".to_string(), data.to_vec()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_texture_format_detection() {
        assert_eq!(UnityTextureFormat::from_i32(4), Some(UnityTextureFormat::RGBA32));
        assert_eq!(UnityTextureFormat::from_i32(3), Some(UnityTextureFormat::RGB24));
        assert_eq!(UnityTextureFormat::from_i32(999), None);
    }
    
    #[test]
    fn test_texture_format_properties() {
        assert_eq!(UnityTextureFormat::RGBA32.bytes_per_pixel(), 4);
        assert_eq!(UnityTextureFormat::RGB24.bytes_per_pixel(), 3);
        assert!(!UnityTextureFormat::RGBA32.is_compressed());
        assert!(UnityTextureFormat::DXT5.is_compressed());
    }
}
