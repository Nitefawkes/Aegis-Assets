//! Asset Database and Search System
//!
//! This module provides a comprehensive asset indexing and search system
//! for organizing extracted game assets and enabling efficient discovery.
//!
//! # Features
//!
//! - **Asset Indexing**: Store metadata about extracted assets
//! - **Full-text Search**: Search by name, description, tags, etc.
//! - **Categorization**: Organize assets by type, source, tags
//! - **Metadata Storage**: Store provenance, compliance, and export info
//! - **Efficient Queries**: Fast search and filtering capabilities
//!
//! # Example
//!
//! ```rust,ignore
//! use aegis_core::asset_db::{AssetDatabase, AssetMetadata};
//!
//! // Create database
//! let mut db = AssetDatabase::new("./assets.db")?;
//!
//! // Index an asset
//! let metadata = AssetMetadata {
//!     id: "mesh_001".to_string(),
//!     name: "Hero Character".to_string(),
//!     asset_type: AssetType::Mesh,
//!     source_path: PathBuf::from("game_data/hero.mesh"),
//!     tags: vec!["character".to_string(), "hero".to_string()],
//!     // ... other fields
//! };
//!
//! db.index_asset(metadata)?;
//!
//! // Search assets
//! let results = db.search("character hero")?;
//! ```

use crate::archive::Provenance;
use crate::export::ExportFormat;
use crate::resource::ResourceType;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Asset type classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetType {
    Texture,
    Mesh,
    Audio,
    Animation,
    Material,
    Level,
    Script,
    Prefab,
    Other(String),
}

impl From<ResourceType> for AssetType {
    fn from(resource_type: ResourceType) -> Self {
        match resource_type {
            ResourceType::Texture => AssetType::Texture,
            ResourceType::Mesh => AssetType::Mesh,
            ResourceType::Audio => AssetType::Audio,
            ResourceType::Animation => AssetType::Animation,
            ResourceType::Material => AssetType::Material,
            ResourceType::Level => AssetType::Level,
            ResourceType::Script => AssetType::Script,
            _ => AssetType::Other("Generic".to_string()),
        }
    }
}

/// Comprehensive asset metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetMetadata {
    /// Unique asset identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Asset type classification
    pub asset_type: AssetType,
    /// Original source file path
    pub source_path: PathBuf,
    /// Output directory where asset is stored
    pub output_path: PathBuf,
    /// File size in bytes
    pub file_size: u64,
    /// Export format (if converted)
    pub export_format: Option<ExportFormat>,
    /// Tags for categorization and search
    pub tags: Vec<String>,
    /// Description or notes
    pub description: Option<String>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last modification timestamp
    pub updated_at: DateTime<Utc>,
    /// Source game identifier
    pub game_id: Option<String>,
    /// Compliance level
    pub compliance_level: String,
    /// Search keywords (auto-generated)
    pub search_keywords: Vec<String>,
    /// Asset hash for integrity checking
    pub content_hash: String,
    /// Provenance information
    pub provenance: Option<Provenance>,
}

/// Search query structure
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// Text to search for (searches name, description, tags, keywords)
    pub text: Option<String>,
    /// Filter by asset type
    pub asset_type: Option<AssetType>,
    /// Filter by tags (must contain all specified tags)
    pub tags: Vec<String>,
    /// Filter by game ID
    pub game_id: Option<String>,
    /// Filter by compliance level
    pub compliance_level: Option<String>,
    /// Limit number of results
    pub limit: Option<usize>,
    /// Sort order
    pub sort_by: SortOrder,
}

/// Sort order for search results
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum SortOrder {
    NameAsc,
    NameDesc,
    CreatedAsc,
    CreatedDesc,
    SizeAsc,
    SizeDesc,
    Relevance,
}

/// Search result with relevance score
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub asset: AssetMetadata,
    pub relevance_score: f32,
    pub matched_fields: Vec<String>,
}

/// Asset database for indexing and searching
#[derive(Debug)]
pub struct AssetDatabase {
    /// Database file path
    db_path: PathBuf,
    /// In-memory asset index
    assets: HashMap<String, AssetMetadata>,
    /// Search index for fast text search
    search_index: HashMap<String, HashSet<String>>, // term -> asset_ids
    /// Tag index for fast tag filtering
    tag_index: HashMap<String, HashSet<String>>, // tag -> asset_ids
}

impl AssetDatabase {
    /// Create a new asset database
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();

        let mut db = Self {
            db_path: db_path.clone(),
            assets: HashMap::new(),
            search_index: HashMap::new(),
            tag_index: HashMap::new(),
        };

        // Load existing database if it exists
        if db_path.exists() {
            db.load()?;
        }

        Ok(db)
    }

    /// Index a new asset
    pub fn index_asset(&mut self, metadata: AssetMetadata) -> Result<()> {
        let asset_id = metadata.id.clone();

        // Add to main index
        self.assets.insert(asset_id.clone(), metadata.clone());

        // Update search index
        self.update_search_index(&asset_id, &metadata);

        // Update tag index
        self.update_tag_index(&asset_id, &metadata.tags);

        // Save to disk
        self.save()?;

        Ok(())
    }

    /// Remove an asset from the database
    pub fn remove_asset(&mut self, asset_id: &str) -> Result<()> {
        if let Some(metadata) = self.assets.remove(asset_id) {
            // Remove from search index
            self.remove_from_search_index(asset_id, &metadata);

            // Remove from tag index
            self.remove_from_tag_index(asset_id, &metadata.tags);

            // Save to disk
            self.save()?;
        }

        Ok(())
    }

    /// Get asset by ID
    pub fn get_asset(&self, asset_id: &str) -> Option<&AssetMetadata> {
        self.assets.get(asset_id)
    }

    /// Search assets using query
    pub fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>> {
        let mut candidates = HashSet::new();

        // Start with all assets if no specific filters
        if query.text.is_none()
            && query.asset_type.is_none()
            && query.tags.is_empty()
            && query.game_id.is_none()
            && query.compliance_level.is_none()
        {
            candidates = self.assets.keys().cloned().collect();
        }

        // Apply text search
        if let Some(text) = &query.text {
            let text_candidates = self.search_text(text);
            if candidates.is_empty() {
                candidates = text_candidates;
            } else {
                candidates = candidates.intersection(&text_candidates).cloned().collect();
            }
        }

        // Apply type filter
        if let Some(asset_type) = &query.asset_type {
            let type_candidates: HashSet<String> = self
                .assets
                .iter()
                .filter(|(_, metadata)| &metadata.asset_type == asset_type)
                .map(|(id, _)| id.clone())
                .collect();

            if candidates.is_empty() {
                candidates = type_candidates;
            } else {
                candidates = candidates.intersection(&type_candidates).cloned().collect();
            }
        }

        // Apply tag filter (must contain ALL specified tags)
        for tag in &query.tags {
            if let Some(tag_candidates) = self.tag_index.get(tag) {
                if candidates.is_empty() {
                    candidates = tag_candidates.clone();
                } else {
                    candidates = candidates.intersection(tag_candidates).cloned().collect();
                }
            } else {
                // No assets with this tag
                return Ok(Vec::new());
            }
        }

        // Apply game ID filter
        if let Some(game_id) = &query.game_id {
            let game_candidates: HashSet<String> = self
                .assets
                .iter()
                .filter(|(_, metadata)| metadata.game_id.as_ref() == Some(game_id))
                .map(|(id, _)| id.clone())
                .collect();

            if candidates.is_empty() {
                candidates = game_candidates;
            } else {
                candidates = candidates.intersection(&game_candidates).cloned().collect();
            }
        }

        // Apply compliance level filter
        if let Some(compliance) = &query.compliance_level {
            let compliance_candidates: HashSet<String> = self
                .assets
                .iter()
                .filter(|(_, metadata)| &metadata.compliance_level == compliance)
                .map(|(id, _)| id.clone())
                .collect();

            if candidates.is_empty() {
                candidates = compliance_candidates;
            } else {
                candidates = candidates
                    .intersection(&compliance_candidates)
                    .cloned()
                    .collect();
            }
        }

        // Convert to search results
        let mut results: Vec<SearchResult> = candidates
            .iter()
            .filter_map(|id| self.assets.get(id))
            .map(|metadata| SearchResult {
                asset: metadata.clone(),
                relevance_score: 1.0,       // TODO: Implement proper scoring
                matched_fields: Vec::new(), // TODO: Track matched fields
            })
            .collect();

        // Sort results
        match query.sort_by {
            SortOrder::NameAsc => results.sort_by(|a, b| a.asset.name.cmp(&b.asset.name)),
            SortOrder::NameDesc => results.sort_by(|a, b| b.asset.name.cmp(&a.asset.name)),
            SortOrder::CreatedAsc => {
                results.sort_by(|a, b| a.asset.created_at.cmp(&b.asset.created_at))
            }
            SortOrder::CreatedDesc => {
                results.sort_by(|a, b| b.asset.created_at.cmp(&a.asset.created_at))
            }
            SortOrder::SizeAsc => results.sort_by(|a, b| a.asset.file_size.cmp(&b.asset.file_size)),
            SortOrder::SizeDesc => {
                results.sort_by(|a, b| b.asset.file_size.cmp(&a.asset.file_size))
            }
            SortOrder::Relevance => {
                results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap())
            }
        }

        // Apply limit
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    /// Get all assets
    pub fn get_all_assets(&self) -> Vec<&AssetMetadata> {
        self.assets.values().collect()
    }

    /// Get assets by type
    pub fn get_assets_by_type(&self, asset_type: &AssetType) -> Vec<&AssetMetadata> {
        self.assets
            .values()
            .filter(|metadata| &metadata.asset_type == asset_type)
            .collect()
    }

    /// Get assets by tag
    pub fn get_assets_by_tag(&self, tag: &str) -> Vec<&AssetMetadata> {
        self.tag_index
            .get(tag)
            .map(|asset_ids| {
                asset_ids
                    .iter()
                    .filter_map(|id| self.assets.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get statistics
    pub fn get_stats(&self) -> DatabaseStats {
        let mut type_counts = HashMap::new();
        let mut total_size = 0u64;
        let mut tag_counts = HashMap::new();

        for metadata in self.assets.values() {
            // Count by type
            let type_str = format!("{:?}", metadata.asset_type);
            *type_counts.entry(type_str).or_insert(0) += 1;

            // Sum sizes
            total_size += metadata.file_size;

            // Count tags
            for tag in &metadata.tags {
                *tag_counts.entry(tag.clone()).or_insert(0) += 1;
            }
        }

        DatabaseStats {
            total_assets: self.assets.len(),
            total_size,
            assets_by_type: type_counts,
            tags: tag_counts,
        }
    }

    /// Search by text (name, description, tags, keywords)
    fn search_text(&self, query: &str) -> HashSet<String> {
        let mut results = HashSet::new();
        let query_lower = query.to_lowercase();

        // Search in main index
        for (term, asset_ids) in &self.search_index {
            if term.contains(&query_lower) {
                results.extend(asset_ids.iter().cloned());
            }
        }

        // Also search in asset names and descriptions
        for (asset_id, metadata) in &self.assets {
            if metadata.name.to_lowercase().contains(&query_lower)
                || metadata
                    .description
                    .as_ref()
                    .map(|desc| desc.to_lowercase().contains(&query_lower))
                    .unwrap_or(false)
                || metadata
                    .tags
                    .iter()
                    .any(|tag| tag.to_lowercase().contains(&query_lower))
            {
                results.insert(asset_id.clone());
            }
        }

        results
    }

    /// Update search index for an asset
    fn update_search_index(&mut self, asset_id: &str, metadata: &AssetMetadata) {
        // Remove old entries
        self.remove_from_search_index(asset_id, metadata);

        // Add new entries
        let terms = self.extract_search_terms(metadata);
        for term in terms {
            self.search_index
                .entry(term)
                .or_insert_with(HashSet::new)
                .insert(asset_id.to_string());
        }
    }

    /// Remove asset from search index
    fn remove_from_search_index(&mut self, asset_id: &str, metadata: &AssetMetadata) {
        let terms = self.extract_search_terms(metadata);
        for term in terms {
            if let Some(asset_ids) = self.search_index.get_mut(&term) {
                asset_ids.remove(asset_id);
                if asset_ids.is_empty() {
                    self.search_index.remove(&term);
                }
            }
        }
    }

    /// Update tag index for an asset
    fn update_tag_index(&mut self, asset_id: &str, tags: &[String]) {
        // Remove old tags
        for (_tag, asset_ids) in &mut self.tag_index {
            asset_ids.remove(asset_id);
        }

        // Clean up empty tag entries
        self.tag_index.retain(|_, asset_ids| !asset_ids.is_empty());

        // Add new tags
        for tag in tags {
            self.tag_index
                .entry(tag.clone())
                .or_insert_with(HashSet::new)
                .insert(asset_id.to_string());
        }
    }

    /// Remove asset from tag index
    fn remove_from_tag_index(&mut self, asset_id: &str, tags: &[String]) {
        for tag in tags {
            if let Some(asset_ids) = self.tag_index.get_mut(tag) {
                asset_ids.remove(asset_id);
                if asset_ids.is_empty() {
                    self.tag_index.remove(tag);
                }
            }
        }
    }

    /// Extract search terms from asset metadata
    fn extract_search_terms(&self, metadata: &AssetMetadata) -> Vec<String> {
        let mut terms = Vec::new();

        // Add name terms
        terms.extend(metadata.name.split_whitespace().map(|s| s.to_lowercase()));

        // Add description terms
        if let Some(desc) = &metadata.description {
            terms.extend(desc.split_whitespace().map(|s| s.to_lowercase()));
        }

        // Add tags
        terms.extend(metadata.tags.iter().map(|tag| tag.to_lowercase()));

        // Add keywords
        terms.extend(metadata.search_keywords.iter().map(|kw| kw.to_lowercase()));

        // Add file extension
        if let Some(ext) = metadata.source_path.extension() {
            if let Some(ext_str) = ext.to_str() {
                terms.push(ext_str.to_lowercase());
            }
        }

        terms
    }

    /// Save database to disk
    fn save(&self) -> Result<()> {
        let data =
            serde_json::to_string_pretty(&self.assets).context("Failed to serialize database")?;

        fs::write(&self.db_path, data).context("Failed to write database file")?;

        Ok(())
    }

    /// Load database from disk
    fn load(&mut self) -> Result<()> {
        let data = fs::read_to_string(&self.db_path).context("Failed to read database file")?;

        self.assets = serde_json::from_str(&data).context("Failed to deserialize database")?;

        // Rebuild search and tag indices
        let assets_clone: Vec<(String, AssetMetadata)> = self.assets.clone().into_iter().collect();
        for (asset_id, metadata) in assets_clone {
            self.update_search_index(&asset_id, &metadata);
            self.update_tag_index(&asset_id, &metadata.tags);
        }

        Ok(())
    }
}

/// Database statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStats {
    pub total_assets: usize,
    pub total_size: u64,
    pub assets_by_type: HashMap<String, usize>,
    pub tags: HashMap<String, usize>,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            text: None,
            asset_type: None,
            tags: Vec::new(),
            game_id: None,
            compliance_level: None,
            limit: Some(50),
            sort_by: SortOrder::Relevance,
        }
    }
}

impl AssetMetadata {
    /// Create new asset metadata
    pub fn new(
        id: String,
        name: String,
        asset_type: AssetType,
        source_path: PathBuf,
        output_path: PathBuf,
        file_size: u64,
        content_hash: String,
    ) -> Self {
        let now = Utc::now();

        Self {
            id,
            name,
            asset_type,
            source_path,
            output_path,
            file_size,
            export_format: None,
            tags: Vec::new(),
            description: None,
            created_at: now,
            updated_at: now,
            game_id: None,
            compliance_level: "unknown".to_string(),
            search_keywords: Vec::new(),
            content_hash,
            provenance: None,
        }
    }

    /// Add a tag to the asset
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Set description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set game ID
    pub fn with_game_id(mut self, game_id: impl Into<String>) -> Self {
        self.game_id = Some(game_id.into());
        self
    }

    /// Set compliance level
    pub fn with_compliance_level(mut self, level: impl Into<String>) -> Self {
        self.compliance_level = level.into();
        self
    }

    /// Add search keywords
    pub fn with_keywords(mut self, keywords: Vec<String>) -> Self {
        self.search_keywords = keywords;
        self
    }
}
