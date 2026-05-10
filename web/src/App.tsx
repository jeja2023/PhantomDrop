import { useEffect, useMemo, useState, type ReactNode } from 'react'
import { RefreshCw, Command, Activity, RadioTower } from 'lucide-react'
import Sidebar from './ui/Sidebar'
import Cmd from './cmd/Cmd'
import './App.css'
import DashboardView from './views/DashboardView'
import EmailListView from './views/EmailListView'
import LogsView from './views/LogsView'
import AutomationView from './views/AutomationView'
import SettingsView from './views/SettingsView'
import TunnelView from './views/TunnelView'
import RegistrationView from './views/RegistrationView'
import AccountListView from './views/AccountListView'
import { createApiEventSource, fetchJson } from './lib/api'
import type {
  AppLog,
  AppTab,
  DashboardStats,
  EmailItem,
  EmailRecordApi,
  PhantomEmailDeletedDetail,
  PhantomLogEventDetail,
  PhantomEmailUpdatedDetail,
  PhantomOpenEmailsDetail,
  PhantomOpenTabDetail,
  PhantomSettingsUpdatedDetail,
  StreamEmailPayload,
  SystemLogPayload,
  WorkflowRunRecord,
  WorkflowRunPageResponse,
  WorkflowStepRecord,
} from './types'

function formatEmail(record: EmailRecordApi): EmailItem {
  return {
    id: record.id,
    from: record.from_addr,
    to: record.to_addr,
    subject: record.subject || '无主题',
    time: new Date(record.created_at * 1000).toLocaleString(),
    code: record.extracted_code || '',
    link: record.extracted_link || undefined,
    isArchived: record.is_archived,
  }
}

function appendLog(
  logs: AppLog[],
  content: string,
  type: AppLog['type'],
  source: AppLog['source'],
  groupLabel?: string,
): AppLog[] {
  return [
    {
      id: `${source}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      time: new Date().toLocaleTimeString(),
      content,
      type,
      source,
      groupLabel,
    },
    ...logs,
  ].slice(0, 100)
}

function App() {
  const [activeTab, setActiveTab] = useState<AppTab>(() => {
    const saved = localStorage.getItem('phantom_active_tab')
    return (saved as AppTab) || 'dashboard'
  })

  useEffect(() => {
    localStorage.setItem('phantom_active_tab', activeTab)
  }, [activeTab])
  const [isCmdOpen, setIsCmdOpen] = useState(false)
  const [isRefreshing, setIsRefreshing] = useState(false)
  const [emailSearchQuery, setEmailSearchQuery] = useState('')
  const [updateRate, setUpdateRate] = useState(1000)
  const [healthStatus, setHealthStatus] = useState<'checking' | 'online' | 'offline'>('checking')
  const [streamStatus, setStreamStatus] = useState<'connecting' | 'online' | 'reconnecting'>('connecting')
  const [emails, setEmails] = useState<EmailItem[]>([])
  const [stats, setStats] = useState<DashboardStats | null>(null)
  const [workflowRuns, setWorkflowRuns] = useState<WorkflowRunRecord[]>([])
  const [workflowSteps, setWorkflowSteps] = useState<WorkflowStepRecord[]>([])
  const [isAuthModalOpen, setIsAuthModalOpen] = useState(false)
  const [authPassword, setAuthPassword] = useState('')
  
  useEffect(() => {
    const handleUnauthorized = () => {
      setIsAuthModalOpen(true)
    }
    window.addEventListener('phantom-unauthorized', handleUnauthorized)
    return () => {
      window.removeEventListener('phantom-unauthorized', handleUnauthorized)
    }
  }, [])

  const handleAuthSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    const token = authPassword.trim()
    if (!token) return
    localStorage.setItem('phantom_auth_token', token)
    const secure = window.location.protocol === 'https:' ? '; Secure' : ''
    document.cookie = `phantom_auth_token=${encodeURIComponent(token)}; path=/; max-age=31536000; SameSite=Lax${secure}`
    setIsAuthModalOpen(false)
    window.location.reload()
  }

  const [logs, setLogs] = useState<AppLog[]>([
    {
      id: `ui-${Date.now()}`,
      time: new Date().toLocaleTimeString(),
      content: '系统正在连接中',
      type: 'info',
      source: 'ui',
      groupLabel: '界面',
    },
  ])

  const loadEmails = async (withIndicator = false) => {
    if (withIndicator) setIsRefreshing(true)

    try {
      const data = await fetchJson<EmailRecordApi[]>('/api/emails')
      setEmails(data.map(formatEmail))
      setLogs((prev) => appendLog(prev, '拉取历史数据成功', 'success', 'ui', '界面'))
    } catch (error) {
      const message = error instanceof Error ? error.message : '未知错误'
      setLogs((prev) => appendLog(prev, `拉取数据失败：${message}`, 'warn', 'ui', '界面'))
    } finally {
      if (withIndicator) setIsRefreshing(false)
    }
  }

  const loadStats = async () => {
    try {
      const data = await fetchJson<DashboardStats>('/api/stats')
      setStats(data)
    } catch (error) {
      const message = error instanceof Error ? error.message : '未知错误'
      setLogs((prev) => appendLog(prev, `读取统计失败：${message}`, 'warn', 'ui', '界面'))
    }
  }

  const loadHealth = async () => {
    try {
      await fetchJson<string>('/health')
      setHealthStatus('online')
    } catch {
      setHealthStatus('offline')
    }
  }

  const loadSettings = async () => {
    try {
      const settings = await fetchJson<{ update_rate?: number | null }>('/api/settings')
      setUpdateRate(Math.max(1000, settings.update_rate ?? 1000))
    } catch {
      setUpdateRate(1000)
    }
  }

  const loadWorkflowRuns = async () => {
    try {
      const data = await fetchJson<WorkflowRunPageResponse>('/api/workflow-runs?page=1&page_size=20')
      setWorkflowRuns(data.items)
      const latestRunId = data.items[0]?.id
      if (latestRunId) {
        const steps = await fetchJson<WorkflowStepRecord[]>(`/api/workflow-runs/${latestRunId}/steps`)
        setWorkflowSteps(steps)
      } else {
        setWorkflowSteps([])
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : '未知错误'
      setLogs((prev) => appendLog(prev, `读取工作流运行记录失败：${message}`, 'warn', 'ui', '界面'))
    }
  }

  useEffect(() => {
    void loadEmails()
    void loadStats()
    void loadWorkflowRuns()
    void loadSettings()
    void loadHealth()

    const eventSource = createApiEventSource('/stream')
    setStreamStatus('connecting')

    eventSource.onopen = () => {
      setStreamStatus('online')
    }

    eventSource.addEventListener('new_email', (event) => {
      const data = JSON.parse(event.data) as StreamEmailPayload
      setEmails((prev) => [
        {
          id: data.id,
          from: data.from,
          to: data.to,
          subject: data.subject || '无主题',
          time: '刚刚',
          code: data.code || '',
          link: data.link || undefined,
        },
        ...prev,
      ].slice(0, 100))

      setLogs((prev) => appendLog(prev, `新邮件流入：${data.from}，验证码：${data.code || '无'}`, 'success', 'system_log', '系统'))
      void loadStats()
    })

    eventSource.addEventListener('system_log', (event) => {
      const data = JSON.parse(event.data) as SystemLogPayload
      setLogs((prev) => appendLog(prev, data.msg, data.level === 'warn' ? 'warn' : data.level, 'system_log', '系统'))
    })

    eventSource.addEventListener('workflow_step', (event) => {
      const data = JSON.parse(event.data) as {
        run_id: string
        workflow_id: string
        workflow_title: string
        step_index: number
        level: WorkflowStepRecord['level']
        msg: string
      }

      setWorkflowSteps((prev) => [
        {
          id: `${data.run_id}-${data.step_index}`,
          run_id: data.run_id,
          workflow_id: data.workflow_id,
          workflow_title: data.workflow_title,
          step_index: data.step_index,
          level: data.level,
          message: data.msg,
          created_at: Math.floor(Date.now() / 1000),
        },
        ...prev,
      ].slice(0, 30))

      setLogs((prev) =>
        appendLog(prev, data.msg, data.level === 'warn' ? 'warn' : data.level === 'success' ? 'success' : 'info', 'workflow_step', data.workflow_title),
      )

      void loadWorkflowRuns()
    })

    eventSource.onerror = () => {
      setStreamStatus('reconnecting')
      setLogs((prev) => appendLog(prev, '实时流连接中断，正在等待浏览器自动重连', 'warn', 'ui', '界面'))
    }

    return () => eventSource.close()
  }, [])

  useEffect(() => {
    const interval = setInterval(() => {
      void loadHealth()
      void loadStats()
      void loadWorkflowRuns()
    }, updateRate)

    return () => clearInterval(interval)
  }, [updateRate])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault()
        setIsCmdOpen(true)
      }
    }

    const handleOpenCmd = () => setIsCmdOpen(true)
    const handleOpenEmails = (event: Event) => {
      const detail = (event as CustomEvent<PhantomOpenEmailsDetail>).detail
      setActiveTab('emails')
      setEmailSearchQuery(detail?.query || '')
    }
    const handleOpenTab = (event: Event) => {
      const detail = (event as CustomEvent<PhantomOpenTabDetail>).detail
      if (detail?.tab) setActiveTab(detail.tab)
    }
    const handleEmailUpdated = (event: Event) => {
      const detail = (event as CustomEvent<PhantomEmailUpdatedDetail>).detail
      if (!detail?.id) return
      setEmails((prev) => prev.map((email) => (email.id === detail.id ? { ...email, isArchived: detail.archived } : email)))
      void loadStats()
    }
    const handleEmailDeleted = (event: Event) => {
      const detail = (event as CustomEvent<PhantomEmailDeletedDetail>).detail
      if (!detail?.id) return
      setEmails((prev) => prev.filter((email) => email.id !== detail.id))
      void loadStats()
    }
    const handleSettingsUpdated = (event: Event) => {
      const detail = (event as CustomEvent<PhantomSettingsUpdatedDetail>).detail
      setUpdateRate(Math.max(1000, detail?.update_rate ?? 1000))
    }
    const handleLog = (event: Event) => {
      const detail = (event as CustomEvent<PhantomLogEventDetail>).detail
      if (!detail?.msg) return
      setLogs((prev) => appendLog(prev, detail.msg, detail.level || 'info', 'ui', '界面'))
    }

    window.addEventListener('keydown', handleKeyDown)
    window.addEventListener('phantom-open-cmd', handleOpenCmd)
    window.addEventListener('phantom-email-updated', handleEmailUpdated)
    window.addEventListener('phantom-email-deleted', handleEmailDeleted)
    window.addEventListener('phantom-open-emails', handleOpenEmails)
    window.addEventListener('phantom-open-tab', handleOpenTab)
    window.addEventListener('phantom-settings-updated', handleSettingsUpdated)
    window.addEventListener('phantom-log', handleLog)

    return () => {
      window.removeEventListener('keydown', handleKeyDown)
      window.removeEventListener('phantom-open-cmd', handleOpenCmd)
      window.removeEventListener('phantom-email-updated', handleEmailUpdated)
      window.removeEventListener('phantom-email-deleted', handleEmailDeleted)
      window.removeEventListener('phantom-open-emails', handleOpenEmails)
      window.removeEventListener('phantom-open-tab', handleOpenTab)
      window.removeEventListener('phantom-settings-updated', handleSettingsUpdated)
      window.removeEventListener('phantom-log', handleLog)
    }
  }, [])

  const headerStatus = useMemo(() => {
    if (emails.length === 0) return '等待数据'
    return `最近 ${emails.length} 封`
  }, [emails.length])

  const isPageScrollable = activeTab === 'auto' || activeTab === 'config'

  const renderView = () => {
    switch (activeTab) {
      case 'dashboard':
        return <DashboardView emails={emails} logs={logs} stats={stats} updateRate={updateRate} />
      case 'emails':
        return <EmailListView emails={emails} externalQuery={emailSearchQuery} />
      case 'logs':
        return <LogsView logs={logs} stats={stats} workflowRuns={workflowRuns} workflowSteps={workflowSteps} />
      case 'tunnel':
        return <TunnelView />
      case 'auto':
        return <AutomationView refreshIntervalMs={updateRate} />
      case 'config':
        return <SettingsView />
      case 'register':
        return <RegistrationView refreshIntervalMs={updateRate} />
      case 'accounts':
        return <AccountListView />
      default:
        return <DashboardView emails={emails} logs={logs} stats={stats} />
    }
  }

  return (
    <div className="min-h-screen dot-matrix flex overflow-hidden bg-[#f8fafc] font-sans text-slate-900 selection:bg-blue-200">
      <Sidebar activeTab={activeTab} onTabChange={setActiveTab} />

      <main className="relative flex h-screen min-w-0 flex-grow flex-col overflow-hidden">
        <div className="pointer-events-none absolute inset-0 z-0 opacity-40">
          <div className="absolute left-[-10%] top-[-10%] h-[40%] w-[40%] animate-pulse rounded-full bg-blue-400/10 blur-[120px]"></div>
          <div className="absolute bottom-[-10%] right-[-10%] h-[40%] w-[40%] animate-pulse rounded-full bg-indigo-400/10 blur-[120px] transition-all duration-1000"></div>
        </div>

        <header className="sticky top-0 z-30 grid h-16 min-h-16 shrink-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-4 border-b border-slate-200 bg-white/95 px-6 shadow-[0_2px_12px_rgba(0,0,0,0.02)] backdrop-blur-sm transition-all duration-300 lg:px-8">
          <div className="flex min-w-0 items-center gap-4 overflow-hidden lg:gap-6">
            <div className="shrink-0 cursor-crosshair rounded-full border border-slate-200 bg-slate-50 px-3.5 py-2 transition-all hover:border-blue-300">
              <div className="flex items-center gap-2">
                <span className="h-1.5 w-1.5 rounded-full bg-emerald-500 shadow-[0_0_8px_#10b981]"></span>
                <div className="flex flex-col">
                  <span className="text-[11px] font-bold leading-none text-slate-900">系统就绪</span>
                  <span className="mt-0.5 text-[8px] font-mono leading-none tracking-widest text-slate-500">系统激活</span>
                </div>
              </div>
            </div>
            <div className="hidden h-4 w-px bg-slate-200 lg:block"></div>
            <div className="min-w-0 truncate text-[10px] font-mono tracking-widest text-slate-600">
              邮件概览：<span className="ml-1 font-black text-blue-600">{headerStatus}</span>
            </div>
            <div className="hidden h-4 w-px bg-slate-200 xl:block"></div>
            <HeaderBadge icon={<Activity size={11} />} label="后端" value={healthStatus === 'online' ? '在线' : healthStatus === 'offline' ? '离线' : '检测中'} tone={healthStatus === 'online' ? 'emerald' : healthStatus === 'offline' ? 'rose' : 'slate'} />
            <HeaderBadge icon={<RadioTower size={11} />} label="流状态" value={streamStatus === 'online' ? '流化中' : streamStatus === 'reconnecting' ? '重连中' : '连接中'} tone={streamStatus === 'online' ? 'blue' : streamStatus === 'reconnecting' ? 'amber' : 'slate'} />
          </div>

          <div className="flex shrink-0 items-center gap-3 lg:gap-6">
            <div className="group hidden h-10 w-[196px] cursor-pointer items-center gap-3 rounded-xl border border-slate-200 bg-slate-100/80 px-4 text-[11px] text-slate-600 transition-all hover:border-blue-400/30 hover:bg-slate-100 xl:flex" onClick={() => setIsCmdOpen(true)}>
              <Command size={12} className="text-slate-600 transition-colors group-hover:text-blue-500" />
              <div className="flex min-w-0 flex-col">
                <span className="text-[11px] font-bold leading-none tracking-tight text-slate-800 group-hover:text-slate-900">指令搜索</span>
                <span className="mt-0.5 text-[8px] font-mono leading-none tracking-widest text-slate-500 group-hover:text-blue-500">打开快捷命令</span>
              </div>
              <kbd className="ml-4 rounded border border-slate-200 bg-white px-1.5 py-0.5 font-mono text-[9px] text-slate-600 shadow-sm">⌘K</kbd>
            </div>

            <button
              className="group rounded-xl p-2.5 text-slate-600 transition-all hover:bg-slate-100 hover:text-slate-900"
              onClick={() => {
                void loadEmails(true)
                void loadHealth()
                void loadStats()
                void loadWorkflowRuns()
              }}
              type="button"
            >
              <RefreshCw size={16} className={isRefreshing ? 'animate-spin' : 'transition-transform duration-1000 group-hover:rotate-180'} />
            </button>
          </div>
        </header>

        <div className={`relative z-10 flex-grow px-8 pt-1 pb-8 scroll-smooth ${isPageScrollable ? 'overflow-y-auto' : 'overflow-hidden'}`}>
          <div className={`mx-auto max-w-[1600px] ${isPageScrollable ? 'min-h-full' : 'h-full min-h-0'}`}>{renderView()}</div>
        </div>

        <Cmd isOpen={isCmdOpen} onClose={() => setIsCmdOpen(false)} />
      </main>

      {isAuthModalOpen && (
        <div className="fixed inset-0 z-[9999] flex items-center justify-center bg-slate-900/60 backdrop-blur-md">
          <div className="w-[360px] rounded-2xl border border-slate-200 bg-white p-8 shadow-2xl animate-in fade-in zoom-in-95 duration-200">
            <div className="flex flex-col items-center text-center">
              <div className="mb-4 flex h-12 w-12 items-center justify-center rounded-full bg-blue-50 text-blue-600">
                <Command size={24} />
              </div>
              <h2 className="text-xl font-bold text-slate-900">系统认证</h2>
              <p className="mt-2 text-xs leading-relaxed text-slate-500">
                该节点已启用安全保护，请输入您的认证密钥 (auth_secret) 以继续。
              </p>
              
              <form onSubmit={handleAuthSubmit} className="mt-6 w-full">
                <input
                  type="password"
                  placeholder="请输入密钥"
                  value={authPassword}
                  onChange={(e) => setAuthPassword(e.target.value)}
                  className="w-full rounded-xl border border-slate-200 px-4 py-3 text-sm outline-none transition-all focus:border-blue-500 focus:ring-2 focus:ring-blue-100"
                  autoFocus
                />
                
                <button
                  type="submit"
                  className="mt-4 w-full rounded-xl bg-blue-600 py-3 text-sm font-bold text-white transition-all hover:bg-blue-700 active:scale-[0.98]"
                >
                  确认登录
                </button>
              </form>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

export default App

function HeaderBadge({
  icon,
  label,
  value,
  tone,
}: {
  icon: ReactNode
  label: string
  value: string
  tone: 'blue' | 'emerald' | 'amber' | 'rose' | 'slate'
}) {
  const toneClass = {
    blue: 'bg-blue-50 text-blue-700 border-blue-100',
    emerald: 'bg-emerald-50 text-emerald-700 border-emerald-100',
    amber: 'bg-amber-50 text-amber-700 border-amber-100',
    rose: 'bg-rose-50 text-rose-700 border-rose-100',
    slate: 'bg-slate-50 text-slate-600 border-slate-200',
  }[tone]

  return (
    <div className={`hidden items-center gap-2 rounded-full border px-3 py-1.5 lg:flex ${toneClass}`}>
      {icon}
      <span className="text-[9px] font-black tracking-widest">{label}</span>
      <span className="text-[9px] font-mono font-bold">{value}</span>
    </div>
  )
}
