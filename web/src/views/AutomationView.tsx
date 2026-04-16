import { useEffect, useState, useRef } from 'react'
import { Zap, Play, CheckCircle2, Loader2, Save, Plus, Trash2, Download, Copy, Square } from 'lucide-react'
import { motion, AnimatePresence } from 'framer-motion'
import { buildApiUrl, deleteJson, fetchJson, postJson } from '../lib/api'
import PageHeader from '../ui/PageHeader'
import type {
  GeneratedAccountRecord,
  WorkflowDefinition,
  WorkflowKind,
  WorkflowParameters,
  WorkflowRunPageResponse,
  WorkflowRunRecord,
  WorkflowStepRecord,
} from '../types'

export default function AutomationView({ refreshIntervalMs }: { refreshIntervalMs: number }) {
  const [showToast, setShowToast] = useState(false)
  const [toastContent, setToastContent] = useState({ title: '', desc: '' })
  const [isLoading, setIsLoading] = useState(true)
  const [runningId, setRunningId] = useState<string | null>(null)
  const [savingId, setSavingId] = useState<string | null>(null)
  const [workflows, setWorkflows] = useState<WorkflowDefinition[]>([])
  const [runs, setRuns] = useState<WorkflowRunRecord[]>([])
  const [runTotal, setRunTotal] = useState(0)
  const [runPage, setRunPage] = useState(1)
  const [runPageSize, setRunPageSize] = useState(20)
  const [runStatusFilter, setRunStatusFilter] = useState('')
  const [runWorkflowFilter, setRunWorkflowFilter] = useState('')
  const [runWorkflowMatch, setRunWorkflowMatch] = useState<'fuzzy' | 'exact'>('fuzzy')
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null)
  const [steps, setSteps] = useState<WorkflowStepRecord[]>([])
  const [accounts, setAccounts] = useState<GeneratedAccountRecord[]>([])
  const [isStepsLoading, setIsStepsLoading] = useState(false)
  const [editingWorkflowId, setEditingWorkflowId] = useState<string | null>(null)
  const [copiedOutput, setCopiedOutput] = useState(false)
  const stepsContainerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (stepsContainerRef.current) {
      stepsContainerRef.current.scrollTop = stepsContainerRef.current.scrollHeight
    }
  }, [steps])

  const createDraftWorkflow = (): WorkflowDefinition => ({
    id: `workflow_${Date.now()}`,
    kind: 'account_generate',
    title: '新工作流',
    summary: '待补充说明',
    status: 'ready',
    builtin: false,
    parameters: {
      batch_size: 10,
      account_domain: 'phantom.local',
    },
  })

  const buildRunQuery = (
    page: number,
    pageSize: number,
    status = runStatusFilter,
    workflowId = runWorkflowFilter,
    match = runWorkflowMatch,
  ) => {
    const search = new URLSearchParams()
    search.set('page', String(page))
    search.set('page_size', String(pageSize))
    if (status) search.set('status', status)
    if (workflowId.trim()) {
      search.set('workflow_id', workflowId.trim())
      search.set('workflow_exact', match === 'exact' ? 'true' : 'false')
    }
    return search.toString()
  }

  const loadRuns = async (
    page = runPage,
    pageSize = runPageSize,
    preserveSelection = true,
    status = runStatusFilter,
    workflowId = runWorkflowFilter,
    match = runWorkflowMatch,
  ): Promise<WorkflowRunRecord[]> => {
    const data = await fetchJson<WorkflowRunPageResponse>(`/api/workflow-runs?${buildRunQuery(page, pageSize, status, workflowId, match)}`)
    setRuns(data.items)
    setRunTotal(data.total)
    setRunPage(data.page)
    setRunPageSize(data.page_size)
    setSelectedRunId((current) => {
      if (preserveSelection && current && data.items.some((run) => run.id === current)) return current
      return data.items[0]?.id ?? null
    })
    return data.items
  }

  const loadSteps = async (runId: string, silent = false) => {
    if (!silent) setIsStepsLoading(true)
    try {
      const data = await fetchJson<WorkflowStepRecord[]>(`/api/workflow-runs/${runId}/steps`)
      setSteps(data)
    } finally {
      if (!silent) setIsStepsLoading(false)
    }
  }

  const loadAccounts = async (runId: string) => {
    const data = await fetchJson<GeneratedAccountRecord[]>(`/api/workflow-runs/${runId}/accounts`)
    setAccounts(data)
  }

  useEffect(() => {
    const loadWorkflows = async () => {
      try {
        const [workflowDefs, workflowRuns] = await Promise.all([
          fetchJson<WorkflowDefinition[]>('/api/workflows'),
          fetchJson<WorkflowRunPageResponse>(`/api/workflow-runs?${buildRunQuery(1, 20, '', '', 'fuzzy')}`),
        ])
        setWorkflows(workflowDefs)
        setRuns(workflowRuns.items)
        setRunTotal(workflowRuns.total)
        setRunPage(workflowRuns.page)
        setRunPageSize(workflowRuns.page_size)
        setSelectedRunId(workflowRuns.items[0]?.id ?? null)
      } catch (error) {
        const message = error instanceof Error ? error.message : '读取工作流定义失败'
        setToastContent({ title: '工作流读取失败', desc: message })
        setShowToast(true)
        setTimeout(() => setShowToast(false), 3000)
      } finally {
        setIsLoading(false)
      }
    }

    void loadWorkflows()
  }, [])

  useEffect(() => {
    const interval = setInterval(() => {
      void loadRuns(runPage, runPageSize, true)
    }, refreshIntervalMs)

    return () => clearInterval(interval)
  }, [refreshIntervalMs, runPage, runPageSize, runStatusFilter, runWorkflowFilter, runWorkflowMatch])

  useEffect(() => {
    if (!selectedRunId) {
      setSteps([])
      setAccounts([])
      return
    }

    void loadSteps(selectedRunId)
    void loadAccounts(selectedRunId)
  }, [selectedRunId])

  useEffect(() => {
    if (!selectedRunId) return

    const interval = setInterval(() => {
      void loadSteps(selectedRunId, true)
      void loadAccounts(selectedRunId)
    }, refreshIntervalMs)

    return () => clearInterval(interval)
  }, [selectedRunId, refreshIntervalMs])

  const handleAction = async (workflow: WorkflowDefinition) => {
    setRunningId(workflow.id)
    try {
      await postJson<{ status: string; run_id: string }, { workflow_id: string }>('/api/workflows/trigger', { workflow_id: workflow.id })
      const latestRuns = await loadRuns(1, runPageSize, false, runStatusFilter, runWorkflowFilter, runWorkflowMatch)
      if (latestRuns[0]?.id) setSelectedRunId(latestRuns[0].id)
      setToastContent({ title: '指令已下发', desc: `工作流“${workflow.title}”已进入异步执行队列。` })
      setShowToast(true)
      setTimeout(() => setShowToast(false), 2000)
    } catch (error) {
      const message = error instanceof Error ? error.message : '触发失败'
      setToastContent({ title: '触发失败', desc: message })
      setShowToast(true)
      setTimeout(() => setShowToast(false), 3000)
    } finally {
      setRunningId(null)
    }
  }

  const updateWorkflowField = (workflowId: string, patch: Partial<WorkflowDefinition>) => {
    setWorkflows((prev) => prev.map((workflow) => (workflow.id === workflowId ? { ...workflow, ...patch } : workflow)))
  }

  const updateWorkflowParameters = (workflowId: string, patch: Partial<WorkflowParameters>) => {
    setWorkflows((prev) => prev.map((workflow) => (workflow.id === workflowId ? { ...workflow, parameters: { ...workflow.parameters, ...patch } } : workflow)))
  }

  const handleSaveWorkflow = async (workflow: WorkflowDefinition) => {
    setSavingId(workflow.id)
    try {
      await postJson<{ status: string }, { id: string; kind: string; title: string; summary: string; status: string; parameters_json: string }>(
        '/api/workflows/save',
        {
          id: workflow.id,
          kind: workflow.kind,
          title: workflow.title,
          summary: workflow.summary,
          status: workflow.status,
          parameters_json: JSON.stringify(workflow.parameters ?? {}),
        },
      )
      setToastContent({ title: '工作流已保存', desc: `${workflow.title} 的定义已写入数据库。` })
      setShowToast(true)
      setTimeout(() => setShowToast(false), 2000)
      setEditingWorkflowId(null)
    } catch (error) {
      const message = error instanceof Error ? error.message : '保存失败'
      setToastContent({ title: '保存失败', desc: message })
      setShowToast(true)
      setTimeout(() => setShowToast(false), 3000)
    } finally {
      setSavingId(null)
    }
  }

  const handleCreateWorkflow = () => {
    const draft = createDraftWorkflow()
    setWorkflows((prev) => [draft, ...prev])
    setEditingWorkflowId(draft.id)
  }

  const handleDeleteWorkflow = async (workflowId: string) => {
    try {
      await deleteJson<{ status: string }>(`/api/workflows/${workflowId}`)
      setWorkflows((prev) => prev.filter((workflow) => workflow.id !== workflowId))
      if (editingWorkflowId === workflowId) setEditingWorkflowId(null)
    } catch (error) {
      const message = error instanceof Error ? error.message : '删除失败'
      setToastContent({ title: '删除失败', desc: message })
      setShowToast(true)
      setTimeout(() => setShowToast(false), 3000)
    }
  }

  const handleStopRun = async (runId: string) => {
    try {
      await postJson<{ status: string }, Record<string, never>>(`/api/workflow-runs/${runId}/stop`, {})
      setToastContent({ title: '已发送终止指令', desc: '正在尝试安全停止工作流执行。' })
      setShowToast(true)
      setTimeout(() => setShowToast(false), 2000)
      void loadRuns(runPage, runPageSize, true)
    } catch (error) {
      const message = error instanceof Error ? error.message : '停止失败'
      setToastContent({ title: '停止失败', desc: message })
      setShowToast(true)
      setTimeout(() => setShowToast(false), 3000)
    }
  }

  const exportAccounts = () => {
    if (accounts.length === 0) return
    const link = document.createElement('a')
    link.href = buildApiUrl(`/api/workflow-runs/${selectedRunId || 'latest'}/accounts/export`)
    link.download = `工作流产物_${selectedRunId || 'latest'}.csv`
    document.body.appendChild(link)
    link.click()
    document.body.removeChild(link)
  }

  const copyAccounts = async () => {
    if (accounts.length === 0) return
    const payload = accounts.map((account) => `${account.address},${account.password},${account.status}`).join('\n')
    await navigator.clipboard.writeText(payload)
    setCopiedOutput(true)
    setTimeout(() => setCopiedOutput(false), 1500)
  }

  useEffect(() => {
    void loadRuns(1, runPageSize, false, runStatusFilter, runWorkflowFilter, runWorkflowMatch)
  }, [runPageSize, runStatusFilter, runWorkflowFilter, runWorkflowMatch])

  const totalRunPages = Math.max(1, Math.ceil(runTotal / runPageSize))

  return (
    <div className="page-shell min-w-0 space-y-6 animate-in fade-in slide-in-from-right-4 duration-500 pb-8">
      <div className={`fixed right-10 top-20 z-[100] transform transition-all duration-500 ${showToast ? 'translate-y-0 opacity-100' : '-translate-y-12 pointer-events-none opacity-0'}`}>
        <div className="flex items-center gap-3 rounded-2xl border border-emerald-100 bg-white px-6 py-3 shadow-2xl shadow-emerald-500/10">
          <CheckCircle2 className="text-emerald-500" size={20} />
          <div className="flex flex-col">
            <span className="text-sm font-bold text-slate-800">{toastContent.title}</span>
            <span className="text-[10px] text-slate-500 font-mono">{toastContent.desc}</span>
          </div>
        </div>
      </div>

      <PageHeader
        title=""
        kicker=""
        description=""
        actions={
          <button type="button" onClick={handleCreateWorkflow} className="phantom-btn phantom-btn--primary">
            <Plus size={16} />
            新建工作流
          </button>
        }
      />

      {isLoading ? (
        <div className="page-panel flex min-h-[260px] items-center justify-center rounded-3xl border border-slate-200">
          <div className="flex items-center gap-3 text-slate-600">
            <Loader2 size={18} className="animate-spin" />
            正在同步工作流定义...
          </div>
        </div>
      ) : (
        <div className="grid min-w-0 grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
          {workflows.map((workflow) => (
            <motion.div key={workflow.id} layout className="glass-panel group flex min-w-0 flex-col gap-3 rounded-3xl border border-slate-200 p-4 transition-all hover:border-blue-500/30">
              <div className="flex items-center justify-between">
                <div className="flex h-10 w-10 items-center justify-center rounded-2xl bg-blue-500/10 text-blue-400 shadow-inner transition-all duration-300 group-hover:scale-110 group-hover:rotate-6">
                  <Zap size={19} />
                </div>
                <div className={`rounded-full border px-3 py-1 text-[10px] font-black tracking-widest ${workflow.status === 'active' ? 'border-emerald-500/20 bg-emerald-500/10 text-emerald-600' : 'border-blue-500/20 bg-blue-500/10 text-blue-600'}`}>
                  {workflow.status === 'active' ? '运行中' : '就绪'}
                </div>
              </div>

              <div className="space-y-1">
                <div className="flex items-center gap-2">
                  <h3 className="text-[14px] font-bold text-slate-900 transition-colors group-hover:text-blue-600">{workflow.title}</h3>
                  {workflow.builtin ? <span className="rounded-full bg-slate-200 px-2 py-0.5 text-[10px] font-black tracking-widest text-slate-600">内置</span> : null}
                </div>
                <p className="break-all text-[10px] font-mono text-slate-500">后端标识：{workflow.id}</p>
              </div>

              {editingWorkflowId === workflow.id ? (
                <div className="space-y-3 rounded-2xl border border-slate-200 bg-slate-50 p-4 text-sm text-slate-600">
                  <input value={workflow.title} onChange={(event) => updateWorkflowField(workflow.id, { title: event.target.value })} className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 font-bold text-slate-900 outline-none" />
                  <textarea value={workflow.summary} onChange={(event) => updateWorkflowField(workflow.id, { summary: event.target.value })} className="min-h-[84px] w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-slate-700 outline-none" />
                  <div className="grid grid-cols-3 gap-3">
                    <select
                      value={workflow.kind}
                      onChange={(event) =>
                        updateWorkflowField(workflow.id, {
                          kind: event.target.value as WorkflowKind,
                          parameters: normalizeParametersForKind(event.target.value as WorkflowKind, workflow.parameters),
                        })
                      }
                      disabled={workflow.builtin}
                      className="phantom-select"
                    >
                      <option value="account_generate">账户生成</option>
                      <option value="openai_register">OpenAI 协议注册</option>
                      <option value="openai_register_browser">OpenAI 浏览器模拟注册</option>
                      <option value="data_cleanup">数据清理</option>
                      <option value="status_report">状态报告</option>
                      <option value="environment_check">环境巡检</option>
                    </select>
                    <select value={workflow.status} onChange={(event) => updateWorkflowField(workflow.id, { status: event.target.value as WorkflowDefinition['status'] })} className="phantom-select">
                      <option value="ready">就绪</option>
                      <option value="active">活跃</option>
                      <option value="idle">空闲</option>
                    </select>
                    <input type="number" value={workflow.parameters.batch_size ?? ''} onChange={(event) => updateWorkflowParameters(workflow.id, { batch_size: event.target.value ? Number(event.target.value) : undefined })} disabled={workflow.kind !== 'account_generate' && workflow.kind !== 'openai_register' && workflow.kind !== 'openai_register_browser'} placeholder="批量数量" className="phantom-input" />
                  </div>
                  <input value={workflow.parameters.account_domain ?? ''} onChange={(event) => updateWorkflowParameters(workflow.id, { account_domain: event.target.value || undefined })} disabled={workflow.kind !== 'account_generate'} placeholder="账户域名" className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 outline-none" />
                  {workflow.kind === 'data_cleanup' ? <input type="number" value={workflow.parameters.days_to_keep ?? ''} onChange={(event) => updateWorkflowParameters(workflow.id, { days_to_keep: event.target.value ? Number(event.target.value) : undefined })} placeholder="保留天数" className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 outline-none" /> : null}
                  {workflow.kind === 'status_report' ? <input type="number" value={workflow.parameters.report_window_hours ?? ''} onChange={(event) => updateWorkflowParameters(workflow.id, { report_window_hours: event.target.value ? Number(event.target.value) : undefined })} placeholder="统计窗口小时数" className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 outline-none" /> : null}
                  {workflow.kind === 'openai_register' || workflow.kind === 'openai_register_browser' ? (
                    <div className="grid grid-cols-1 gap-3">
                      <input value={workflow.parameters.proxy_url ?? ''} onChange={(event) => updateWorkflowParameters(workflow.id, { proxy_url: event.target.value || undefined })} placeholder="全局代理 (如 http://127.0.0.1:10809)" className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 outline-none text-sm" />
                      <input value={workflow.parameters.captcha_key ?? ''} onChange={(event) => updateWorkflowParameters(workflow.id, { captcha_key: event.target.value || undefined })} placeholder="打码平台 API Key" className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 outline-none text-sm" />
                      <div className="grid grid-cols-2 gap-3">
                        <input value={workflow.parameters.cpa_url ?? ''} onChange={(event) => updateWorkflowParameters(workflow.id, { cpa_url: event.target.value || undefined })} placeholder="账号分发 URL (CPA/NewAPI)" className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 outline-none text-sm" />
                        <input value={workflow.parameters.cpa_key ?? ''} onChange={(event) => updateWorkflowParameters(workflow.id, { cpa_key: event.target.value || undefined })} placeholder="分发密钥 (可选)" className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 outline-none text-sm" />
                      </div>
                    </div>
                  ) : null}
                  {workflow.kind === 'environment_check' ? (
                    <div className="grid grid-cols-1 gap-3">
                      <ToggleField label="检查环境密钥是否一致" checked={workflow.parameters.require_env_secret_match ?? true} onChange={(checked) => updateWorkflowParameters(workflow.id, { require_env_secret_match: checked })} />
                      <ToggleField label="检查公网中枢地址" checked={workflow.parameters.require_public_hub_url ?? true} onChange={(checked) => updateWorkflowParameters(workflow.id, { require_public_hub_url: checked })} />
                      <ToggleField label="检查 Webhook 配置" checked={workflow.parameters.require_webhook ?? false} onChange={(checked) => updateWorkflowParameters(workflow.id, { require_webhook: checked })} />
                    </div>
                  ) : null}
                  <button type="button" onClick={() => void handleSaveWorkflow(workflow)} disabled={savingId === workflow.id} className="phantom-btn phantom-btn--primary w-full">
                    {savingId === workflow.id ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}
                    {savingId === workflow.id ? '保存中...' : '保存定义'}
                  </button>
                </div>
              ) : (
                <div className="rounded-2xl border border-slate-200 bg-slate-50 p-2.5 text-[12px] leading-relaxed text-slate-600">
                  <div className="line-clamp-2">{workflow.summary}</div>
                  <div className="mt-2 flex flex-wrap gap-2 text-[10px] font-mono text-slate-500">
                    <span className="rounded-full bg-white px-2 py-1">类型={translateWorkflowKind(workflow.kind)}</span>
                    {workflow.parameters.batch_size ? <span className="rounded-full bg-white px-2 py-1">批量={workflow.parameters.batch_size}</span> : null}
                    {workflow.parameters.account_domain ? <span className="rounded-full bg-white px-2 py-1">域名={workflow.parameters.account_domain}</span> : null}
                    {workflow.parameters.days_to_keep ? <span className="rounded-full bg-white px-2 py-1">天数={workflow.parameters.days_to_keep}</span> : null}
                    {workflow.parameters.report_window_hours ? <span className="rounded-full bg-white px-2 py-1">窗口={workflow.parameters.report_window_hours}小时</span> : null}
                  </div>
                </div>
              )}

              <div className="mt-auto grid grid-cols-2 gap-1.5">
                <button type="button" onClick={() => setEditingWorkflowId(editingWorkflowId === workflow.id ? null : workflow.id)} className="phantom-btn phantom-btn--sm phantom-btn--muted">
                  {editingWorkflowId === workflow.id ? '取消编辑' : '编辑定义'}
                </button>
                <button type="button" onClick={() => void handleAction(workflow)} disabled={runningId === workflow.id} className="phantom-btn phantom-btn--sm phantom-btn--secondary">
                  {runningId === workflow.id ? <Loader2 size={14} className="animate-spin" /> : <Play size={14} />}
                  {runningId === workflow.id ? '执行中...' : '触发'}
                </button>
              </div>
              <button type="button" onClick={() => void handleDeleteWorkflow(workflow.id)} disabled={workflow.builtin} className="phantom-btn phantom-btn--sm phantom-btn--danger mt-1.5">
                <Trash2 size={14} />
                {workflow.builtin ? '内置保护' : '删除'}
              </button>
            </motion.div>
          ))}
        </div>
      )}

      <AnimatePresence>
        {!isLoading && workflows.length === 0 ? (
          <motion.div initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: 10 }} className="glass-panel rounded-3xl border border-slate-200 p-10 text-center text-slate-500">
            当前没有可执行的工作流定义。
          </motion.div>
        ) : null}
      </AnimatePresence>

      <section className="page-panel rounded-3xl border border-slate-200 p-5">
        <div className="mb-4 flex items-center justify-between">
          <div>
            <h3 className="text-lg font-bold text-slate-900">最近执行记录</h3>
            <p className="text-xs font-mono text-slate-500">工作流运行记录</p>
          </div>
          <span className="rounded-full border border-slate-200 bg-slate-50 px-3 py-1 text-[10px] font-bold tracking-widest text-slate-500">
            当前页 {runs.length} 条 / 总计 {runTotal}
          </span>
        </div>

        <div className="mb-4 grid grid-cols-1 gap-3 md:grid-cols-[160px_1fr_140px_120px]">
          <select value={runStatusFilter} onChange={(event) => { setRunStatusFilter(event.target.value); setRunPage(1) }} className="phantom-select phantom-btn--sm">
            <option value="">全部状态</option>
            <option value="running">运行中</option>
            <option value="success">成功</option>
            <option value="warn">警告</option>
          </select>
          <input value={runWorkflowFilter} onChange={(event) => { setRunWorkflowFilter(event.target.value); setRunPage(1) }} placeholder="筛选工作流标识" className="phantom-input phantom-btn--sm" />
          <select value={runWorkflowMatch} onChange={(event) => { setRunWorkflowMatch(event.target.value as 'fuzzy' | 'exact'); setRunPage(1) }} className="phantom-select phantom-btn--sm">
            <option value="fuzzy">模糊匹配</option>
            <option value="exact">精确匹配</option>
          </select>
          <select value={runPageSize} onChange={(event) => { setRunPageSize(Number(event.target.value) || 20); setRunPage(1) }} className="phantom-select phantom-btn--sm">
            <option value={20}>每页 20 条</option>
            <option value={50}>每页 50 条</option>
            <option value={100}>每页 100 条</option>
          </select>
        </div>

        <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">
          {runs.length === 0 ? (
            <div className="rounded-2xl border border-dashed border-slate-200 p-8 text-center text-sm text-slate-500">暂无工作流执行记录。</div>
          ) : (
            runs.map((run) => (
              <div key={run.id} className="rounded-2xl border border-slate-200 bg-slate-50/60 p-3.5">
                <button type="button" onClick={() => setSelectedRunId(run.id)} className={`w-full rounded-2xl text-left transition-all ${selectedRunId === run.id ? 'ring-2 ring-blue-500/20' : ''}`}>
                  <div className="flex items-center justify-between gap-4">
                    <div>
                      <div className="text-[15px] font-bold text-slate-900">{run.workflow_title}</div>
                      <div className="mt-1 text-[10px] font-mono text-slate-500">运行标识：{run.id}</div>
                    </div>
                    <div className="flex items-center gap-2">
                      {run.status === 'running' && (
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation();
                            void handleStopRun(run.id);
                          }}
                          className="flex h-6 w-6 items-center justify-center rounded-lg bg-red-500/10 text-red-500 transition-colors hover:bg-red-500/20"
                          title="停止运行"
                        >
                          <Square size={12} fill="currentColor" />
                        </button>
                      )}
                      <span className={`rounded-full px-3 py-1 text-[10px] font-black tracking-widest ${statusTone(run.status)}`}>{translateRunStatus(run.status)}</span>
                    </div>
                  </div>
                  <div className="mt-2 text-[13px] leading-relaxed text-slate-600">{run.message}</div>
                  <div className="mt-3 grid grid-cols-2 gap-x-4 gap-y-1 text-[10px] font-mono text-slate-500">
                    <span>开始：{new Date(run.started_at * 1000).toLocaleString()}</span>
                    <span>结束：{run.finished_at ? new Date(run.finished_at * 1000).toLocaleString() : '执行中'}</span>
                  </div>
                </button>
              </div>
            ))
          )}
        </div>

        <div className="mt-4 flex items-center justify-end gap-3">
          <button type="button" onClick={() => void loadRuns(Math.max(1, runPage - 1), runPageSize, false)} disabled={runPage <= 1} className="phantom-btn phantom-btn--sm phantom-btn--secondary">
            上一页
          </button>
          <span className="text-[11px] font-mono text-slate-500">第 {runPage} / {totalRunPages} 页</span>
          <button type="button" onClick={() => void loadRuns(Math.min(totalRunPages, runPage + 1), runPageSize, false)} disabled={runPage >= totalRunPages} className="phantom-btn phantom-btn--sm phantom-btn--secondary">
            下一页
          </button>
        </div>
      </section>

      <section className="page-panel rounded-3xl border border-slate-200 p-5">
        <div className="mb-4 flex items-center justify-between">
          <div>
            <h3 className="text-lg font-bold text-slate-900">执行步骤明细</h3>
            <p className="text-xs font-mono text-slate-500">步骤轨迹</p>
          </div>
          <span className="rounded-full border border-slate-200 bg-slate-50 px-3 py-1 text-[10px] font-bold tracking-widest text-slate-500">
            {selectedRunId ? `运行标识：${selectedRunId}` : '未选择记录'}
          </span>
        </div>

        {!selectedRunId ? (
          <div className="rounded-2xl border border-dashed border-slate-200 p-8 text-center text-sm text-slate-500">选择一条执行记录后可查看步骤详情。</div>
        ) : (isStepsLoading && steps.length === 0) ? (
          <div className="flex min-h-[180px] items-center justify-center text-slate-500">
            <Loader2 size={18} className="mr-3 animate-spin" />
            正在读取步骤详情...
          </div>
        ) : steps.length === 0 ? (
          <div className="rounded-2xl border border-dashed border-slate-200 p-8 text-center text-sm text-slate-500">当前执行记录暂无步骤详情。</div>
        ) : (
          <div ref={stepsContainerRef} className="max-h-[500px] overflow-y-auto pr-2 rounded-2xl">
            <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">
              {steps.map((step) => (
                <div key={step.id} className="rounded-2xl border border-slate-200 bg-slate-50/60 p-3.5">
                  <div className="flex items-center justify-between gap-4">
                    <div className="font-mono text-[15px] font-bold text-slate-900">第 {step.step_index} 步</div>
                    <span className={`rounded-full px-3 py-1 text-[10px] font-black tracking-widest ${stepTone(step.level)}`}>{translateStepLevel(step.level)}</span>
                  </div>
                  <div className="mt-2 text-[13px] leading-relaxed text-slate-600">{step.message}</div>
                  <div className="mt-2 text-[10px] font-mono text-slate-500">{new Date(step.created_at * 1000).toLocaleString()}</div>
                </div>
              ))}
            </div>
          </div>
        )}
      </section>

      <section className="page-panel rounded-3xl border border-slate-200 p-5">
        <div className="mb-4 flex items-center justify-between">
          <div>
            <h3 className="text-lg font-bold text-slate-900">任务产物</h3>
            <p className="text-xs font-mono text-slate-500">工作流输出</p>
          </div>
          <div className="flex items-center gap-2">
            <span className="rounded-full border border-slate-200 bg-slate-50 px-3 py-1 text-[10px] font-bold tracking-widest text-slate-500">{accounts.length} 条产物</span>
            <button type="button" onClick={copyAccounts} disabled={accounts.length === 0} className="phantom-btn phantom-btn--sm phantom-btn--secondary">
              <Copy size={14} />
              {copiedOutput ? '已复制' : '复制'}
            </button>
            <button type="button" onClick={exportAccounts} disabled={accounts.length === 0} className="phantom-btn phantom-btn--sm phantom-btn--secondary">
              <Download size={14} />
              导出
            </button>
          </div>
        </div>

        {accounts.length === 0 ? (
          <div className="rounded-2xl border border-dashed border-slate-200 p-8 text-center text-sm text-slate-500">当前执行记录没有可展示的任务产物。</div>
        ) : (
          <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">
            {accounts.map((account) => (
              <div key={account.id} className="rounded-2xl border border-slate-200 bg-slate-50/60 p-3.5">
                <div className="flex items-center justify-between gap-4">
                  <div className="break-all text-[15px] font-bold text-slate-900">{account.address}</div>
                  <span className="rounded-full bg-blue-500/10 px-3 py-1 text-[10px] font-black tracking-widest text-blue-600">{account.status}</span>
                </div>
                <div className="mt-2 grid grid-cols-[56px_minmax(0,1fr)] gap-x-2 text-[13px] font-mono text-slate-600">
                  <span className="text-slate-400">密码：</span>
                  <span className="break-all">{account.password}</span>
                </div>
                <div className="mt-2 text-[10px] font-mono text-slate-500">{new Date(account.created_at * 1000).toLocaleString()}</div>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  )
}

function normalizeParametersForKind(kind: WorkflowKind, parameters: WorkflowParameters): WorkflowParameters {
  switch (kind) {
    case 'account_generate':
      return {
        batch_size: parameters.batch_size ?? 10,
        account_domain: parameters.account_domain ?? 'phantom.local',
      }
    case 'openai_register':
      return {
        batch_size: parameters.batch_size ?? 1,
        proxy_url: parameters.proxy_url ?? '',
        captcha_key: parameters.captcha_key ?? '',
        cpa_url: parameters.cpa_url ?? '',
        cpa_key: parameters.cpa_key ?? '',
        full_name: parameters.full_name ?? '',
        age: parameters.age,
      }
    case 'data_cleanup':
      return { days_to_keep: parameters.days_to_keep ?? 7 }
    case 'status_report':
      return { report_window_hours: parameters.report_window_hours ?? 24 }
    case 'environment_check':
      return {
        require_env_secret_match: parameters.require_env_secret_match ?? true,
        require_public_hub_url: parameters.require_public_hub_url ?? true,
        require_webhook: parameters.require_webhook ?? false,
      }
    case 'openai_register_browser':
      return {
        batch_size: parameters.batch_size ?? 1,
        proxy_url: parameters.proxy_url ?? '',
        headless: parameters.headless ?? true,
      }
    default:
      return {}
  }
}

function ToggleField({ label, checked, onChange }: { label: string; checked: boolean; onChange: (checked: boolean) => void }) {
  return (
    <label className="flex items-center justify-between rounded-xl border border-slate-200 bg-white px-3 py-2 text-xs font-mono">
      <span>{label}</span>
      <input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />
    </label>
  )
}

function translateWorkflowKind(kind: WorkflowKind) {
  switch (kind) {
    case 'account_generate':
      return '账户生成'
    case 'openai_register':
      return 'OpenAI 协议注册'
    case 'openai_register_browser':
      return 'OpenAI 浏览器模拟注册'
    case 'data_cleanup':
      return '数据清理'
    case 'status_report':
      return '状态报告'
    case 'environment_check':
      return '环境巡检'
  }
}

function translateRunStatus(status: WorkflowRunRecord['status']) {
  switch (status) {
    case 'running':
      return '运行中'
    case 'success':
      return '成功'
    case 'warn':
      return '警告'
    case 'cancelled':
      return '已取消'
  }
}

function translateStepLevel(level: WorkflowStepRecord['level']) {
  switch (level) {
    case 'running':
      return '运行中'
    case 'success':
      return '成功'
    case 'warn':
      return '警告'
    case 'info':
      return '信息'
  }
}

function statusTone(status: WorkflowRunRecord['status']) {
  switch (status) {
    case 'success':
      return 'bg-emerald-500/10 text-emerald-600'
    case 'warn':
      return 'bg-amber-500/10 text-amber-600'
    case 'running':
      return 'bg-blue-500/10 text-blue-600'
    case 'cancelled':
      return 'bg-slate-500/10 text-slate-600'
  }
}

function stepTone(level: WorkflowStepRecord['level']) {
  switch (level) {
    case 'success':
      return 'bg-emerald-500/10 text-emerald-600'
    case 'warn':
      return 'bg-amber-500/10 text-amber-600'
    case 'running':
      return 'bg-blue-500/10 text-blue-600'
    case 'info':
      return 'bg-slate-200 text-slate-600'
  }
}
