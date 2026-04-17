export type LogLevel = 'info' | 'warn' | 'success' | 'error'
export type LogSource = 'system_log' | 'workflow_step' | 'ui'

export type AppTab = 'dashboard' | 'emails' | 'logs' | 'tunnel' | 'auto' | 'config' | 'register' | 'accounts'

export interface AppLog {
  id: string
  time: string
  content: string
  type: LogLevel
  source: LogSource
  groupLabel?: string
}

export interface EmailRecordApi {
  id: string
  created_at: number
  from_addr: string
  to_addr: string
  subject: string | null
  extracted_code: string | null
  extracted_link: string | null
  extracted_text: string | null
  is_archived: boolean
}

export interface EmailDetailApi extends EmailRecordApi {
  body_text: string | null
  body_html: string | null
}

export interface EmailItem {
  id: string
  from: string
  to: string
  subject: string
  time: string
  code: string
  link?: string
  isArchived?: boolean
}

export interface StreamEmailPayload {
  id: string
  from: string
  to: string
  subject: string
  code?: string | null
  link?: string | null
  custom_text?: string | null
}

export interface SystemLogPayload {
  level: 'info' | 'warn' | 'success'
  msg: string
}

export interface TunnelStatus {
  active: boolean
  url: string | null
  port: number
  subdomain: string | null
  provider?: string
}

export interface SettingsPayload {
  webhook_url?: string | null
  update_rate?: number | null
  auth_secret?: string | null
  decode_depth?: string | null
  public_hub_url?: string | null
  account_domain?: string | null
  cloudflare_default_mode?: 'local_trycloudflare' | 'public_ip' | 'public_domain' | null
  cloudflare_public_url?: string | null
  cloudflare_route_local_part?: string | null
  cloudflare_zone_domain?: string | null
  cloudflare_api_token?: string | null
  cloudflare_zone_id?: string | null
  cloudflare_account_id?: string | null
  cpa_url?: string | null
  cpa_key?: string | null
}

export interface CloudflareAutomationStatus {
  running: boolean
  current_step?: string | null
  last_started_at?: number | null
  last_finished_at?: number | null
  last_success?: boolean | null
  last_mode?: string | null
  last_public_url?: string | null
  summary?: Record<string, unknown> | null
  stdout?: string | null
  stderr?: string | null
  error?: string | null
  logs?: Array<{ level: string; message: string }>
  worker_url?: string | null
  email_address?: string | null
}

export type WorkflowStatus = 'ready' | 'active' | 'idle'
export type WorkflowRunStatus = 'running' | 'success' | 'warn' | 'error' | 'cancelled'
export type WorkflowKind = 'account_generate' | 'data_cleanup' | 'status_report' | 'environment_check' | 'openai_register' | 'openai_register_browser'

export interface WorkflowDefinition {
  id: string
  kind: WorkflowKind
  title: string
  summary: string
  status: WorkflowStatus
  builtin: boolean
  parameters: WorkflowParameters
}

export interface WorkflowParameters {
  batch_size?: number
  account_domain?: string
  days_to_keep?: number
  report_window_hours?: number
  require_env_secret_match?: boolean
  require_public_hub_url?: boolean
  require_webhook?: boolean
  proxy_url?: string
  captcha_key?: string
  cpa_url?: string
  cpa_key?: string
  concurrency?: number
  account_type?: string
  full_name?: string
  age?: number
  sms_key?: string
  headless?: boolean
}

export interface WorkflowRunRecord {
  id: string
  workflow_id: string
  workflow_title: string
  status: WorkflowRunStatus
  message: string
  started_at: number
  finished_at: number | null
}

export interface WorkflowRunPageResponse {
  items: WorkflowRunRecord[]
  total: number
  page: number
  page_size: number
}

export interface WorkflowStepRecord {
  id: string
  run_id: string
  step_index: number
  level: WorkflowRunStatus | 'info'
  message: string
  created_at: number
  workflow_id?: string
  workflow_title?: string
}

export interface DashboardStats {
  total_emails: number
  active_emails: number
  archived_emails: number
  code_emails: number
  recent_emails_24h: number
  active_webhooks: number
  workflow_runs_24h: number
  successful_runs_24h: number
  total_accounts?: number
  today_accounts_24h?: number
  latest_email_at: number | null
}

export interface GeneratedAccountRecord {
  id: string
  run_id: string
  address: string
  password: string
  status: string
  created_at: number
  access_token?: string | null
  refresh_token?: string | null
  session_token?: string | null
  device_id?: string | null
  workspace_id?: string | null
  upload_status?: string | null
  account_type?: string | null
}

export interface EmailPageResponse {
  items: EmailRecordApi[]
  total: number
  page: number
  page_size: number
}

export interface PhantomLogEventDetail {
  msg: string
  level?: LogLevel
}

export interface PhantomOpenEmailsDetail {
  query?: string
}

export interface PhantomOpenTabDetail {
  tab: AppTab
}

export interface PhantomSettingsUpdatedDetail {
  update_rate?: number | null
  decode_depth?: string | null
  account_domain?: string | null
}

export interface PhantomEmailUpdatedDetail {
  id: string
  archived: boolean
}

export interface PhantomEmailDeletedDetail {
  id: string
}
