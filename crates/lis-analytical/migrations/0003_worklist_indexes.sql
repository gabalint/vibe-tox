-- Narrow the analytical worklist to material without an active execution
-- before its owner gates and catalog rules are evaluated (REQ-OPS-004, NFR-05).

CREATE INDEX ana_active_executions_method_container
    ON ana_executions (method_version_id, container_id)
    WHERE state = 'active';
