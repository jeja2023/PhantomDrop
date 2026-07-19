CREATE INDEX IF NOT EXISTS idx_generated_accounts_routing
ON generated_accounts (
    pool_tag,
    rate_limit_reset_at,
    last_used_at,
    created_at DESC
);

CREATE INDEX IF NOT EXISTS idx_emails_to_created_at
ON emails (to_addr, created_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS idx_workflow_run_steps_run_step
ON workflow_run_steps (run_id, step_index);

CREATE TABLE IF NOT EXISTS webhook_outbox (
    id TEXT PRIMARY KEY,
    webhook_url TEXT NOT NULL,
    payload TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at INTEGER NOT NULL,
    last_error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_webhook_outbox_due
ON webhook_outbox (status, next_attempt_at, created_at);