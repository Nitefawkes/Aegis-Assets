-- Initial audit tables schema

-- Security events table
CREATE TABLE security_events (
    id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    severity TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    plugin_path TEXT,
    plugin_hash TEXT,
    publisher_id TEXT,
    user_id TEXT,
    session_id TEXT,
    source_ip TEXT,
    user_agent TEXT,
    details TEXT NOT NULL, -- JSON
    risk_score INTEGER,
    compliance_status TEXT,
    remediation_required BOOLEAN NOT NULL DEFAULT FALSE,
    created_at INTEGER DEFAULT (strftime('%s', 'now'))
);

-- Security alerts table
CREATE TABLE security_alerts (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,
    alert_type TEXT NOT NULL,
    severity TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    acknowledged BOOLEAN NOT NULL DEFAULT FALSE,
    acknowledged_by TEXT,
    acknowledged_at INTEGER,
    resolved BOOLEAN NOT NULL DEFAULT FALSE,
    resolved_by TEXT,
    resolved_at INTEGER,
    FOREIGN KEY (event_id) REFERENCES security_events(id)
);

-- Event relationships for correlation
CREATE TABLE event_relationships (
    event_id TEXT NOT NULL,
    related_event_id TEXT NOT NULL,
    relationship_type TEXT DEFAULT 'related',
    created_at INTEGER DEFAULT (strftime('%s', 'now')),
    PRIMARY KEY (event_id, related_event_id),
    FOREIGN KEY (event_id) REFERENCES security_events(id),
    FOREIGN KEY (related_event_id) REFERENCES security_events(id)
);

-- Audit trail queries for compliance
CREATE TABLE audit_queries (
    id TEXT PRIMARY KEY,
    query_name TEXT NOT NULL,
    query_parameters TEXT NOT NULL, -- JSON
    generated_by TEXT,
    generated_at INTEGER NOT NULL,
    result_count INTEGER,
    export_format TEXT
);

-- Plugin risk assessments
CREATE TABLE plugin_risk_assessments (
    id TEXT PRIMARY KEY,
    plugin_path TEXT NOT NULL,
    plugin_hash TEXT NOT NULL,
    assessment_date INTEGER NOT NULL,
    overall_score INTEGER NOT NULL,
    signature_score INTEGER,
    static_analysis_score INTEGER,
    threat_score INTEGER,
    compliance_score INTEGER,
    policy_score INTEGER,
    findings_count INTEGER,
    high_severity_findings INTEGER,
    assessment_data TEXT, -- JSON
    assessed_by TEXT
);

-- Publisher trust tracking
CREATE TABLE publisher_trust_history (
    id TEXT PRIMARY KEY,
    publisher_id TEXT NOT NULL,
    trust_level TEXT NOT NULL,
    previous_trust_level TEXT,
    changed_at INTEGER NOT NULL,
    changed_by TEXT NOT NULL,
    reason TEXT NOT NULL,
    evidence TEXT -- JSON
);

-- Compliance policy violations
CREATE TABLE compliance_violations (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,
    policy_name TEXT NOT NULL,
    policy_version TEXT NOT NULL,
    violation_type TEXT NOT NULL,
    severity TEXT NOT NULL,
    plugin_path TEXT,
    publisher_id TEXT,
    user_id TEXT,
    description TEXT NOT NULL,
    remediation_plan TEXT,
    resolved BOOLEAN NOT NULL DEFAULT FALSE,
    resolved_at INTEGER,
    resolved_by TEXT,
    FOREIGN KEY (event_id) REFERENCES security_events(id)
);