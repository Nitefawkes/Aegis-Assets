use anyhow::{Result, Context, bail};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use std::collections::HashMap;
use std::io::{Cursor, Read, Seek, SeekFrom};

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
}

/// Unity asset bundle structure (UnityFS format)
#[derive(Debug, Clone)]
pub struct AssetBundle {
    pub signature: String,
    pub version: u32,
    pub unity_version: String,
    pub unity_revision: String,
    pub size: u64,
    pub compressed_blocks_info_size: u32,
    pub uncompressed_blocks_info_size: u32,
    pub flags: u32,
    pub blocks_info: Vec<BlockInfo>,
    pub directory_info: Vec<DirectoryInfo>,
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
        let mut cursor = Cursor::new(data);
        
        // Read signature
        let mut signature = [0u8; 8];
        cursor.read_exact(&mut signature)?;
        let signature = String::from_utf8_lossy(&signature).trim_end_matches('\0').to_string();
        
        if signature != "UnityFS" {
            bail!("Invalid UnityFS signature: {}", signature);
        }
        
        // Read format version
        let version = cursor.read_u32::<BigEndian>()?;
        
        // Read Unity version string
        let unity_version = Self::read_null_terminated_string(&mut cursor)?;
        
        // Read Unity revision string
        let unity_revision = Self::read_null_terminated_string(&mut cursor)?;
        
        // Read bundle size
        let size = cursor.read_u64::<BigEndian>()?;
        
        // Read compressed blocks info size
        let compressed_blocks_info_size = cursor.read_u32::<BigEndian>()?;
        
        // Read uncompressed blocks info size
        let uncompressed_blocks_info_size = cursor.read_u32::<BigEndian>()?;
        
        // Read flags
        let flags = cursor.read_u32::<BigEndian>()?;
        
        // Read blocks info (simplified for this example)
        let blocks_info = Vec::new(); // Would parse actual block info here
        
        // Read directory info (simplified for this example)
        let directory_info = Self::parse_directory_info(&mut cursor, flags)?;
        
        Ok(AssetBundle {
            signature,
            version,
            unity_version,
            unity_revision,
            size,
            compressed_blocks_info_size,
            uncompressed_blocks_info_size,
            flags,
            blocks_info,
            directory_info,
        })
    }
    
    /// Read null-terminated string from cursor
    fn read_null_terminated_string(cursor: &mut Cursor<&[u8]>) -> Result<String> {
        let mut bytes = Vec::new();
        
        loop {
            let byte = cursor.read_u8()?;
            if byte == 0 {
                break;
            }
            bytes.push(byte);
        }
        
        String::from_utf8(bytes).context("Invalid UTF-8 in string")
    }
    
    /// Parse directory information (simplified)
    fn parse_directory_info(cursor: &mut Cursor<&[u8]>, flags: u32) -> Result<Vec<DirectoryInfo>> {
        // This is a simplified implementation
        // Real implementation would handle the complex UnityFS directory structure
        
        let mut directory_info = Vec::new();
        
        // Mock directory entry for demonstration
        directory_info.push(DirectoryInfo {
            offset: 0,
            size: 1024,
            flags: 0,
            name: "mock_asset.unity3d".to_string(),
            compressed_size: 512,
            compression_type: 0,
        });
        
        Ok(directory_info)
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
        
        // Determine endianness
        let endianness_byte = cursor.read_u8()?;
        let endianness = endianness_byte != 0;
        
        // Skip reserved bytes
        cursor.seek(SeekFrom::Current(3))?;
        
        // Read Unity version (format varies by file version)
        let unity_version = if version >= 9 {
            Self::read_string(&mut cursor)?
        } else {
            "Unknown".to_string()
        };
        
        // Read target platform
        let target_platform = cursor.read_u32::<LittleEndian>()?;
        
        // Read type tree (simplified)
        let type_tree = Self::parse_type_tree(&mut cursor, version)?;
        
        // Read objects
        let objects = Self::parse_objects(&mut cursor, version)?;
        
        // Read script types (if present)
        let script_types = Vec::new(); // Simplified
        
        // Read externals (if present)
        let externals = Vec::new(); // Simplified
        
        Ok(SerializedFile {
            metadata_size,
            file_size,
            version,
            data_offset,
            endianness,
            unity_version,
            target_platform,
            type_tree,
            objects,
            script_types,
            externals,
        })
    }
    
    /// Read string from cursor
    fn read_string(cursor: &mut Cursor<&[u8]>) -> Result<String> {
        let length = cursor.read_u32::<LittleEndian>()?;
        let mut bytes = vec![0u8; length as usize];
        cursor.read_exact(&mut bytes)?;
        
        // Align to 4-byte boundary
        let padding = (4 - (length % 4)) % 4;
        cursor.seek(SeekFrom::Current(padding as i64))?;
        
        String::from_utf8(bytes).context("Invalid UTF-8 in string")
    }
    
    /// Parse type tree (simplified)
    fn parse_type_tree(cursor: &mut Cursor<&[u8]>, version: u32) -> Result<HashMap<i32, TypeInfo>> {
        let mut type_tree = HashMap::new();
        
        // Simplified type tree parsing
        // Real implementation would handle the complex type system
        
        // Add some common Unity types
        type_tree.insert(1, TypeInfo {
            class_id: 1,
            is_stripped_type: false,
            script_type_index: -1,
            type_name: "GameObject".to_string(),
            type_dependencies: vec![],
        });
        
        type_tree.insert(28, TypeInfo {
            class_id: 28,
            is_stripped_type: false,
            script_type_index: -1,
            type_name: "Texture2D".to_string(),
            type_dependencies: vec![],
        });
        
        type_tree.insert(43, TypeInfo {
            class_id: 43,
            is_stripped_type: false,
            script_type_index: -1,
            type_name: "Mesh".to_string(),
            type_dependencies: vec![],
        });
        
        Ok(type_tree)
    }
    
    /// Parse object information (simplified)
    fn parse_objects(cursor: &mut Cursor<&[u8]>, version: u32) -> Result<Vec<ObjectInfo>> {
        let mut objects = Vec::new();
        
        // Simplified object parsing
        // Real implementation would read the object table
        
        // Mock object for demonstration
        objects.push(ObjectInfo {
            path_id: 1,
            offset: 1024,
            size: 512,
            class_id: 28, // Texture2D
        });
        
        Ok(objects)
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
        
        let old_version = UnityVersion::parse("5.2.0f1").unwrap();
        assert!(!old_version.supports_unity_fs());
        
        let new_version = UnityVersion::parse("5.3.0f1").unwrap();
        assert!(new_version.supports_unity_fs());
    }
    
    #[test]
    fn test_invalid_version_parsing() {
        assert!(UnityVersion::parse("invalid").is_err());
        assert!(UnityVersion::parse("2022").is_err());
    }
}
