//! Advanced plugin template with full testing and CI/CD

use std::collections::HashMap;

/// Get advanced plugin template files
pub fn get_advanced_template_files() -> HashMap<String, String> {
    // For now, return the Unity template as the advanced template
    // In a full implementation, this would have additional features
    super::unity::get_unity_template_files()
}

/// Get custom template files
pub fn get_custom_template_files() -> HashMap<String, String> {
    // For now, return the minimal template
    // In a full implementation, this would prompt for custom options
    super::minimal::get_minimal_template_files()
}