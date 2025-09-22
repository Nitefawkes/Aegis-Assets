use crate::extract::{ExtractionError, Extractor};
use crate::{archive::ComplianceRegistry, AegisCore, PluginRegistry};
use tempfile::TempDir;

/// Integration test for the core extraction pipeline
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extractor_creation() {
        let plugin_registry = PluginRegistry::new();
        let compliance_registry = ComplianceRegistry::new();

        let _extractor = Extractor::new(plugin_registry, compliance_registry);
        // If we get here, the extractor was created successfully
    }

    #[test]
    fn test_aegis_core_creation() {
        let aegis = AegisCore::new().expect("Failed to initialize AegisCore");

        // Test that we can get system info
        let info = aegis.system_info();
        assert_eq!(info.registered_plugins, 0); // No plugins registered yet
        assert_eq!(info.compliance_profiles, 0); // No profiles loaded yet
    }

    #[test]
    fn test_extraction_with_no_plugins() {
        let plugin_registry = PluginRegistry::new();
        let compliance_registry = ComplianceRegistry::new();

        let mut extractor = Extractor::new(plugin_registry, compliance_registry);
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.dat");

        // Create a dummy file
        std::fs::write(&test_file, b"dummy content").unwrap();

        // Should fail because no plugins are registered
        let result = extractor.extract_from_file(&test_file, temp_dir.path());

        match result {
            Err(ExtractionError::NoSuitablePlugin(_)) => {
                // This is expected - no plugins registered
            }
            other => panic!("Expected NoSuitablePlugin error, got: {:?}", other),
        }
    }
}

/// Create a basic end-to-end test setup
pub fn create_test_setup() -> (AegisCore, TempDir) {
    let aegis = AegisCore::new().expect("Failed to create AegisCore");
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    (aegis, temp_dir)
}
