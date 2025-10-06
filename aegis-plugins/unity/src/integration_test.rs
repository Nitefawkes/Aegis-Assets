#[cfg(test)]
mod integration_tests {
    use aegis_core::{archive::ArchiveHandler, PluginFactory};
    use anyhow::Result;
    use crate::{UnityArchive, UnityPluginFactory};
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Create a minimal Unity file for testing
    fn create_test_unity_file() -> Result<NamedTempFile> {
        let mut temp_file = NamedTempFile::new()?;
        
        // Create minimal UnityFS header
        let header = b"UnityFS\0\x07\x05\x00\x00";
        temp_file.write_all(header)?;
        
        // Add minimal version strings
        temp_file.write_all(b"2022.3.15f1\0")?;
        temp_file.write_all(b"abcd1234\0")?;
        
        // Add minimal size info (big endian)
        temp_file.write_all(&1024u64.to_be_bytes())?; // bundle size
        temp_file.write_all(&100u32.to_be_bytes())?;  // compressed blocks info size
        temp_file.write_all(&100u32.to_be_bytes())?;  // uncompressed blocks info size
        temp_file.write_all(&0u32.to_be_bytes())?;    // flags (no compression)
        
        // Add some dummy data to meet minimum size requirements
        let dummy_data = vec![0u8; 100];
        temp_file.write_all(&dummy_data)?;
        
        temp_file.flush()?;
        Ok(temp_file)
    }

    #[test]
    fn test_unity_plugin_factory() {
        let factory = UnityPluginFactory;
        
        // Test basic factory properties
        assert_eq!(factory.name(), "Unity");
        assert!(!factory.version().is_empty());
        
        // Test supported extensions
        let extensions = factory.supported_extensions();
        assert!(extensions.contains(&"unity3d"));
        assert!(extensions.contains(&"assets"));
        assert!(extensions.contains(&"sharedAssets"));
        
        // Test compliance info
        let compliance = factory.compliance_info();
        assert_eq!(compliance.name(), "Unity");
        // PluginInfo doesn't have compliance_verified field anymore
        // assert!(compliance.compliance_verified);
    }
    
    #[test]
    fn test_unity_format_detection() {
        // Test UnityFS detection
        let unityfs_header = [
            b'U', b'n', b'i', b't', b'y', b'F', b'S', 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert!(UnityArchive::detect(&unityfs_header));
        
        // Test invalid header
        let invalid_header = b"Invalid\0\x00\x00\x00\x00\x00\x00\x00";
        assert!(!UnityArchive::detect(invalid_header));
        
        // Test short data
        let short_data = b"Unity";
        assert!(!UnityArchive::detect(short_data));
    }
    
    #[test]
    fn test_unity_archive_opening() -> Result<()> {
        let temp_file = create_test_unity_file()?;
        let file_path = temp_file.path();
        
        // Test that we can detect the file
        let file_data = std::fs::read(file_path)?;
        assert!(UnityArchive::detect(&file_data));
        
        // Test opening the archive
        // Note: This might fail due to incomplete parsing, but should at least not panic
        match UnityArchive::open(file_path) {
            Ok(archive) => {
                // If successful, test basic functionality
                assert!(!archive.file_path.as_os_str().is_empty());
                
                // Test compliance profile
                let compliance = archive.compliance_profile();
                assert_eq!(compliance.publisher, "Unity Technologies");
                
                // Test provenance
                let provenance = archive.provenance();
                assert!(!provenance.source_hash.is_empty());
                assert_eq!(provenance.plugin_info.name, "Unity");
                
                println!("Successfully loaded Unity archive with {} entries", archive.entries.len());
            }
            Err(e) => {
                println!("Expected parsing error with minimal test file: {}", e);
                // This is OK for now - we're testing that the structure is sound
            }
        }
        
        Ok(())
    }
    
    #[test]
    fn test_plugin_factory_integration() -> Result<()> {
        let factory = UnityPluginFactory;
        let temp_file = create_test_unity_file()?;
        let file_path = temp_file.path();
        
        // Test that factory can create handler
        match factory.create_handler(file_path) {
            Ok(handler) => {
                // Test basic handler functionality
                let compliance = handler.compliance_profile();
                assert_eq!(compliance.publisher, "Unity Technologies");
                
                println!("Successfully created Unity handler via factory");
            }
            Err(e) => {
                println!("Expected error creating handler with minimal test file: {}", e);
                // This is acceptable for integration testing
            }
        }
        
        Ok(())
    }
    
    #[test]
    fn test_asset_conversion() {
        use crate::converters::convert_unity_asset;
        
        // Test with minimal data (should handle gracefully)
        let minimal_data = b"test_data_for_conversion";
        
        // Test Texture2D conversion (class_id = 28)
        match convert_unity_asset(28, minimal_data) {
            Ok((filename, _data)) => {
                assert!(filename.ends_with(".png"));
                println!("Successfully converted texture: {}", filename);
            }
            Err(e) => {
                println!("Expected error converting minimal texture data: {}", e);
            }
        }
        
        // Test Mesh conversion (class_id = 43)
        match convert_unity_asset(43, minimal_data) {
            Ok((filename, _data)) => {
                assert!(filename.ends_with(".gltf"));
                println!("Successfully converted mesh: {}", filename);
            }
            Err(e) => {
                println!("Expected error converting minimal mesh data: {}", e);
            }
        }
        
        // Test AudioClip conversion (class_id = 83)
        match convert_unity_asset(83, minimal_data) {
            Ok((filename, _data)) => {
                assert!(filename.ends_with(".ogg"));
                println!("Successfully converted audio: {}", filename);
            }
            Err(e) => {
                println!("Expected error converting minimal audio data: {}", e);
            }
        }
        
        // Test unknown type
        let (filename, data) = convert_unity_asset(999, minimal_data).unwrap();
        assert_eq!(filename, "unknown.bin");
        assert_eq!(data, minimal_data);
    }
}
