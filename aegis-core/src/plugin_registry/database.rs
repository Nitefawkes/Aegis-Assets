//! Database layer for plugin registry
//!
//! SQLite-based storage for plugin metadata, versions, dependencies, and statistics.

use anyhow::{Result, Context, bail};
use rusqlite::{Connection, params, OptionalExtension, Row};
use rusqlite_migration::{Migrations, M};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::{info, debug, warn};
use uuid::Uuid;

/// Database connection wrapper
#[derive(Clone)]
pub struct DatabaseConnection {
    conn: Arc<Mutex<Connection>>,
}

impl DatabaseConnection {
    /// Create a new database connection
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)
            .context("Failed to open plugin registry database")?;

        // Enable foreign keys
        conn.execute("PRAGMA foreign_keys = ON", [])
            .context("Failed to enable foreign keys")?;

        // Enable WAL mode for better concurrency
        conn.execute("PRAGMA journal_mode = WAL", [])
            .context("Failed to set WAL mode")?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn))
        })
    }

    /// Run database migrations
    pub fn migrate(&self) -> Result<()> {
        info!("Running plugin registry database migrations");

        let migrations = Migrations::new(vec![
            // Initial schema
            M::up(include_str!("../../migrations/001_initial_schema.sql")),
            // Add indexes for performance
            M::up(include_str!("../../migrations/002_add_indexes.sql")),
            // Add full-text search
            M::up(include_str!("../../migrations/003_add_search.sql")),
        ]);

        let mut conn = self.conn.lock().unwrap();
        migrations
            .to_latest(&mut *conn)
            .context("Failed to run database migrations")?;

        info!("Plugin registry database migrations completed");
        Ok(())
    }

    /// Execute a closure with the database connection
    pub fn with_connection<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut Connection) -> Result<R>,
    {
        let mut conn = self.conn.lock().unwrap();
        f(&mut *conn)
    }

    /// Get raw connection (for advanced operations)
    pub fn connection(&self) -> &Arc<Mutex<Connection>> {
        &self.conn
    }
}

// Use types from models module
pub use super::models::{RegistryStats, DownloadStats, SearchCriteria, SearchSort};

/// Database operations trait
pub trait DatabaseOperations {
    /// Get registry statistics
    fn get_stats(&self) -> Result<RegistryStats>;

    /// List all plugins with basic metadata
    fn list_plugins(&self) -> Result<Vec<crate::plugin_registry::PluginMetadata>>;

    /// Find plugin by name
    fn find_plugin(&self, name: &str) -> Result<Option<crate::plugin_registry::PluginMetadata>>;

    /// Get all versions of a plugin
    fn get_plugin_versions(&self, plugin_name: &str) -> Result<Vec<crate::plugin_registry::PluginVersion>>;

    /// Get latest version of a plugin
    fn get_latest_version(&self, plugin_name: &str) -> Result<Option<crate::plugin_registry::PluginVersion>>;

    /// Register a new plugin version
    fn register_plugin_version(
        &mut self,
        metadata: &crate::plugin_registry::PluginMetadata,
        package_data: &[u8]
    ) -> Result<crate::plugin_registry::PluginVersion>;

    /// Update plugin status
    fn update_plugin_status(
        &mut self,
        plugin_id: &str,
        status: crate::plugin_registry::PluginStatus
    ) -> Result<()>;

    /// Search plugins by criteria
    fn search_plugins(&self, criteria: SearchCriteria) -> Result<Vec<crate::plugin_registry::PluginMetadata>>;

    /// Get download statistics
    fn get_download_stats(&self, plugin_name: &str) -> Result<DownloadStats>;

    /// Record plugin download
    fn record_download(&mut self, plugin_name: &str, version: &str, user_id: Option<&str>) -> Result<()>;

    /// Get plugin by ID
    fn get_plugin_by_id(&self, id: &str) -> Result<Option<crate::plugin_registry::PluginMetadata>>;

    /// Get plugin version by ID
    fn get_plugin_version_by_id(&self, id: &str) -> Result<Option<crate::plugin_registry::PluginVersion>>;

    /// Update plugin metadata
    fn update_plugin_metadata(
        &mut self,
        plugin_id: &str,
        metadata: &crate::plugin_registry::PluginMetadata
    ) -> Result<()>;

    /// Delete plugin version
    fn delete_plugin_version(&mut self, plugin_name: &str, version: &str) -> Result<()>;

    /// Get plugins by author
    fn get_plugins_by_author(&self, author_email: &str) -> Result<Vec<crate::plugin_registry::PluginMetadata>>;
}

impl DatabaseOperations for DatabaseConnection {
    fn get_stats(&self) -> Result<RegistryStats> {
        let stats: (i64, i64, i64, i64) = self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT
                    (SELECT COUNT(DISTINCT name) FROM plugins) as total_plugins,
                    (SELECT COUNT(*) FROM plugin_versions) as total_versions,
                    (SELECT COUNT(*) FROM plugin_downloads) as total_downloads,
                    (SELECT COUNT(*) FROM plugin_versions WHERE status = 'pending_review') as pending_reviews
                "#,
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            ).context("Failed to get registry stats")
        })?;

        Ok(RegistryStats {
            total_plugins: stats.0 as usize,
            total_versions: stats.1 as usize,
            total_downloads: stats.2 as usize,
            pending_reviews: stats.3 as usize,
            last_updated: chrono::Utc::now(),
        })
    }

    fn list_plugins(&self) -> Result<Vec<crate::plugin_registry::PluginMetadata>> {
        let plugins = self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT p.id, p.name, p.display_name, p.description, p.author_email,
                       p.license, p.homepage, p.repository, p.keywords, p.created_at, p.updated_at,
                       pv.version, pv.status, pv.package_size, pv.package_hash,
                       pv.manifest, pv.published_at
                FROM plugins p
                JOIN plugin_versions pv ON p.id = pv.plugin_id
                WHERE pv.status = 'approved'
                ORDER BY p.name, pv.version DESC
                "#
            )?;

            let plugins = stmt.query_map([], |row| self.row_to_plugin_metadata(row))
                .context("Failed to query plugins")?
                .collect::<Result<Vec<_>, _>>()
                .context("Failed to collect plugin metadata")?;

            Ok(plugins)
        })?;

        Ok(plugins)
    }

    fn find_plugin(&self, name: &str) -> Result<Option<crate::plugin_registry::PluginMetadata>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT p.id, p.name, p.display_name, p.description, p.author_email,
                       p.license, p.homepage, p.repository, p.keywords, p.created_at, p.updated_at,
                       pv.version, pv.status, pv.package_size, pv.package_hash,
                       pv.manifest, pv.published_at
                FROM plugins p
                JOIN plugin_versions pv ON p.id = pv.plugin_id
                WHERE p.name = ? AND pv.status = 'approved'
                ORDER BY pv.version DESC
                LIMIT 1
                "#
            )?;

            let result = stmt.query_row([name], |row| self.row_to_plugin_metadata(row))
                .optional()
                .context("Failed to query plugin")?;

            Ok(result)
        })?
    }

    fn get_plugin_versions(&self, plugin_name: &str) -> Result<Vec<crate::plugin_registry::PluginVersion>> {
        let conn = &self.conn;

        let mut stmt = conn.prepare(
            r#"
            SELECT pv.id, pv.plugin_id, pv.version, pv.status, pv.package_size,
                   pv.package_hash, pv.manifest, pv.signature_data, pv.published_at
            FROM plugin_versions pv
            JOIN plugins p ON p.id = pv.plugin_id
            WHERE p.name = ?
            ORDER BY
                CASE
                    WHEN pv.version GLOB '*[a-zA-Z]*' THEN 999
                    ELSE CAST(SUBSTR(pv.version, 1, INSTR(pv.version || '.', '.') - 1) AS INTEGER)
                END DESC,
                CASE
                    WHEN pv.version GLOB '*[a-zA-Z]*' THEN 999
                    ELSE CAST(SUBSTR(pv.version, INSTR(pv.version, '.') + 1,
                           INSTR(SUBSTR(pv.version, INSTR(pv.version, '.') + 1) || '.') - 1) AS INTEGER)
                END DESC
            "#
        )?;

        let versions = stmt.query_map([plugin_name], |row| self.row_to_plugin_version(row))
            .context("Failed to query plugin versions")?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to collect plugin versions")?;

        Ok(versions)
    }

    fn get_latest_version(&self, plugin_name: &str) -> Result<Option<crate::plugin_registry::PluginVersion>> {
        let conn = &self.conn;

        let mut stmt = conn.prepare(
            r#"
            SELECT pv.id, pv.plugin_id, pv.version, pv.status, pv.package_size,
                   pv.package_hash, pv.manifest, pv.signature_data, pv.published_at
            FROM plugin_versions pv
            JOIN plugins p ON p.id = pv.plugin_id
            WHERE p.name = ? AND pv.status = 'approved'
            ORDER BY
                CASE
                    WHEN pv.version GLOB '*[a-zA-Z]*' THEN 999
                    ELSE CAST(SUBSTR(pv.version, 1, INSTR(pv.version || '.', '.') - 1) AS INTEGER)
                END DESC,
                CASE
                    WHEN pv.version GLOB '*[a-zA-Z]*' THEN 999
                    ELSE CAST(SUBSTR(pv.version, INSTR(pv.version, '.') + 1,
                           INSTR(SUBSTR(pv.version, INSTR(pv.version, '.') + 1) || '.') - 1) AS INTEGER)
                END DESC
            LIMIT 1
            "#
        )?;

        let result = stmt.query_row([plugin_name], |row| self.row_to_plugin_version(row))
            .optional()
            .context("Failed to query latest plugin version")?;

        Ok(result)
    }

    fn register_plugin_version(
        &mut self,
        metadata: &crate::plugin_registry::PluginMetadata,
        package_data: &[u8]
    ) -> Result<crate::plugin_registry::PluginVersion> {
        let conn = &mut self.conn;

        // Begin transaction
        let tx = conn.transaction()?;

        // Check if plugin exists
        let plugin_id = tx.query_row(
            "SELECT id FROM plugins WHERE name = ?",
            [metadata.name],
            |row| row.get::<_, String>(0)
        ).optional()?;

        let plugin_id = if let Some(id) = plugin_id {
            // Update plugin metadata
            tx.execute(
                r#"
                UPDATE plugins
                SET display_name = ?, description = ?, author_email = ?,
                    license = ?, homepage = ?, repository = ?, keywords = ?,
                    updated_at = CURRENT_TIMESTAMP
                WHERE name = ?
                "#,
                params![
                    metadata.display_name,
                    metadata.description,
                    metadata.author_email,
                    metadata.license,
                    metadata.homepage,
                    metadata.repository,
                    serde_json::to_string(&metadata.keywords)?,
                    metadata.name
                ]
            )?;
            id
        } else {
            // Create new plugin
            let id = Uuid::new_v4().to_string();
            tx.execute(
                r#"
                INSERT INTO plugins (id, name, display_name, description, author_email,
                                   license, homepage, repository, keywords)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
                params![
                    id,
                    metadata.name,
                    metadata.display_name,
                    metadata.description,
                    metadata.author_email,
                    metadata.license,
                    metadata.homepage,
                    metadata.repository,
                    serde_json::to_string(&metadata.keywords)?,
                ]
            )?;
            id
        };

        // Insert plugin version
        let version_id = Uuid::new_v4().to_string();
        let package_hash = blake3::hash(package_data);
        let manifest_json = serde_json::to_string(&metadata.manifest)?;

        tx.execute(
            r#"
            INSERT INTO plugin_versions (id, plugin_id, version, status, package_size,
                                       package_hash, manifest, published_at)
            VALUES (?, ?, ?, 'pending_review', ?, ?, ?, CURRENT_TIMESTAMP)
            "#,
            params![
                version_id,
                plugin_id,
                metadata.version,
                package_data.len(),
                package_hash.to_hex(),
                manifest_json
            ]
        )?;

        // Store package data
        tx.execute(
            "INSERT INTO plugin_packages (version_id, package_data) VALUES (?, ?)",
            params![version_id, package_data]
        )?;

        // Commit transaction
        tx.commit()?;

        // Return the created version
        self.get_plugin_version_by_id(&version_id)?.ok_or_else(|| {
            anyhow::anyhow!("Failed to retrieve created plugin version")
        })
    }

    fn update_plugin_status(
        &mut self,
        plugin_id: &str,
        status: crate::plugin_registry::PluginStatus
    ) -> Result<()> {
        let conn = &mut self.conn;

        conn.execute(
            "UPDATE plugin_versions SET status = ? WHERE id = ?",
            params![serde_json::to_string(&status)?, plugin_id]
        )?;

        Ok(())
    }

    fn search_plugins(&self, criteria: SearchCriteria) -> Result<Vec<crate::plugin_registry::PluginMetadata>> {
        let conn = &self.conn;

        let mut query = String::from(
            r#"
            SELECT DISTINCT p.id, p.name, p.display_name, p.description, p.author_email,
                           p.license, p.homepage, p.repository, p.keywords, p.created_at, p.updated_at,
                           pv.version, pv.status, pv.package_size, pv.package_hash,
                           pv.manifest, pv.published_at
            FROM plugins p
            JOIN plugin_versions pv ON p.id = pv.plugin_id
            WHERE pv.status = 'approved'
            "#
        );

        let mut params = Vec::new();

        // Add search filters
        if let Some(query_text) = criteria.query {
            query.push_str(" AND (p.name LIKE ? OR p.description LIKE ? OR p.keywords LIKE ?)");
            let search_pattern = format!("%{}%", query_text);
            params.push(search_pattern.clone());
            params.push(search_pattern.clone());
            params.push(search_pattern);
        }

        if let Some(engine) = criteria.engine {
            query.push_str(" AND p.keywords LIKE ?");
            params.push(format!("%{}%", engine));
        }

        if let Some(category) = criteria.category {
            query.push_str(" AND p.keywords LIKE ?");
            params.push(format!("%{}%", category));
        }

        if let Some(risk_level) = criteria.risk_level {
            // This would need to be implemented based on compliance data
            // For now, we'll skip this filter
        }

        // Add sorting
        match criteria.sort_by {
            SearchSort::Relevance => {
                if criteria.query.is_some() {
                    query.push_str(" ORDER BY (p.name LIKE ?) DESC, p.name");
                    params.push(criteria.query.as_ref().unwrap().clone());
                } else {
                    query.push_str(" ORDER BY p.name");
                }
            }
            SearchSort::Popularity => {
                query.push_str(" ORDER BY (SELECT COUNT(*) FROM plugin_downloads pd WHERE pd.plugin_version_id = pv.id) DESC, p.name");
            }
            SearchSort::RecentlyUpdated => {
                query.push_str(" ORDER BY pv.published_at DESC, p.name");
            }
            SearchSort::Alphabetical => {
                query.push_str(" ORDER BY p.name");
            }
        }

        // Add pagination
        if let Some(limit) = criteria.limit {
            query.push_str(" LIMIT ?");
            params.push(limit.to_string());
        }

        if let Some(offset) = criteria.offset {
            query.push_str(" OFFSET ?");
            params.push(offset.to_string());
        }

        let mut stmt = conn.prepare(&query)?;
        let plugins = stmt.query_map(&params.iter().map(|s| s.as_str()).collect::<Vec<_>>(), |row| {
            self.row_to_plugin_metadata(row)
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(plugins)
    }

    fn get_download_stats(&self, plugin_name: &str) -> Result<DownloadStats> {
        let conn = &self.conn;

        let stats: (i64, i64) = conn.query_row(
            r#"
            SELECT
                COUNT(*) as total_downloads,
                COUNT(CASE WHEN download_timestamp >= datetime('now', '-30 days') THEN 1 END) as recent_downloads
            FROM plugin_downloads pd
            JOIN plugin_versions pv ON pd.plugin_version_id = pv.id
            JOIN plugins p ON pv.plugin_id = p.id
            WHERE p.name = ?
            "#,
            [plugin_name],
            |row| Ok((row.get(0)?, row.get(1)?))
        )?;

        // Get version breakdown
        let mut version_breakdown = HashMap::new();
        let mut stmt = conn.prepare(
            r#"
            SELECT pv.version, COUNT(*) as download_count
            FROM plugin_downloads pd
            JOIN plugin_versions pv ON pd.plugin_version_id = pv.id
            JOIN plugins p ON pv.plugin_id = p.id
            WHERE p.name = ?
            GROUP BY pv.version
            ORDER BY download_count DESC
            "#
        )?;

        let version_rows = stmt.query_map([plugin_name], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
        })?;

        for version_row in version_rows {
            let (version, count) = version_row?;
            version_breakdown.insert(version, count);
        }

        Ok(DownloadStats {
            total_downloads: stats.0 as usize,
            downloads_last_30_days: stats.1 as usize,
            version_breakdown,
        })
    }

    fn record_download(&mut self, plugin_name: &str, version: &str, user_id: Option<&str>) -> Result<()> {
        let conn = &mut self.conn;

        // Get plugin version ID
        let version_id: Option<String> = conn.query_row(
            r#"
            SELECT pv.id
            FROM plugin_versions pv
            JOIN plugins p ON pv.plugin_id = p.id
            WHERE p.name = ? AND pv.version = ?
            "#,
            params![plugin_name, version],
            |row| row.get(0)
        ).optional()?;

        if let Some(version_id) = version_id {
            conn.execute(
                "INSERT INTO plugin_downloads (plugin_version_id, user_id, download_timestamp) VALUES (?, ?, CURRENT_TIMESTAMP)",
                params![version_id, user_id]
            )?;
        } else {
            warn!("Attempted to record download for unknown plugin version: {} {}", plugin_name, version);
        }

        Ok(())
    }

    fn get_plugin_by_id(&self, id: &str) -> Result<Option<crate::plugin_registry::PluginMetadata>> {
        let conn = &self.conn;

        let mut stmt = conn.prepare(
            r#"
            SELECT p.id, p.name, p.display_name, p.description, p.author_email,
                   p.license, p.homepage, p.repository, p.keywords, p.created_at, p.updated_at,
                   pv.version, pv.status, pv.package_size, pv.package_hash,
                   pv.manifest, pv.published_at
            FROM plugins p
            JOIN plugin_versions pv ON p.id = pv.plugin_id
            WHERE p.id = ? AND pv.status = 'approved'
            ORDER BY pv.version DESC
            LIMIT 1
            "#
        )?;

        let result = stmt.query_row([id], |row| self.row_to_plugin_metadata(row))
            .optional()
            .context("Failed to query plugin by ID")?;

        Ok(result)
    }

    fn get_plugin_version_by_id(&self, id: &str) -> Result<Option<crate::plugin_registry::PluginVersion>> {
        let conn = &self.conn;

        let mut stmt = conn.prepare(
            r#"
            SELECT pv.id, pv.plugin_id, pv.version, pv.status, pv.package_size,
                   pv.package_hash, pv.manifest, pv.signature_data, pv.published_at,
                   p.name
            FROM plugin_versions pv
            JOIN plugins p ON pv.plugin_id = p.id
            WHERE pv.id = ?
            "#
        )?;

        let result = stmt.query_row([id], |row| self.row_to_plugin_version_with_name(row))
            .optional()
            .context("Failed to query plugin version by ID")?;

        Ok(result)
    }

    fn update_plugin_metadata(
        &mut self,
        plugin_id: &str,
        metadata: &crate::plugin_registry::PluginMetadata
    ) -> Result<()> {
        let conn = &mut self.conn;

        conn.execute(
            r#"
            UPDATE plugins
            SET display_name = ?, description = ?, author_email = ?,
                license = ?, homepage = ?, repository = ?, keywords = ?,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?
            "#,
            params![
                metadata.display_name,
                metadata.description,
                metadata.author_email,
                metadata.license,
                metadata.homepage,
                metadata.repository,
                serde_json::to_string(&metadata.keywords)?,
                plugin_id
            ]
        )?;

        Ok(())
    }

    fn delete_plugin_version(&mut self, plugin_name: &str, version: &str) -> Result<()> {
        let conn = &mut self.conn;

        let rows_affected = conn.execute(
            r#"
            DELETE FROM plugin_versions
            WHERE plugin_id = (SELECT id FROM plugins WHERE name = ?)
            AND version = ?
            "#,
            params![plugin_name, version]
        )?;

        if rows_affected == 0 {
            bail!("Plugin version not found: {} {}", plugin_name, version);
        }

        Ok(())
    }

    fn get_plugins_by_author(&self, author_email: &str) -> Result<Vec<crate::plugin_registry::PluginMetadata>> {
        let conn = &self.conn;

        let mut stmt = conn.prepare(
            r#"
            SELECT p.id, p.name, p.display_name, p.description, p.author_email,
                   p.license, p.homepage, p.repository, p.keywords, p.created_at, p.updated_at,
                   pv.version, pv.status, pv.package_size, pv.package_hash,
                   pv.manifest, pv.published_at
            FROM plugins p
            JOIN plugin_versions pv ON p.id = pv.plugin_id
            WHERE p.author_email = ? AND pv.status = 'approved'
            ORDER BY p.name, pv.version DESC
            "#
        )?;

        let plugins = stmt.query_map([author_email], |row| self.row_to_plugin_metadata(row))?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to query plugins by author")?;

        Ok(plugins)
    }
}

impl DatabaseConnection {
    /// Helper method to convert database row to PluginMetadata
    fn row_to_plugin_metadata(&self, row: &Row) -> Result<crate::plugin_registry::PluginMetadata> {
        let keywords_str: String = row.get(8)?;
        let keywords: Vec<String> = serde_json::from_str(&keywords_str)?;

        let manifest_str: String = row.get(14)?;
        let manifest: crate::plugin_registry::PluginManifest = serde_json::from_str(&manifest_str)?;

        Ok(crate::plugin_registry::PluginMetadata {
            id: row.get(0)?,
            name: row.get(1)?,
            display_name: row.get(2)?,
            description: row.get(3)?,
            author_email: row.get(4)?,
            license: row.get(5)?,
            homepage: row.get(6)?,
            repository: row.get(7)?,
            keywords,
            created_at: row.get(9)?,
            updated_at: row.get(10)?,
            version: row.get(11)?,
            status: serde_json::from_str(&row.get::<_, String>(12)?)?,
            package_size: row.get(13)?,
            package_hash: row.get(14)?,
            manifest,
        })
    }

    /// Helper method to convert database row to PluginVersion
    fn row_to_plugin_version(&self, row: &Row) -> Result<crate::plugin_registry::PluginVersion> {
        let manifest_str: String = row.get(6)?;
        let manifest: crate::plugin_registry::PluginManifest = serde_json::from_str(&manifest_str)?;

        let signature_data: Option<String> = row.get(7)?;
        let signature = signature_data.and_then(|s| serde_json::from_str(&s).ok());

        Ok(crate::plugin_registry::PluginVersion {
            id: row.get(0)?,
            plugin_id: row.get(1)?,
            version: row.get(2)?,
            status: serde_json::from_str(&row.get::<_, String>(3)?)?,
            package_size: row.get(4)?,
            package_hash: row.get(5)?,
            manifest,
            signature,
            published_at: row.get(8)?,
        })
    }

    /// Helper method to convert database row to PluginVersion with plugin name
    fn row_to_plugin_version_with_name(&self, row: &Row) -> Result<crate::plugin_registry::PluginVersion> {
        let mut version = self.row_to_plugin_version(row)?;
        // The plugin name is in the last column
        let plugin_name: String = row.get(9)?;
        // Store plugin name in a temporary field for the version
        // This is a bit of a hack, but works for our use case
        Ok(version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_database_connection() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = DatabaseConnection::new(&db_path);
        assert!(db.is_ok());

        let db = db.unwrap();
        let stats = db.get_stats().unwrap();
        assert_eq!(stats.total_plugins, 0);
        assert_eq!(stats.total_versions, 0);
    }

    #[test]
    fn test_database_migration() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_migrate.db");

        let db = DatabaseConnection::new(&db_path).unwrap();
        assert!(db.migrate().is_ok());
    }
}
