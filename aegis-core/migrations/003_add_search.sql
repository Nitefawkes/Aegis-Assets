-- Add full-text search capabilities and search optimization
-- This migration enhances search performance and adds advanced search features

-- Create virtual FTS5 table for plugin search
CREATE VIRTUAL TABLE IF NOT EXISTS plugins_fts USING fts5(
    name, display_name, description, keywords, author_email,
    content='plugins',
    content_rowid='id',
    tokenize = 'porter ascii'
);

-- Create virtual FTS5 table for plugin versions (for searching within versions)
CREATE VIRTUAL TABLE IF NOT EXISTS plugin_versions_fts USING fts5(
    version, manifest, plugin_name UNINDEXED,
    content='plugin_versions',
    content_rowid='id',
    tokenize = 'porter ascii'
);

-- Populate FTS tables with existing data
INSERT OR IGNORE INTO plugins_fts(rowid, name, display_name, description, keywords, author_email)
SELECT id, name, display_name, description, array_to_string(keywords, ' '), author_email
FROM plugins;

INSERT OR IGNORE INTO plugin_versions_fts(rowid, version, manifest, plugin_name)
SELECT pv.id, pv.version, pv.manifest, p.name
FROM plugin_versions pv
JOIN plugins p ON pv.plugin_id = p.id;

-- Create triggers to keep FTS tables in sync with main tables

-- Triggers for plugins table
CREATE TRIGGER IF NOT EXISTS plugins_fts_insert AFTER INSERT ON plugins BEGIN
    INSERT INTO plugins_fts(rowid, name, display_name, description, keywords, author_email)
    VALUES (new.id, new.name, new.display_name, new.description, new.keywords, new.author_email);
END;

CREATE TRIGGER IF NOT EXISTS plugins_fts_delete AFTER DELETE ON plugins BEGIN
    INSERT INTO plugins_fts(plugins_fts, rowid, name, display_name, description, keywords, author_email)
    VALUES('delete', old.id, old.name, old.display_name, old.description, old.keywords, old.author_email);
END;

CREATE TRIGGER IF NOT EXISTS plugins_fts_update AFTER UPDATE ON plugins BEGIN
    INSERT INTO plugins_fts(plugins_fts, rowid, name, display_name, description, keywords, author_email)
    VALUES('delete', old.id, old.name, old.display_name, old.description, old.keywords, old.author_email);
    INSERT INTO plugins_fts(rowid, name, display_name, description, keywords, author_email)
    VALUES (new.id, new.name, new.display_name, new.description, new.keywords, new.author_email);
END;

-- Triggers for plugin_versions table
CREATE TRIGGER IF NOT EXISTS plugin_versions_fts_insert AFTER INSERT ON plugin_versions BEGIN
    INSERT INTO plugin_versions_fts(rowid, version, manifest, plugin_name)
    SELECT new.id, new.version, new.manifest, p.name
    FROM plugins p WHERE p.id = new.plugin_id;
END;

CREATE TRIGGER IF NOT EXISTS plugin_versions_fts_delete AFTER DELETE ON plugin_versions BEGIN
    INSERT INTO plugin_versions_fts(plugin_versions_fts, rowid, version, manifest, plugin_name)
    VALUES('delete', old.id, old.version, old.manifest, 'deleted');
END;

CREATE TRIGGER IF NOT EXISTS plugin_versions_fts_update AFTER UPDATE ON plugin_versions BEGIN
    INSERT INTO plugin_versions_fts(plugin_versions_fts, rowid, version, manifest, plugin_name)
    VALUES('delete', old.id, old.version, old.manifest, 'deleted');
    INSERT INTO plugin_versions_fts(rowid, version, manifest, plugin_name)
    SELECT new.id, new.version, new.manifest, p.name
    FROM plugins p WHERE p.id = new.plugin_id;
END;

-- Create additional search helper functions

-- Function to search plugins with relevance scoring
CREATE OR REPLACE FUNCTION search_plugins_fts(query TEXT, limit_val INTEGER DEFAULT 50)
RETURNS TABLE (
    id TEXT,
    name TEXT,
    display_name TEXT,
    description TEXT,
    author_email TEXT,
    keywords TEXT,
    relevance_score REAL,
    match_info TEXT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        p.id,
        p.name,
        p.display_name,
        p.description,
        p.author_email,
        array_to_string(p.keywords, ', '),
        bm25(plugins_fts, 0, 1, 2, 3, 4) as relevance_score,
        matchinfo(plugins_fts, 'pcsx') as match_info
    FROM plugins_fts
    JOIN plugins p ON plugins_fts.rowid = p.id
    WHERE plugins_fts MATCH query
    ORDER BY bm25(plugins_fts, 0, 1, 2, 3, 4) DESC
    LIMIT limit_val;
END;
$$ LANGUAGE plpgsql;

-- Function to search plugin versions with relevance scoring
CREATE OR REPLACE FUNCTION search_plugin_versions_fts(query TEXT, limit_val INTEGER DEFAULT 20)
RETURNS TABLE (
    id TEXT,
    plugin_id TEXT,
    version TEXT,
    manifest TEXT,
    plugin_name TEXT,
    relevance_score REAL
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        pv.id,
        pv.plugin_id,
        pv.version,
        pv.manifest,
        p.name as plugin_name,
        bm25(plugin_versions_fts, 0, 1) as relevance_score
    FROM plugin_versions_fts
    JOIN plugin_versions pv ON plugin_versions_fts.rowid = pv.id
    JOIN plugins p ON pv.plugin_id = p.id
    WHERE plugin_versions_fts MATCH query
    ORDER BY bm25(plugin_versions_fts, 0, 1) DESC
    LIMIT limit_val;
END;
$$ LANGUAGE plpgsql;

-- Function to get plugin suggestions for autocomplete
CREATE OR REPLACE FUNCTION get_plugin_suggestions(partial_query TEXT, limit_val INTEGER DEFAULT 10)
RETURNS TABLE (
    name TEXT,
    display_name TEXT,
    suggestion_score REAL
) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT
        p.name,
        p.display_name,
        (
            CASE
                WHEN p.name LIKE partial_query || '%' THEN 1.0
                WHEN p.display_name LIKE '%' || partial_query || '%' THEN 0.8
                WHEN p.description LIKE '%' || partial_query || '%' THEN 0.6
                ELSE 0.4
            END
        ) as suggestion_score
    FROM plugins p
    WHERE
        p.name LIKE partial_query || '%' OR
        p.display_name LIKE '%' || partial_query || '%' OR
        p.description LIKE '%' || partial_query || '%'
    ORDER BY suggestion_score DESC, p.name
    LIMIT limit_val;
END;
$$ LANGUAGE plpgsql;

-- Function to search plugins by tags/keywords with exact matching
CREATE OR REPLACE FUNCTION search_plugins_by_keywords(keyword_array TEXT[], limit_val INTEGER DEFAULT 20)
RETURNS TABLE (
    id TEXT,
    name TEXT,
    display_name TEXT,
    description TEXT,
    author_email TEXT,
    keyword_match_count INTEGER
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        p.id,
        p.name,
        p.display_name,
        p.description,
        p.author_email,
        (
            SELECT COUNT(*)
            FROM unnest(p.keywords) AS keyword
            WHERE keyword = ANY(keyword_array)
        ) as keyword_match_count
    FROM plugins p
    WHERE EXISTS (
        SELECT 1
        FROM unnest(p.keywords) AS keyword
        WHERE keyword = ANY(keyword_array)
    )
    ORDER BY keyword_match_count DESC, p.name
    LIMIT limit_val;
END;
$$ LANGUAGE plpgsql;

-- Create views for common search patterns

-- View for approved plugins with search metadata
CREATE OR REPLACE VIEW approved_plugins_search AS
SELECT
    p.id,
    p.name,
    p.display_name,
    p.description,
    p.author_email,
    p.license,
    p.keywords,
    p.created_at,
    p.updated_at,
    pv.version,
    pv.status,
    pv.package_size,
    pv.published_at,
    (
        SELECT COUNT(*)
        FROM plugin_downloads pd
        WHERE pd.plugin_version_id = pv.id
    ) as download_count,
    (
        SELECT AVG(rating)
        FROM plugin_reviews pr
        WHERE pr.plugin_id = p.id
    ) as average_rating,
    (
        SELECT COUNT(*)
        FROM plugin_reviews pr
        WHERE pr.plugin_id = p.id
    ) as review_count
FROM plugins p
JOIN plugin_versions pv ON p.id = pv.plugin_id
WHERE pv.status = 'approved';

-- View for plugin statistics
CREATE OR REPLACE VIEW plugin_statistics AS
SELECT
    p.name,
    p.display_name,
    COUNT(DISTINCT pv.id) as version_count,
    MAX(pv.published_at) as latest_update,
    COUNT(pd.id) as total_downloads,
    COUNT(CASE WHEN pd.download_timestamp >= datetime('now', '-30 days') THEN 1 END) as recent_downloads,
    AVG(pr.rating) as average_rating,
    COUNT(pr.id) as review_count,
    p.created_at as first_published
FROM plugins p
LEFT JOIN plugin_versions pv ON p.id = pv.plugin_id
LEFT JOIN plugin_downloads pd ON pv.id = pd.plugin_version_id
LEFT JOIN plugin_reviews pr ON p.id = pr.plugin_id
GROUP BY p.id, p.name, p.display_name;

-- Create materialized view for popular plugins (refreshed periodically)
CREATE MATERIALIZED VIEW IF NOT EXISTS popular_plugins AS
SELECT
    p.id,
    p.name,
    p.display_name,
    p.description,
    COUNT(pd.id) as download_count,
    AVG(pr.rating) as average_rating,
    COUNT(pr.id) as review_count,
    MAX(pv.published_at) as latest_update
FROM plugins p
JOIN plugin_versions pv ON p.id = pv.plugin_id AND pv.status = 'approved'
LEFT JOIN plugin_downloads pd ON pv.id = pd.plugin_version_id
LEFT JOIN plugin_reviews pr ON p.id = pr.plugin_id
GROUP BY p.id, p.name, p.display_name, p.description
HAVING COUNT(pd.id) > 0
ORDER BY COUNT(pd.id) DESC, AVG(pr.rating) DESC NULLS LAST;

-- Create unique index on materialized view
CREATE UNIQUE INDEX IF NOT EXISTS idx_popular_plugins_id ON popular_plugins(id);

-- Refresh function for materialized view
CREATE OR REPLACE FUNCTION refresh_popular_plugins()
RETURNS VOID AS $$
BEGIN
    REFRESH MATERIALIZED VIEW popular_plugins;
END;
$$ LANGUAGE plpgsql;
