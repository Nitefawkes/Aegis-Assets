-- Plugin Registry Database Schema - Initial Migration
-- This migration creates all the core tables for the plugin registry system

-- Core plugin metadata table
CREATE TABLE plugins (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) UNIQUE NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    description TEXT,
    author_email VARCHAR(255) NOT NULL,
    license VARCHAR(50) NOT NULL,
    homepage TEXT,
    repository TEXT,
    keywords TEXT[], -- Array of keywords for search
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Plugin versions table
CREATE TABLE plugin_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plugin_id UUID REFERENCES plugins(id) ON DELETE CASCADE,
    version VARCHAR(50) NOT NULL,
    status VARCHAR(50) DEFAULT 'pending_review', -- pending_review, approved, rejected, deprecated
    package_size BIGINT NOT NULL, -- Package size in bytes
    package_hash CHAR(64) NOT NULL, -- Blake3 hash of package
    manifest JSONB NOT NULL, -- Full plugin.toml content
    signature_data JSONB, -- Code signature information
    published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(plugin_id, version)
);

-- Plugin dependencies table
CREATE TABLE plugin_dependencies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plugin_version_id UUID REFERENCES plugin_versions(id) ON DELETE CASCADE,
    dependency_name VARCHAR(255) NOT NULL,
    version_requirement VARCHAR(100) NOT NULL, -- Semver constraint like "^1.0.0"
    dependency_type VARCHAR(50) DEFAULT 'runtime', -- runtime, dev, build
    optional BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Supported file formats table
CREATE TABLE plugin_formats (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plugin_version_id UUID REFERENCES plugin_versions(id) ON DELETE CASCADE,
    file_extension VARCHAR(20) NOT NULL,
    format_description TEXT,
    mime_type VARCHAR(100),
    tested BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Download statistics table
CREATE TABLE plugin_downloads (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plugin_version_id UUID REFERENCES plugin_versions(id) ON DELETE CASCADE,
    user_id VARCHAR(255), -- Optional user tracking
    download_timestamp TIMESTAMPTZ DEFAULT NOW(),
    ip_address INET,
    user_agent TEXT,
    success BOOLEAN DEFAULT TRUE
);

-- Plugin reviews and ratings table
CREATE TABLE plugin_reviews (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plugin_id UUID REFERENCES plugins(id) ON DELETE CASCADE,
    user_id VARCHAR(255) NOT NULL,
    rating INTEGER CHECK (rating >= 1 AND rating <= 5),
    review_text TEXT,
    version_used VARCHAR(50),
    helpful_count INTEGER DEFAULT 0,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(plugin_id, user_id) -- One review per user per plugin
);

-- Plugin security scans table
CREATE TABLE plugin_security_scans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plugin_version_id UUID REFERENCES plugin_versions(id) ON DELETE CASCADE,
    scan_timestamp TIMESTAMPTZ DEFAULT NOW(),
    scan_status VARCHAR(50), -- passed, failed, pending
    vulnerability_count INTEGER DEFAULT 0,
    scan_report JSONB, -- Detailed scan results
    scanner_version VARCHAR(50),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Plugin package storage table (separate from metadata for performance)
CREATE TABLE plugin_packages (
    version_id UUID PRIMARY KEY REFERENCES plugin_versions(id) ON DELETE CASCADE,
    package_data BLOB NOT NULL, -- Actual plugin package file
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Audit log table for tracking all changes
CREATE TABLE plugin_audit_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    table_name VARCHAR(100) NOT NULL,
    record_id UUID NOT NULL,
    operation VARCHAR(50) NOT NULL, -- INSERT, UPDATE, DELETE
    old_values JSONB,
    new_values JSONB,
    changed_by VARCHAR(255),
    change_timestamp TIMESTAMPTZ DEFAULT NOW(),
    ip_address INET,
    user_agent TEXT
);

-- Create indexes for performance
CREATE INDEX idx_plugin_versions_plugin_id ON plugin_versions(plugin_id);
CREATE INDEX idx_plugin_versions_status ON plugin_versions(status);
CREATE INDEX idx_plugin_versions_published_at ON plugin_versions(published_at);

CREATE INDEX idx_plugin_dependencies_version_id ON plugin_dependencies(plugin_version_id);
CREATE INDEX idx_plugin_dependencies_name ON plugin_dependencies(dependency_name);

CREATE INDEX idx_plugin_formats_extension ON plugin_formats(file_extension);
CREATE INDEX idx_plugin_formats_version_id ON plugin_formats(plugin_version_id);

CREATE INDEX idx_plugin_downloads_version_id ON plugin_downloads(plugin_version_id);
CREATE INDEX idx_plugin_downloads_timestamp ON plugin_downloads(download_timestamp);

CREATE INDEX idx_plugin_reviews_plugin_id ON plugin_reviews(plugin_id);
CREATE INDEX idx_plugin_reviews_rating ON plugin_reviews(rating);

CREATE INDEX idx_plugin_security_scans_version_id ON plugin_security_scans(plugin_version_id);
CREATE INDEX idx_plugin_security_scans_status ON plugin_security_scans(scan_status);

CREATE INDEX idx_plugin_audit_log_table_record ON plugin_audit_log(table_name, record_id);
CREATE INDEX idx_plugin_audit_log_timestamp ON plugin_audit_log(change_timestamp);

-- Full-text search index for plugins
CREATE INDEX idx_plugins_search ON plugins USING gin(to_tsvector('english',
    name || ' ' || description || ' ' || array_to_string(keywords, ' ')));

-- Version parsing indexes for efficient semver queries
CREATE INDEX idx_plugin_versions_semver ON plugin_versions(
    CAST(SUBSTR(version, 1, INSTR(version || '.', '.') - 1) AS INTEGER),
    CAST(SUBSTR(version, INSTR(version, '.') + 1,
        INSTR(SUBSTR(version, INSTR(version, '.') + 1) || '.') - 1) AS INTEGER)
) WHERE version NOT GLOB '*[a-zA-Z]*';

-- Trigger to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_plugins_updated_at
    BEFORE UPDATE ON plugins
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_plugin_reviews_updated_at
    BEFORE UPDATE ON plugin_reviews
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Trigger for audit logging
CREATE OR REPLACE FUNCTION audit_trigger_function()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        INSERT INTO plugin_audit_log (table_name, record_id, operation, old_values, changed_by)
        VALUES (TG_TABLE_NAME, OLD.id, TG_OP, row_to_json(OLD), 'system');
        RETURN OLD;
    ELSIF TG_OP = 'UPDATE' THEN
        INSERT INTO plugin_audit_log (table_name, record_id, operation, old_values, new_values, changed_by)
        VALUES (TG_TABLE_NAME, NEW.id, TG_OP, row_to_json(OLD), row_to_json(NEW), 'system');
        RETURN NEW;
    ELSIF TG_OP = 'INSERT' THEN
        INSERT INTO plugin_audit_log (table_name, record_id, operation, new_values, changed_by)
        VALUES (TG_TABLE_NAME, NEW.id, TG_OP, row_to_json(NEW), 'system');
        RETURN NEW;
    END IF;
    RETURN NULL;
END;
$$ language 'plpgsql';

-- Enable audit logging for key tables
CREATE TRIGGER audit_plugins_trigger
    AFTER INSERT OR UPDATE OR DELETE ON plugins
    FOR EACH ROW EXECUTE FUNCTION audit_trigger_function();

CREATE TRIGGER audit_plugin_versions_trigger
    AFTER INSERT OR UPDATE OR DELETE ON plugin_versions
    FOR EACH ROW EXECUTE FUNCTION audit_trigger_function();

CREATE TRIGGER audit_plugin_downloads_trigger
    AFTER INSERT ON plugin_downloads
    FOR EACH ROW EXECUTE FUNCTION audit_trigger_function();
