-- Performance indexes for audit queries

-- Security events indexes
CREATE INDEX idx_security_events_timestamp ON security_events(timestamp);
CREATE INDEX idx_security_events_event_type ON security_events(event_type);
CREATE INDEX idx_security_events_severity ON security_events(severity);
CREATE INDEX idx_security_events_plugin_path ON security_events(plugin_path);
CREATE INDEX idx_security_events_publisher_id ON security_events(publisher_id);
CREATE INDEX idx_security_events_user_id ON security_events(user_id);
CREATE INDEX idx_security_events_session_id ON security_events(session_id);
CREATE INDEX idx_security_events_risk_score ON security_events(risk_score);
CREATE INDEX idx_security_events_remediation ON security_events(remediation_required);

-- Composite indexes for common queries
CREATE INDEX idx_security_events_type_severity ON security_events(event_type, severity);
CREATE INDEX idx_security_events_timestamp_type ON security_events(timestamp, event_type);
CREATE INDEX idx_security_events_publisher_timestamp ON security_events(publisher_id, timestamp);

-- Security alerts indexes
CREATE INDEX idx_security_alerts_event_id ON security_alerts(event_id);
CREATE INDEX idx_security_alerts_alert_type ON security_alerts(alert_type);
CREATE INDEX idx_security_alerts_severity ON security_alerts(severity);
CREATE INDEX idx_security_alerts_created_at ON security_alerts(created_at);
CREATE INDEX idx_security_alerts_acknowledged ON security_alerts(acknowledged);
CREATE INDEX idx_security_alerts_resolved ON security_alerts(resolved);

-- Event relationships indexes
CREATE INDEX idx_event_relationships_event_id ON event_relationships(event_id);
CREATE INDEX idx_event_relationships_related_id ON event_relationships(related_event_id);
CREATE INDEX idx_event_relationships_type ON event_relationships(relationship_type);

-- Plugin risk assessments indexes
CREATE INDEX idx_plugin_risk_plugin_path ON plugin_risk_assessments(plugin_path);
CREATE INDEX idx_plugin_risk_plugin_hash ON plugin_risk_assessments(plugin_hash);
CREATE INDEX idx_plugin_risk_assessment_date ON plugin_risk_assessments(assessment_date);
CREATE INDEX idx_plugin_risk_overall_score ON plugin_risk_assessments(overall_score);
CREATE INDEX idx_plugin_risk_high_severity ON plugin_risk_assessments(high_severity_findings);

-- Publisher trust history indexes
CREATE INDEX idx_publisher_trust_publisher_id ON publisher_trust_history(publisher_id);
CREATE INDEX idx_publisher_trust_changed_at ON publisher_trust_history(changed_at);
CREATE INDEX idx_publisher_trust_level ON publisher_trust_history(trust_level);

-- Compliance violations indexes
CREATE INDEX idx_compliance_violations_event_id ON compliance_violations(event_id);
CREATE INDEX idx_compliance_violations_policy ON compliance_violations(policy_name);
CREATE INDEX idx_compliance_violations_severity ON compliance_violations(severity);
CREATE INDEX idx_compliance_violations_publisher ON compliance_violations(publisher_id);
CREATE INDEX idx_compliance_violations_resolved ON compliance_violations(resolved);