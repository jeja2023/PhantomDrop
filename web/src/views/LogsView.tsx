import { useMemo, useState, type ReactNode } from 'react'
import { Activity, CheckCircle2, Clock3, Shield } from 'lucide-react'
import Terminal from '../terminal/Terminal'
import PageHeader from '../ui/PageHeader'
import type { AppLog, DashboardStats, LogSource, WorkflowRunRecord, WorkflowStepRecord } from '../types'

interface LogsViewProps {
  logs: AppLog[]
  stats: DashboardStats | null
  workflowRuns: WorkflowRunRecord[]
  workflowSteps: WorkflowStepRecord[]
}

export default function LogsView({ logs, stats, workflowRuns, workflowSteps }: LogsViewProps) {
  const latestRun = workflowRuns[0] ?? null
  const [activeFilter, setActiveFilter] = useState<'all' | LogSource>('all')

  const filterStats = useMemo(
    () => ({
      all: logs.length,
      system_log: logs.filter((log) => log.source === 'system_log').length,
      workflow_step: logs.filter((log) => log.source === 'workflow_step').length,
      ui: logs.filter((log) => log.source === 'ui').length,
    }),
    [logs],
  )

  return (
    <div className="page-shell page-shell--full min-w-0 animate-in fade-in duration-700">
      <PageHeader
        title=""
        kicker=""
        description=""
        status={
          <>
            <StatusBadge label="内核状态" status="就绪" color="blue" />
            <StatusBadge label="近 24 小时工作流" status={`${stats?.workflow_runs_24h ?? 0} 次`} color="cyan" />
          </>
        }
      />

      <div className="flex-grow grid min-h-0 min-w-0 grid-cols-1 gap-3 lg:grid-cols-[minmax(0,1.8fr)_280px_280px]">
        <div className="page-panel overflow-hidden flex min-w-0 flex-col">
          <div className="px-4 py-2 bg-slate-100 border-b border-slate-200 flex flex-wrap items-center justify-between shrink-0 gap-3">
            <div className="flex min-w-0 items-center gap-4">
              <span className="text-[10px] font-black text-slate-600 tracking-widest">原始日志流</span>
              <div className="flex items-center gap-1.5">
                <div className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse"></div>
                <span className="text-[9px] text-emerald-500 font-mono font-bold">正在流化</span>
              </div>
            </div>
            <div className="flex flex-wrap items-center gap-2 text-[9px] font-mono">
              <FilterChip label={`全部 ${filterStats.all}`} active={activeFilter === 'all'} onClick={() => setActiveFilter('all')} />
              <FilterChip label={`系统 ${filterStats.system_log}`} active={activeFilter === 'system_log'} onClick={() => setActiveFilter('system_log')} />
              <FilterChip label={`步骤 ${filterStats.workflow_step}`} active={activeFilter === 'workflow_step'} onClick={() => setActiveFilter('workflow_step')} />
              <FilterChip label={`界面 ${filterStats.ui}`} active={activeFilter === 'ui'} onClick={() => setActiveFilter('ui')} />
            </div>
          </div>
          <div className="flex-grow min-h-0 min-w-0">
            <Terminal logs={logs} activeFilter={activeFilter} />
          </div>
        </div>

        <div className="page-panel flex min-h-0 min-w-0 flex-col overflow-hidden bg-gradient-to-br from-blue-600/5 via-white to-transparent">
          <div className="border-b border-slate-200 px-4 py-3">
            <div className="text-[10px] font-black text-blue-500 tracking-widest leading-none">运行态摘要</div>
          </div>
          <div className="space-y-2 p-3">
            <div className="grid grid-cols-2 gap-2">
              <RuntimeMetric icon={<Activity size={14} />} label="近 24 小时新邮件" value={`${stats?.recent_emails_24h ?? 0}`} />
              <RuntimeMetric icon={<Shield size={14} />} label="活跃 Webhook" value={`${stats?.active_webhooks ?? 0}`} />
              <RuntimeMetric icon={<CheckCircle2 size={14} />} label="成功执行" value={`${stats?.successful_runs_24h ?? 0}`} />
              <RuntimeMetric icon={<Clock3 size={14} />} label="最近邮件时间" value={stats?.latest_email_at ? new Date(stats.latest_email_at * 1000).toLocaleTimeString() : '--'} />
            </div>

            <div className="rounded-2xl border border-slate-200 bg-slate-50/90 p-3">
              <div className="flex items-center justify-between gap-3">
                <div className="text-[9px] font-black tracking-widest text-slate-500">最近执行</div>
                {latestRun ? <StepStateBadge level={latestRun.status} /> : null}
              </div>
              {latestRun ? (
                <>
                  <div className="mt-1.5 text-sm font-bold text-slate-900">{latestRun.workflow_title}</div>
                  <div className="mt-1 line-clamp-2 text-[10px] leading-relaxed text-slate-600">{latestRun.message}</div>
                  <div className="mt-2 flex items-center justify-between gap-2 text-[9px] font-mono text-slate-500">
                    <span className="truncate">运行标识：{latestRun.id}</span>
                    <span className="shrink-0">{new Date(latestRun.started_at * 1000).toLocaleTimeString()}</span>
                  </div>
                </>
              ) : (
                <div className="mt-2 text-[10px] text-slate-500">暂时没有最近执行记录。</div>
              )}
            </div>
          </div>
        </div>

        <div className="page-panel flex min-h-0 min-w-0 flex-col overflow-hidden font-mono">
          <div className="border-b border-slate-200 px-4 py-3">
            <div className="flex items-center justify-between gap-3">
              <div>
                <div className="text-[10px] font-black text-slate-600 tracking-widest leading-none">工作流步骤流</div>
                <div className="mt-1 text-[10px] text-slate-500">最近步骤事件按时间倒序排列。</div>
              </div>
              <div className="rounded-full border border-slate-200 bg-slate-50 px-3 py-1 text-[9px] font-black tracking-widest text-slate-500">
                {workflowSteps.length} 条
              </div>
            </div>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto custom-scrollbar p-4">
            {workflowSteps.length === 0 ? (
              <div className="rounded-2xl border border-dashed border-slate-200 p-4 text-[10px] text-slate-500">
                当前没有步骤事件流入。
              </div>
            ) : (
              <div className="space-y-3">
                {workflowSteps.map((step) => (
                  <div key={step.id} className="rounded-2xl border border-slate-200 bg-slate-50/80 p-3.5 text-[10px]">
                    <div className="flex items-start justify-between gap-3">
                      <div className="min-w-0">
                        <div className="truncate text-[10px] font-black tracking-widest text-blue-600">
                          {step.workflow_title || step.workflow_id || '未命名工作流'}
                        </div>
                        <div className="mt-1 text-[9px] font-mono text-slate-400">
                          第 {step.step_index} 步 / {new Date(step.created_at * 1000).toLocaleTimeString()}
                        </div>
                      </div>
                      <StepStateBadge level={step.level} />
                    </div>
                    <div className="mt-3 break-words text-[11px] leading-relaxed text-slate-700">{step.message}</div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>

      <div className="page-panel h-10 flex items-center px-6 justify-between border-b-0 border-t-2 border-t-blue-500/20 shrink-0">
        <div className="flex items-center gap-6">
          <div className="text-[9px] text-slate-600 font-bold tracking-widest">
            工作流记录：<span className="text-blue-500 ml-1">{workflowRuns.length}</span>
          </div>
          <div className="text-[9px] text-slate-600 font-bold tracking-widest">
            步骤事件：<span className="text-blue-400 ml-1">{workflowSteps.length}</span>
          </div>
        </div>
        <div className="text-[9px] text-slate-700 font-mono italic animate-pulse">
          当前运行态由真实步骤流驱动
        </div>
      </div>
    </div>
  )
}

function RuntimeMetric({ icon, label, value }: { icon: ReactNode; label: string; value: string }) {
  return (
    <div className="rounded-2xl border border-slate-200 bg-white/80 p-2.5">
      <div className="flex items-center gap-2 text-slate-500">
        {icon}
        <span className="text-[9px] font-bold tracking-widest">{label}</span>
      </div>
      <div className="mt-1.5 text-sm font-black text-slate-900">{value}</div>
    </div>
  )
}

function StepStateBadge({ level }: { level: 'running' | 'success' | 'warn' | 'info' | 'error' | 'cancelled' }) {
  const tone =
    level === 'success'
      ? 'bg-emerald-500/10 text-emerald-600'
      : level === 'warn'
        ? 'bg-amber-500/10 text-amber-600'
        : level === 'error'
          ? 'bg-rose-500/10 text-rose-600'
          : level === 'info'
            ? 'bg-slate-200 text-slate-600'
            : level === 'cancelled'
              ? 'bg-neutral-500/10 text-neutral-600'
              : 'bg-blue-500/10 text-blue-600'

  const labelMap = {
    running: '运行中',
    success: '成功',
    warn: '警告',
    error: '错误',
    info: '信息',
    cancelled: '已取消',
  } as const

  return <span className={`shrink-0 rounded-full px-2.5 py-1 text-[9px] font-black tracking-widest ${tone}`}>{labelMap[level]}</span>
}

function FilterChip({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-full px-2.5 py-1 transition-colors ${
        active ? 'bg-blue-600 text-white' : 'bg-white text-slate-500 hover:bg-slate-200'
      }`}
    >
      {label}
    </button>
  )
}

function StatusBadge({ label, status, color }: { label: string; status: string; color: 'blue' | 'cyan' }) {
  const toneClass = color === 'cyan' ? 'text-cyan-600' : 'text-blue-600'

  return (
    <div className="flex items-center gap-2 px-2.5 py-1 bg-slate-50 border border-slate-200 rounded-lg">
      <span className="text-[9px] text-slate-500 font-bold tracking-tighter">{label}</span>
      <div className="h-2 w-px bg-slate-200"></div>
      <span className={`text-[9px] font-black font-mono tracking-tight ${toneClass}`}>{status}</span>
    </div>
  )
}
