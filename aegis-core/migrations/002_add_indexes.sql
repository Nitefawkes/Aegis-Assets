-- Add performance indexes for plugin registry
-- This migration optimizes query performance for common operations

-- Plugin version lookup indexes
CREATE INDEX IF NOT EXISTS idx_plugin_versions_semver_components
ON plugin_versions (
    CAST(SUBSTR(version, 1, INSTR(version || '.', '.') - 1) AS INTEGER),
    CAST(SUBSTR(SUBSTR(version, INSTR(version, '.') + 1),
        1, INSTR(SUBSTR(version, INSTR(version, '.') + 1) || '.') - 1) AS INTEGER)
) WHERE version NOT GLOB '*[a-zA-Z]*';

-- Full-text search optimization
CREATE INDEX IF NOT EXISTS idx_plugins_search_optimized
ON plugins USING gin(to_tsvector('english',
    COALESCE(name, '') || ' ' ||
    COALESCE(description, '') || ' ' ||
    COALESCE(array_to_string(keywords, ' '), '')
));

-- Plugin discovery indexes
CREATE INDEX IF NOT EXISTS idx_plugins_by_author ON plugins(author_email);
CREATE INDEX IF NOT EXISTS idx_plugins_by_license ON plugins(license);
CREATE INDEX IF NOT EXISTS idx_plugins_created_at ON plugins(created_at);
CREATE INDEX IF NOT EXISTS idx_plugins_updated_at ON plugins(updated_at);

-- Version management indexes
CREATE INDEX IF NOT EXISTS idx_plugin_versions_by_status ON plugin_versions(status);
CREATE INDEX IF NOT EXISTS idx_plugin_versions_published_at ON plugin_versions(published_at);
CREATE INDEX IF NOT EXISTS idx_plugin_versions_package_size ON plugin_versions(package_size);

-- Dependency management indexes
CREATE INDEX IF NOT EXISTS idx_dependencies_by_plugin ON plugin_dependencies(plugin_version_id);
CREATE INDEX IF NOT EXISTS idx_dependencies_by_name ON plugin_dependencies(dependency_name);
CREATE INDEX IF NOT EXISTS idx_dependencies_by_type ON plugin_dependencies(dependency_type);

-- Format support indexes
CREATE INDEX IF NOT EXISTS idx_formats_by_extension ON plugin_formats(file_extension);
CREATE INDEX IF NOT EXISTS idx_formats_by_plugin ON plugin_formats(plugin_version_id);

-- Download tracking indexes
CREATE INDEX IF NOT EXISTS idx_downloads_by_plugin ON plugin_downloads(plugin_version_id);
CREATE INDEX IF NOT EXISTS idx_downloads_by_user ON plugin_downloads(user_id);
CREATE INDEX IF NOT EXISTS idx_downloads_timestamp ON plugin_downloads(download_timestamp);
CREATE INDEX IF NOT EXISTS idx_downloads_success ON plugin_downloads(success);

-- Review system indexes
CREATE INDEX IF NOT EXISTS idx_reviews_by_plugin ON plugin_reviews(plugin_id);
CREATE INDEX IF NOT EXISTS idx_reviews_by_user ON plugin_reviews(user_id);
CREATE INDEX IF NOT EXISTS idx_reviews_rating ON plugin_reviews(rating);
CREATE INDEX IF NOT EXISTS idx_reviews_created_at ON plugin_reviews(created_at);

-- Security scan indexes
CREATE INDEX IF NOT EXISTS idx_security_scans_by_plugin ON plugin_security_scans(plugin_version_id);
CREATE INDEX IF NOT EXISTS idx_security_scans_by_status ON plugin_security_scans(scan_status);
CREATE INDEX IF NOT EXISTS idx_security_scans_timestamp ON plugin_security_scans(scan_timestamp);

-- Audit log indexes
CREATE INDEX IF NOT EXISTS idx_audit_by_table ON plugin_audit_log(table_name, record_id);
CREATE INDEX IF NOT EXISTS idx_audit_by_user ON plugin_audit_log(changed_by);
CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON plugin_audit_log(change_timestamp);

-- Compound indexes for common queries
CREATE INDEX IF NOT EXISTS idx_plugins_versions_status
ON plugin_versions(plugin_id, status)
WHERE status = 'approved';

CREATE INDEX IF NOT EXISTS idx_downloads_plugin_recent
ON plugin_downloads(plugin_version_id, download_timestamp)
WHERE download_timestamp >= datetime('now', '-30 days');

-- Partial indexes for common filters
CREATE INDEX IF NOT EXISTS idx_plugins_approved_only
ON plugins(id, name, updated_at)
WHERE id IN (
    SELECT DISTINCT plugin_id
    FROM plugin_versions
    WHERE status = 'approved'
);

CREATE INDEX IF NOT EXISTS idx_versions_latest_per_plugin
ON plugin_versions(plugin_id, published_at, version)
WHERE status = 'approved';
