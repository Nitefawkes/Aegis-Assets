use anyhow::{Result, Context, bail};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use std::collections::HashMap;
use std::io::{Cursor, Read, Seek, SeekFrom};
use crate::compression::decompress_unity_data;

/// Unity engine version information
#[derive(Debug, Clone)]
pub struct UnityVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub build: String,
}

impl UnityVersion {
    /// Parse version string like "2022.3.15f1"
    pub fn parse(version_str: &str) -> Result<Self> {
        let parts: Vec<&str> = version_str.split('.').collect();
        if parts.len() < 3 {
            bail!("Invalid version format: {}", version_str);
        }
        
        let major = parts[0].parse().context("Invalid major version")?;
        let minor = parts[1].parse().context("Invalid minor version")?;
        
        // Parse patch and build (e.g., "15f1")
        let patch_build = parts[2];
        let patch_end = patch_build.find(char::is_alphabetic).unwrap_or(patch_build.len());
        let patch = patch_build[..patch_end].parse().context("Invalid patch version")?;
        let build = patch_build[patch_end..].to_string();
        
        Ok(UnityVersion {
            major,
            minor,
            patch,
            build,
        })
    }
    
    /// Check if this version supports a specific feature
    pub fn supports_unity_fs(&self) -> bool {
        // UnityFS format was introduced in Unity 5.3
        self.major > 5 || (self.major == 5 && self.minor >= 3)
    }
    
    /// Check if this version uses the new serialization format
    pub fn uses_new_serialization(&self) -> bool {
        self.major >= 2017
    }
}

/// Unity asset bundle structure (UnityFS format)
#[derive(Debug, Clone)]
pub struct AssetBundle {
    pub signature: String,
    pub unityfs_version: u32,
    pub version: u32,
    pub unity_version: String,
    pub unity_revision: String,
    pub size: u64,
    pub compressed_blocks_info_size: u32,
    pub uncompressed_blocks_info_size: u32,
    pub flags: u32,
    pub blocks_info: Vec<BlockInfo>,
    pub directory_info: Vec<DirectoryInfo>,
    pub header_size: u64,
}

/// Information about a compressed block in the bundle
#[derive(Debug, Clone)]
pub struct BlockInfo {
    pub uncompressed_size: u32,
    pub compressed_size: u32,
    pub flags: u16,
}

/// Information about a file/directory in the bundle
#[derive(Debug, Clone)]
pub struct DirectoryInfo {
    pub offset: u64,
    pub size: u64,
    pub flags: u32,
    pub name: String,
    pub compressed_size: u64,
    pub compression_type: u32,
}

impl AssetBundle {
    /// Parse UnityFS bundle format
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 64 {
            bail!("File too small to be a valid UnityFS bundle (need at least 64 bytes, got {})", data.len());
        }
        
        let mut cursor = Cursor::new(data);
        
        // Read signature (8 bytes)
        let mut signature_bytes = [0u8; 8];
        cursor.read_exact(&mut signature_bytes)?;
        let signature = String::from_utf8_lossy(&signature_bytes).trim_end_matches('\0').to_string();
        
        if !signature.starts_with("UnityFS") {
            bail!("Invalid UnityFS signature: {}", signature);
        }

        // Extract UnityFS version (UnityFS = v6, UnityFS2 = v7+, etc.)
        let unityfs_version = if signature == "UnityFS" {
            6
        } else if signature == "UnityFS2" {
            7
        } else {
            // Extract version number from signature like "UnityFS5"
            signature.strip_prefix("UnityFS")
                .and_then(|v| v.parse().ok())
                .unwrap_or(6) // Default to v6 if parsing fails
        };
        
        // Read format version (4 bytes, big endian)
        let version = cursor.read_u32::<BigEndian>()?;
        
        // Read Unity version string (null-terminated)
        let unity_version = Self::read_null_terminated_string(&mut cursor)?;
        
        // Read Unity revision string (null-terminated)
        let unity_revision = Self::read_null_terminated_string(&mut cursor)?;
        
        // Read bundle size (8 bytes, big endian)
        let size = cursor.read_u64::<BigEndian>()?;
        
        // Read compressed blocks info size (4 bytes, big endian)
        let compressed_blocks_info_size = cursor.read_u32::<BigEndian>()?;
        
        // Read uncompressed blocks info size (4 bytes, big endian)
        let uncompressed_blocks_info_size = cursor.read_u32::<BigEndian>()?;
        
        // Read flags (4 bytes, big endian)
        let flags = cursor.read_u32::<BigEndian>()?;
        
        // Store header size for later calculations
        let header_size = cursor.position();
        
        // Read blocks info
        let blocks_info = if compressed_blocks_info_size > 0 {
            Self::parse_blocks_info(&mut cursor, compressed_blocks_info_size, uncompressed_blocks_info_size, flags)?
        } else {
            Vec::new()
        };
        
        // Read directory info
        let directory_info = Self::parse_directory_info(&mut cursor, &blocks_info, flags)?;
        
        Ok(AssetBundle {
            signature,
            unityfs_version,
            version,
            unity_version,
            unity_revision,
            size,
            compressed_blocks_info_size,
            uncompressed_blocks_info_size,
            flags,
            blocks_info,
            directory_info,
            header_size,
        })
    }
    
    /// Read null-terminated string from cursor
    fn read_null_terminated_string(cursor: &mut Cursor<&[u8]>) -> Result<String> {
        let mut bytes = Vec::new();
        
        loop {
            match cursor.read_u8() {
                Ok(byte) => {
                    if byte == 0 {
                        break;
                    }
                    bytes.push(byte);
                }
                Err(_) => break, // End of file
            }
        }
        
        String::from_utf8(bytes).context("Invalid UTF-8 in string")
    }
    
    /// Parse block information
    fn parse_blocks_info(
        cursor: &mut Cursor<&[u8]>,
        compressed_size: u32,
        uncompressed_size: u32,
        flags: u32,
    ) -> Result<Vec<BlockInfo>> {
        let compression_type = (flags >> 6) & 0x3F; // Extract compression type from flags
        
        // Read compressed blocks info data
        let mut compressed_data = vec![0u8; compressed_size as usize];
        cursor.read_exact(&mut compressed_data)?;
        
        // Decompress if needed
        let decompressed_data = if compression_type > 0 {
            decompress_unity_data(&compressed_data, compression_type, uncompressed_size as usize)?
        } else {
            compressed_data
        };
        
        // Parse block info from decompressed data
        let mut blocks_cursor = Cursor::new(&decompressed_data[..]);
        let mut blocks = Vec::new();
        
        // Read number of blocks (assumed first 4 bytes, but may vary by version)
        if decompressed_data.len() >= 16 {
            // Simple parsing for now - real implementation would handle version differences
            let uncompressed_size = blocks_cursor.read_u32::<BigEndian>()?;
            let compressed_size = blocks_cursor.read_u32::<BigEndian>()?;
            let flags = blocks_cursor.read_u16::<BigEndian>()?;
            
            blocks.push(BlockInfo {
                uncompressed_size,
                compressed_size,
                flags,
            });
        }
        
        Ok(blocks)
    }
    
    /// Parse directory information
    fn parse_directory_info(
        cursor: &mut Cursor<&[u8]>,
        blocks_info: &[BlockInfo],
        flags: u32,
    ) -> Result<Vec<DirectoryInfo>> {
        let mut directory_info = Vec::new();
        
        // For now, create a basic directory entry for the main asset file
        // Real implementation would parse the actual directory structure
        if !blocks_info.is_empty() {
            directory_info.push(DirectoryInfo {
                offset: 0,
                size: blocks_info[0].uncompressed_size as u64,
                flags: 0,
                name: "CAB-archive".to_string(), // Common Unity archive name
                compressed_size: blocks_info[0].compressed_size as u64,
                compression_type: (flags >> 6) & 0x3F,
            });
        }
        
        Ok(directory_info)
    }
    
    /// Get data for a specific directory entry
    pub fn get_directory_data(&self, entry: &DirectoryInfo, source_data: &[u8]) -> Result<Vec<u8>> {
        let start_offset = self.header_size + entry.offset;
        let end_offset = start_offset + entry.compressed_size;
        
        if end_offset > source_data.len() as u64 {
            bail!("Directory entry extends beyond file boundaries");
        }
        
        let compressed_data = &source_data[start_offset as usize..end_offset as usize];
        
        if entry.compression_type > 0 {
            decompress_unity_data(compressed_data, entry.compression_type, entry.size as usize)
        } else {
            Ok(compressed_data.to_vec())
        }
    }
}

/// Unity serialized file format (for .assets files)
#[derive(Debug, Clone)]
pub struct SerializedFile {
    pub metadata_size: u32,
    pub file_size: u32,
    pub version: u32,
    pub data_offset: u32,
    pub endianness: bool,
    pub unity_version: String,
    pub target_platform: u32,
    pub type_tree: HashMap<i32, TypeInfo>,
    pub objects: Vec<ObjectInfo>,
    pub script_types: Vec<LocalSerializedObjectIdentifier>,
    pub externals: Vec<FileIdentifier>,
}

/// Type information for Unity objects
#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub class_id: i32,
    pub is_stripped_type: bool,
    pub script_type_index: i16,
    pub type_name: String,
    pub type_dependencies: Vec<i32>,
}

/// Information about a Unity object
#[derive(Debug, Clone)]
pub struct ObjectInfo {
    pub path_id: u64,
    pub offset: u64,
    pub size: u32,
    pub class_id: i32,
}

/// Local serialized object identifier
#[derive(Debug, Clone)]
pub struct LocalSerializedObjectIdentifier {
    pub local_serialized_file_index: i32,
    pub local_identifier_in_file: u64,
}

/// File identifier for external references
#[derive(Debug, Clone)]
pub struct FileIdentifier {
    pub temp_empty: String,
    pub guid: [u8; 16],
    pub type_: i32,
    pub path_name: String,
}

impl SerializedFile {
    /// Parse Unity serialized file format
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);
        
        // Read header
        let metadata_size = cursor.read_u32::<LittleEndian>()?;
        let file_size = cursor.read_u32::<LittleEndian>()?;
        let version = cursor.read_u32::<LittleEndian>()?;
        let data_offset = cursor.read_u32::<LittleEndian>()?;
        
        if version >= 9 {
            // Read endianness flag
            let endianness_byte = cursor.read_u8()?;
            let endianness = endianness_byte != 0;
            
            // Skip reserved bytes
            cursor.seek(SeekFrom::Current(3))?;
        } else {
            // Older versions don't have endianness flag
            let endianness = false;
        }
        
        // Read Unity version (format varies by file version)
        let unity_version = if version >= 9 {
            Self::read_string(&mut cursor)?
        } else {
            "Unknown".to_string()
        };
        
        // Read target platform
        let target_platform = cursor.read_u32::<LittleEndian>()?;
        
        // Read type tree
        let type_tree = Self::parse_type_tree(&mut cursor, version)?;
        
        // Read objects
        let objects = Self::parse_objects(&mut cursor, version)?;
        
        // Read script types (if present)
        let script_types = if version >= 7 {
            Self::parse_script_types(&mut cursor)?
        } else {
            Vec::new()
        };
        
        // Read externals (if present) 
        let externals = if version >= 6 {
            Self::parse_externals(&mut cursor)?
        } else {
            Vec::new()
        };
        
        Ok(SerializedFile {
            metadata_size,
            file_size,
            version,
            data_offset,
            endianness: false, // Set based on actual parsing
            unity_version,
            target_platform,
            type_tree,
            objects,
            script_types,
            externals,
        })
    }
    
    /// Read string from cursor (Unity string format)
    fn read_string(cursor: &mut Cursor<&[u8]>) -> Result<String> {
        let length = cursor.read_u32::<LittleEndian>()?;
        let mut bytes = vec![0u8; length as usize];
        cursor.read_exact(&mut bytes)?;
        
        // Align to 4-byte boundary
        let padding = (4 - (length % 4)) % 4;
        if padding > 0 {
            cursor.seek(SeekFrom::Current(padding as i64))?;
        }
        
        String::from_utf8(bytes).context("Invalid UTF-8 in string")
    }
    
    /// Parse type tree information
    fn parse_type_tree(cursor: &mut Cursor<&[u8]>, version: u32) -> Result<HashMap<i32, TypeInfo>> {
        let mut type_tree = HashMap::new();
        
        if version >= 7 {
            // Read type count
            let type_count = cursor.read_u32::<LittleEndian>()?;
            
            for _ in 0..type_count {
                let class_id = cursor.read_i32::<LittleEndian>()?;
                let is_stripped_type = cursor.read_u8()? != 0;
                let script_type_index = cursor.read_i16::<LittleEndian>()?;
                
                // For common types, use known names
                let type_name = match class_id {
                    1 => "GameObject".to_string(),
                    4 => "Transform".to_string(),
                    21 => "Material".to_string(),
                    23 => "MeshRenderer".to_string(),
                    28 => "Texture2D".to_string(),
                    33 => "MeshFilter".to_string(),
                    43 => "Mesh".to_string(),
                    74 => "AnimationClip".to_string(),
                    83 => "AudioClip".to_string(),
                    114 => "MonoBehaviour".to_string(),
                    115 => "MonoScript".to_string(),
                    128 => "Font".to_string(),
                    _ => format!("UnknownType_{}", class_id),
                };
                
                type_tree.insert(class_id, TypeInfo {
                    class_id,
                    is_stripped_type,
                    script_type_index,
                    type_name,
                    type_dependencies: Vec::new(), // Would be populated with real parsing
                });
            }
        } else {
            // Older version format - add basic types
            Self::add_basic_types(&mut type_tree);
        }
        
        Ok(type_tree)
    }
    
    /// Add basic Unity types for older versions
    fn add_basic_types(type_tree: &mut HashMap<i32, TypeInfo>) {
        let basic_types = vec![
            (1, "GameObject"),
            (4, "Transform"),
            (21, "Material"),
            (28, "Texture2D"),
            (43, "Mesh"),
            (83, "AudioClip"),
        ];
        
        for (class_id, name) in basic_types {
            type_tree.insert(class_id, TypeInfo {
                class_id,
                is_stripped_type: false,
                script_type_index: -1,
                type_name: name.to_string(),
                type_dependencies: Vec::new(),
            });
        }
    }
    
    /// Parse object information table
    fn parse_objects(cursor: &mut Cursor<&[u8]>, version: u32) -> Result<Vec<ObjectInfo>> {
        let object_count = cursor.read_u32::<LittleEndian>()?;
        let mut objects = Vec::new();
        
        for _ in 0..object_count {
            // Align to 4-byte boundary
            let pos = cursor.position();
            let aligned_pos = (pos + 3) & !3;
            cursor.seek(SeekFrom::Start(aligned_pos))?;
            
            let path_id = if version >= 14 {
                cursor.read_u64::<LittleEndian>()?
            } else {
                cursor.read_u32::<LittleEndian>()? as u64
            };
            
            let offset = cursor.read_u64::<LittleEndian>()?;
            let size = cursor.read_u32::<LittleEndian>()?;
            let class_id = cursor.read_i32::<LittleEndian>()?;
            
            objects.push(ObjectInfo {
                path_id,
                offset,
                size,
                class_id,
            });
        }
        
        Ok(objects)
    }
    
    /// Parse script type information
    fn parse_script_types(cursor: &mut Cursor<&[u8]>) -> Result<Vec<LocalSerializedObjectIdentifier>> {
        let script_count = cursor.read_u32::<LittleEndian>()?;
        let mut script_types = Vec::new();
        
        for _ in 0..script_count {
            let local_serialized_file_index = cursor.read_i32::<LittleEndian>()?;
            let local_identifier_in_file = cursor.read_u64::<LittleEndian>()?;
            
            script_types.push(LocalSerializedObjectIdentifier {
                local_serialized_file_index,
                local_identifier_in_file,
            });
        }
        
        Ok(script_types)
    }
    
    /// Parse external file references
    fn parse_externals(cursor: &mut Cursor<&[u8]>) -> Result<Vec<FileIdentifier>> {
        let external_count = cursor.read_u32::<LittleEndian>()?;
        let mut externals = Vec::new();
        
        for _ in 0..external_count {
            let temp_empty = Self::read_string(cursor)?;
            
            let mut guid = [0u8; 16];
            cursor.read_exact(&mut guid)?;
            
            let type_ = cursor.read_i32::<LittleEndian>()?;
            let path_name = Self::read_string(cursor)?;
            
            externals.push(FileIdentifier {
                temp_empty,
                guid,
                type_,
                path_name,
            });
        }
        
        Ok(externals)
    }
    
    /// Get data for a specific object
    pub fn get_object_data(&self, object: &ObjectInfo, source_data: &[u8]) -> Result<Vec<u8>> {
        let start_offset = self.data_offset as u64 + object.offset;
        let end_offset = start_offset + object.size as u64;
        
        if end_offset > source_data.len() as u64 {
            bail!("Object data extends beyond file boundaries");
        }
        
        Ok(source_data[start_offset as usize..end_offset as usize].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_unity_version_parsing() {
        let version = UnityVersion::parse("2022.3.15f1").unwrap();
        assert_eq!(version.major, 2022);
        assert_eq!(version.minor, 3);
        assert_eq!(version.patch, 15);
        assert_eq!(version.build, "f1");
        assert!(version.supports_unity_fs());
        assert!(version.uses_new_serialization());
        
        let old_version = UnityVersion::parse("5.2.0f1").unwrap();
        assert!(!old_version.supports_unity_fs());
        assert!(!old_version.uses_new_serialization());
        
        let new_version = UnityVersion::parse("5.3.0f1").unwrap();
        assert!(new_version.supports_unity_fs());
        assert!(!new_version.uses_new_serialization());
    }
    
    #[test]
    fn test_invalid_version_parsing() {
        assert!(UnityVersion::parse("invalid").is_err());
        assert!(UnityVersion::parse("2022").is_err());
    }
}
