CREATE TABLE IF NOT EXISTS emails (
    id TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL,
    from_addr TEXT NOT NULL,
    to_addr TEXT NOT NULL,
    subject TEXT,
    body_text TEXT,
    body_html TEXT,
    extracted_code TEXT,
    extracted_link TEXT,
    extracted_text TEXT,
    is_archived BOOLEAN DEFAULT FALSE
);

CREATE TABLE IF NOT EXISTS webhooks (
    id TEXT PRIMARY KEY,
    url TEXT NOT NULL,
    event_filter TEXT DEFAULT '*',
    is_active BOOLEAN DEFAULT TRUE,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS workflow_runs (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    workflow_title TEXT NOT NULL,
    status TEXT NOT NULL,
    message TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    finished_at INTEGER
);

CREATE TABLE IF NOT EXISTS workflow_definitions (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL DEFAULT 'account_generate',
    title TEXT NOT NULL,
    summary TEXT NOT NULL,
    status TEXT NOT NULL,
    parameters_json TEXT NOT NULL DEFAULT '{}',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS workflow_run_steps (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    step_index INTEGER NOT NULL,
    level TEXT NOT NULL,
    message TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS generated_accounts (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    address TEXT NOT NULL,
    password TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    access_token TEXT,
    refresh_token TEXT,
    session_token TEXT,
    device_id TEXT,
    workspace_id TEXT,
    upload_status TEXT DEFAULT 'pending',
    account_type TEXT,
    proxy_url TEXT
);

CREATE INDEX IF NOT EXISTS idx_to_addr ON emails (to_addr);
CREATE INDEX IF NOT EXISTS idx_emails_created_at ON emails (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_emails_archived_created_at ON emails (is_archived, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_workflow_runs_started_at ON workflow_runs (started_at DESC);
CREATE INDEX IF NOT EXISTS idx_workflow_definitions_updated_at ON workflow_definitions (updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_workflow_run_steps_run_id ON workflow_run_steps (run_id, step_index);
CREATE INDEX IF NOT EXISTS idx_generated_accounts_run_id ON generated_accounts (run_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_generated_accounts_created_at ON generated_accounts (created_at DESC);
