-- Partitioning and cleanup for large-scale audit data

-- Create monthly partitions for security events (if supported)
-- Note: SQLite doesn't support true partitioning, but we can create separate tables
-- and use views for unified access

-- Cleanup old audit data procedure (implemented via triggers)
CREATE TRIGGER audit_cleanup_trigger
AFTER INSERT ON security_events
BEGIN
  DELETE FROM security_events 
  WHERE timestamp < strftime('%s', 'now', '-365 days');
  
  DELETE FROM security_alerts 
  WHERE created_at < strftime('%s', 'now', '-365 days');
  
  DELETE FROM event_relationships 
  WHERE created_at < strftime('%s', 'now', '-365 days');
END;

-- Audit data retention settings
CREATE TABLE audit_retention_settings (
    setting_name TEXT PRIMARY KEY,
    setting_value TEXT NOT NULL,
    updated_at INTEGER DEFAULT (strftime('%s', 'now')),
    updated_by TEXT
);

INSERT INTO audit_retention_settings (setting_name, setting_value, updated_by) VALUES
('security_events_retention_days', '365', 'system'),
('security_alerts_retention_days', '365', 'system'),
('compliance_violations_retention_days', '2555', 'system'), -- 7 years for compliance
('plugin_assessments_retention_days', '1095', 'system'), -- 3 years
('audit_export_retention_days', '90', 'system');

-- Archive old data views
CREATE VIEW archived_security_events AS
SELECT * FROM security_events 
WHERE timestamp < strftime('%s', 'now', '-30 days');

CREATE VIEW recent_security_events AS
SELECT * FROM security_events 
WHERE timestamp >= strftime('%s', 'now', '-30 days');

-- High-risk events view for monitoring
CREATE VIEW high_risk_events AS
SELECT 
    se.*,
    sa.title as alert_title,
    sa.acknowledged,
    sa.resolved
FROM security_events se
LEFT JOIN security_alerts sa ON se.id = sa.event_id
WHERE se.risk_score > 70 
   OR se.severity IN ('High', 'Critical')
   OR se.remediation_required = 1
ORDER BY se.timestamp DESC;

-- Compliance dashboard view
CREATE VIEW compliance_dashboard AS
SELECT 
    date(timestamp, 'unixepoch') as event_date,
    COUNT(*) as total_events,
    SUM(CASE WHEN event_type = 'ComplianceViolation' THEN 1 ELSE 0 END) as violations,
    SUM(CASE WHEN severity = 'Critical' THEN 1 ELSE 0 END) as critical_events,
    SUM(CASE WHEN severity = 'High' THEN 1 ELSE 0 END) as high_events,
    SUM(CASE WHEN remediation_required = 1 THEN 1 ELSE 0 END) as remediation_needed
FROM security_events
WHERE timestamp >= strftime('%s', 'now', '-30 days')
GROUP BY date(timestamp, 'unixepoch')
ORDER BY event_date DESC;