import { useCallback, useEffect, useState, useRef } from 'react'
import {
  Zap,
  Play,
  Loader2,
  Save,
  Plus,
  Trash2,
  Copy,
  Square,
  X,
  ChevronLeft,
  ChevronRight,
  Shield,
  CheckCircle2,
  Terminal,
  Globe,
  Activity,
  FolderSync,
} from 'lucide-react'
import { motion } from 'framer-motion'
import { buildApiUrl, deleteJson, fetchJson, postJson } from '../lib/api'
import { maskProxyUrl, redactMessage } from '../lib/utils'
import type {
  GeneratedAccountRecord,
  WorkflowDefinition,
  WorkflowRunPageResponse,
  WorkflowRunRecord,
  WorkflowStepRecord,
  LogLevel,
} from '../types'
import SnapshotModal from '../ui/SnapshotModal'
import ProxyModal from '../ui/ProxyModal'
import { RunStatusBadge, StepStatusBadge } from '../ui/StatusBadge'

import { useToast } from '../ui/Toast'
import ConfirmModal from '../ui/ConfirmModal'

// 工作流保存请求体
interface WorkflowSavePayload {
  id: string
  kind: string
  title: string
  summary: string
  status: string
  parameters_json: string
}

// ==========================================
// 1. 辅助构建 Runs 查询参数
// ==========================================
function buildRunQuery(
  page: number,
  pageSize: number,
  status: string,
  workflowId: string,
  match: 'fuzzy' | 'exact',
) {
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

function emitLog(msg: string, level: LogLevel = 'info') {
  window.dispatchEvent(new CustomEvent('phantom-log', { detail: { msg, level } }))
}

// ==========================================
// 2. 主大一统自动化中心大 View
// ==========================================
export default function AutomationHubView({ refreshIntervalMs }: { refreshIntervalMs: number }) {
  const showToast = useToast()
  
  // 顶层 Tab
  const [activeTab, setActiveTab] = useState<'register' | 'workflows'>('register')

  // 工作区状态
  const [workflows, setWorkflows] = useState<WorkflowDefinition[]>([])
  const [runs, setRuns] = useState<WorkflowRunRecord[]>([])
  const [runTotal, setRunTotal] = useState(0)
  const [runPage, setRunPage] = useState(1)
  const [runPageSize, setRunPageSize] = useState(10) // 压缩为每页10条
  const [_runStatusFilter, _setRunStatusFilter] = useState('')
  const [_runWorkflowFilter, _setRunWorkflowFilter] = useState('')
  const [_runWorkflowMatch, _setRunWorkflowMatch] = useState<'fuzzy' | 'exact'>('fuzzy')
  
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null)
  const [steps, setSteps] = useState<WorkflowStepRecord[]>([])
  const [accounts, setAccounts] = useState<GeneratedAccountRecord[]>([])
  const [isStepsLoading, setIsStepsLoading] = useState(false)
  const [previewUrl, setPreviewUrl] = useState<string | null>(null)
  const [_copiedOutput, _setCopiedOutput] = useState(false)
  const [activeMonitorTab, setActiveMonitorTab] = useState<'steps' | 'snapshots' | 'outputs'>('steps')

  const stepsContainerRef = useRef<HTMLDivElement>(null)

  // 1. 加载所有工作流
  const loadWorkflows = async () => {
    try {
      const data = await fetchJson<WorkflowDefinition[]>('/api/workflows')
      setWorkflows(data)
    } catch (error) {
      console.error('Failed to load workflows:', error)
    }
  }

  // 2. 加载 Runs 运行历史（共享监控大盘）
  const loadRuns = useCallback(async (
    page = runPage,
    pageSize = runPageSize,
    preserveSelection = true,
    status = _runStatusFilter,
    workflowId = _runWorkflowFilter,
    match = _runWorkflowMatch,
  ): Promise<WorkflowRunRecord[]> => {
    try {
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
    } catch (error) {
      console.error('Failed to load runs:', error)
      return []
    }
  }, [runPage, runPageSize, _runStatusFilter, _runWorkflowFilter, _runWorkflowMatch])

  // 3. 加载运行步骤
  const loadSteps = useCallback(async (runId: string, silent = false) => {
    if (!silent) setIsStepsLoading(true)
    try {
      const data = await fetchJson<WorkflowStepRecord[]>(`/api/workflow-runs/${runId}/steps`)
      setSteps(data)
    } catch (error) {
      console.error('Failed to load steps:', error)
    } finally {
      if (!silent) setIsStepsLoading(false)
    }
  }, [])

  // 4. 加载生成的账号
  const loadAccounts = useCallback(async (runId: string) => {
    try {
      const data = await fetchJson<GeneratedAccountRecord[]>(`/api/workflow-runs/${runId}/accounts`)
      setAccounts(data)
    } catch (error) {
      console.error('Failed to load accounts:', error)
    }
  }, [])

  // 初始加载
  useEffect(() => {
    void loadWorkflows()
    void loadRuns(1, 10, false)
  }, [])

  // 轮询 Runs 列表状态
  useEffect(() => {
    const interval = setInterval(() => {
      void loadRuns(runPage, runPageSize, true)
    }, refreshIntervalMs)
    return () => clearInterval(interval)
  }, [loadRuns, refreshIntervalMs, runPage, runPageSize])

  // 监控当前选中的 Run 步骤和账号
  useEffect(() => {
    if (!selectedRunId) return
    void loadSteps(selectedRunId, true)
    void loadAccounts(selectedRunId)

    const interval = setInterval(() => {
      void loadSteps(selectedRunId, true)
      void loadAccounts(selectedRunId)
    }, refreshIntervalMs)
    return () => clearInterval(interval)
  }, [loadSteps, loadAccounts, selectedRunId, refreshIntervalMs])

  // 步骤自动置底
  useEffect(() => {
    if (stepsContainerRef.current) {
      stepsContainerRef.current.scrollTop = stepsContainerRef.current.scrollHeight
    }
  }, [steps])

  // 中断执行
  const handleAbort = async (runId: string) => {
    try {
      await postJson<{ status: string }, Record<string, never>>(`/api/workflow-runs/${runId}/abort`, {})
      showToast({ title: '已发送中止信号', desc: `正在强行终止运行进程: ${runId}` })
      emitLog(`终止工作流运行指令下发: ${runId}`, 'warn')
      void loadRuns()
    } catch (error) {
      const msg = error instanceof Error ? error.message : '终止失败'
      showToast({ title: '中止请求失败', desc: msg })
    }
  }

  // 触发指定工作流运行
  const triggerWorkflowRun = async (workflowId: string) => {
    try {
      const res = await postJson<{ run_id: string }, { workflow_id: string }>('/api/workflows/trigger', { workflow_id: workflowId })
      showToast({ title: '触发指令已送达', desc: '自动化调度引擎已安排线程介入。' })
      emitLog(`调度触发工作流: ${workflowId}`, 'success')
      if (res.run_id) {
        setSelectedRunId(res.run_id)
        setActiveMonitorTab('steps')
      }
      void loadRuns(1, 10, false)
    } catch (error) {
      const msg = error instanceof Error ? error.message : '启动失败'
      showToast({ title: '触发失败', desc: msg })
    }
  }

  return (
    <div className="page-shell relative animate-in fade-in duration-700 flex flex-col min-h-full pb-8">
      {/* 顶部航母级分类大 Tab 栏 */}
      <div className="flex items-center gap-2 border-b border-slate-200 pb-3 mb-5 shrink-0">
        <button
          onClick={() => setActiveTab('register')}
          className={`flex items-center gap-2.5 px-5 py-2.5 rounded-2xl text-xs font-black tracking-widest uppercase transition-all duration-300 ${
            activeTab === 'register'
              ? 'bg-emerald-600 text-white shadow-lg shadow-emerald-500/20 border-transparent'
              : 'bg-slate-100 hover:bg-slate-200 text-slate-500 border border-slate-200/50'
          }`}
        >
          <Zap size={14} />
          ⚡ 极速注册中心 (OpenAI Register)
        </button>

        <button
          onClick={() => setActiveTab('workflows')}
          className={`flex items-center gap-2.5 px-5 py-2.5 rounded-2xl text-xs font-black tracking-widest uppercase transition-all duration-300 ${
            activeTab === 'workflows'
              ? 'bg-purple-600 text-white shadow-lg shadow-purple-500/20 border-transparent'
              : 'bg-slate-100 hover:bg-slate-200 text-slate-500 border border-slate-200/50'
          }`}
        >
          <FolderSync size={14} />
          🔧 自动化工作流设计师 (Workflow Designer)
        </button>
      </div>

      {/* 工作表单与配置 */}
      <div className="mb-6 shrink-0">
        {activeTab === 'register' ? (
          <RegistrationSubPanel
            workflows={workflows}
            onLoadWorkflows={loadWorkflows}
            onTriggerRun={triggerWorkflowRun}
            refreshIntervalMs={refreshIntervalMs}
          />
        ) : (
          <WorkflowDesignerSubPanel
            workflows={workflows}
            onLoadWorkflows={loadWorkflows}
            onTriggerRun={triggerWorkflowRun}
          />
        )}
      </div>

      {/* 底部大一统：自动化运行历史与日志步骤时间轴监控沙盘 */}
      <div className="flex-grow flex flex-col lg:flex-row gap-6 min-h-[420px] overflow-hidden bg-slate-50/20 rounded-3xl border border-slate-200/60 p-5 shadow-sm">
        
        {/* 左栏（38%）：工作流最近执行历史 (Workflow Runs) */}
        <div className="flex-[1.2] flex flex-col min-w-0 overflow-hidden">
          <div className="flex items-center justify-between border-b border-slate-100 pb-2 mb-3 shrink-0">
            <div className="flex items-center gap-2">
              <span className="h-1.5 w-1.5 rounded-full bg-blue-500 animate-pulse" />
              <span className="text-[10px] font-black text-slate-600 tracking-wider uppercase">调度执行历史 (RUN HISTORY)</span>
            </div>
            <span className="text-[8px] font-mono text-slate-400 font-bold uppercase">TOTAL: {runTotal}</span>
          </div>

          <div className="flex-grow overflow-y-auto custom-scrollbar pr-1 space-y-2">
            {runs.length > 0 ? (
              runs.map((run) => {
                const isSelected = selectedRunId === run.id
                return (
                  <div
                    key={run.id}
                    onClick={() => setSelectedRunId(run.id)}
                    className={`cursor-pointer rounded-2xl border p-3.5 transition-all duration-300 flex flex-col gap-2 relative ${
                      isSelected
                        ? 'border-blue-500 bg-blue-50/30 shadow-md shadow-blue-500/5'
                        : 'border-slate-200 bg-white hover:border-slate-350'
                    }`}
                  >
                    {isSelected && (
                      <div className="absolute left-0 top-1/2 -translate-y-1/2 w-1 h-8 bg-blue-500 rounded-r-full" />
                    )}

                    <div className="flex items-center justify-between">
                      <div className="flex flex-col min-w-0">
                        <span className="text-[11px] font-black text-slate-800 truncate" title={run.workflow_title}>
                          {run.workflow_title}
                        </span>
                        <span className="text-[8px] font-mono text-slate-450 tracking-tight mt-0.5" title={run.id}>
                          RUN_ID: {run.id.slice(0, 16)}...
                        </span>
                      </div>
                      <RunStatusBadge status={run.status} />
                    </div>

                    <div className="flex items-center justify-between text-[9px] font-mono text-slate-400 font-bold border-t border-slate-100/60 pt-2 shrink-0">
                      <span>{new Date(run.started_at * 1000).toLocaleString()}</span>
                      {run.status === 'running' && (
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation()
                            void handleAbort(run.id)
                          }}
                          className="text-rose-500 hover:text-rose-700 font-black tracking-widest uppercase shrink-0 flex items-center gap-1 bg-rose-50 px-2 py-0.5 rounded-lg border border-rose-100"
                        >
                          <Square size={10} /> 强行终止
                        </button>
                      )}
                    </div>
                  </div>
                )
              })
            ) : (
              <div className="h-full flex flex-col items-center justify-center p-8 text-center text-slate-400 font-bold border border-dashed border-slate-200 rounded-2xl">
                暂无自动化调度执行记录
              </div>
            )}
          </div>

          {/* Runs 翻页 */}
          <div className="flex items-center justify-between mt-3 pt-2 border-t border-slate-100 shrink-0">
            <button
              onClick={() => void loadRuns(Math.max(1, runPage - 1), runPageSize, true)}
              disabled={runPage <= 1}
              className="phantom-btn phantom-btn--secondary phantom-btn--sm h-7 min-h-7 px-2.5 rounded-xl text-[10px]"
            >
              <ChevronLeft size={12} />
            </button>
            <span className="text-[10px] font-bold text-slate-500">
              {runPage} / {totalPages(runTotal, runPageSize)} 页
            </span>
            <button
              onClick={() => void loadRuns(Math.min(totalPages(runTotal, runPageSize), runPage + 1), runPageSize, true)}
              disabled={runPage >= totalPages(runTotal, runPageSize)}
              className="phantom-btn phantom-btn--secondary phantom-btn--sm h-7 min-h-7 px-2.5 rounded-xl text-[10px]"
            >
              <ChevronRight size={12} />
            </button>
          </div>
        </div>

        {/* 垂直分界线 */}
        <div className="hidden lg:block w-px bg-slate-200/80 shrink-0 self-stretch my-2" />

        {/* 右栏（62%）：同屏步骤详情时间轴、快照、输出 (Run Monitor Console) */}
        <div className="flex-[2] flex flex-col min-w-0 overflow-hidden">
          {/* 大盘头部 Tab 切换 */}
          <div className="flex items-center justify-between border-b border-slate-100 pb-2 mb-3 shrink-0">
            <div className="flex items-center gap-1 bg-slate-100/80 p-0.5 rounded-xl border border-slate-200 shadow-sm shrink-0">
              <button
                onClick={() => setActiveMonitorTab('steps')}
                className={`flex items-center gap-1.5 px-3 py-1 rounded-lg text-[10px] font-bold tracking-wider transition-all duration-300 ${
                  activeMonitorTab === 'steps' ? 'bg-white text-slate-800 shadow-sm' : 'text-slate-500 hover:text-slate-800'
                }`}
              >
                <Terminal size={11} /> 步骤轨迹
              </button>
              <button
                onClick={() => setActiveMonitorTab('snapshots')}
                className={`flex items-center gap-1.5 px-3 py-1 rounded-lg text-[10px] font-bold tracking-wider transition-all duration-300 ${
                  activeMonitorTab === 'snapshots' ? 'bg-white text-slate-800 shadow-sm' : 'text-slate-500 hover:text-slate-800'
                }`}
              >
                <Globe size={11} /> 运行快照
              </button>
              <button
                onClick={() => setActiveMonitorTab('outputs')}
                className={`flex items-center gap-1.5 px-3 py-1 rounded-lg text-[10px] font-bold tracking-wider transition-all duration-300 ${
                  activeMonitorTab === 'outputs' ? 'bg-white text-slate-800 shadow-sm' : 'text-slate-500 hover:text-slate-800'
                }`}
              >
                <Copy size={11} /> 资产产出 ({accounts.length})
              </button>
            </div>
            
            <span className="text-[8px] font-mono text-slate-400 font-bold truncate max-w-[200px]" title={selectedRunId || ''}>
              MONITORING: {selectedRunId ? selectedRunId.slice(0, 16) + '...' : 'NONE'}
            </span>
          </div>

          {/* 内容区 */}
          {selectedRunId ? (
            <div className="flex-grow flex flex-col min-h-0 relative">
              {activeMonitorTab === 'steps' ? (
                // 虚线步骤轨迹时间轴生命树 (Timeline Trace)
                <div
                  ref={stepsContainerRef}
                  className="flex-grow overflow-y-auto custom-scrollbar pr-1 relative pl-5 space-y-4 py-2"
                >
                  {/* 垂直连接虚线主干 */}
                  <div className="absolute left-6 top-4 bottom-4 w-px border-l border-dashed border-slate-200 pointer-events-none" />

                  {isStepsLoading && steps.length === 0 ? (
                    <div className="absolute inset-0 flex items-center justify-center p-8">
                      <Loader2 className="animate-spin text-blue-500 mr-2" size={16} />
                      <span className="text-[10px] font-bold text-slate-500">正在调取步骤时间树...</span>
                    </div>
                  ) : steps.length > 0 ? (
                    steps.map((step, idx) => {
                      const colorMap: Record<string, string> = {
                        info: 'bg-blue-500 shadow-blue-500/30',
                        success: 'bg-emerald-500 shadow-emerald-500/30',
                        warn: 'bg-amber-500 shadow-amber-500/30',
                        error: 'bg-rose-500 shadow-rose-500/30',
                        running: 'bg-blue-500 shadow-blue-500/30',
                        cancelled: 'bg-slate-500 shadow-slate-500/30',
                      }
                      const dotColor = colorMap[step.level] || 'bg-slate-500 shadow-slate-500/30'
                      return (
                        <div key={idx} className="relative flex items-start gap-4 animate-in fade-in duration-300">
                          {/* 时间轴精致小节点 */}
                          <div className={`relative z-10 w-2.5 h-2.5 rounded-full mt-1.5 shrink-0 shadow-[0_0_8px_rgba(0,0,0,0.15)] transition-all ${dotColor} animate-pulse`} />
                          
                          <div className="flex flex-col flex-grow bg-white border border-slate-200/80 rounded-2xl p-3 shadow-sm hover:border-slate-300 transition-all">
                            <div className="flex items-center justify-between border-b border-slate-50 pb-1.5 mb-1.5 shrink-0">
                              <span className="text-[10px] font-mono font-bold text-slate-400">步骤 #{step.step_index}</span>
                              <StepStatusBadge level={step.level} />
                            </div>
                            <p className="text-[11px] font-bold text-slate-700 leading-relaxed break-words font-sans">
                              {redactMessage(step.message)}
                            </p>
                            <span className="text-[8px] font-mono text-slate-400 font-bold text-right mt-1">
                              {new Date(step.created_at * 1000).toLocaleTimeString()}
                            </span>
                          </div>
                        </div>
                      )
                    })
                  ) : (
                    <div className="h-full flex flex-col items-center justify-center p-8 text-center text-slate-400 font-bold">
                      当前工作流实例暂无事件步骤流入
                    </div>
                  )}
                </div>
              ) : activeMonitorTab === 'snapshots' ? (
                // 运行快照预览
                <div className="flex-grow overflow-y-auto custom-scrollbar pr-1 flex flex-col gap-4">
                  {steps.some((s) => s.message.includes('screenshot_')) ? (
                    <div className="grid grid-cols-2 gap-3 p-1">
                      {steps
                        .filter((s) => s.message.includes('screenshot_'))
                        .map((step, idx) => {
                          const match = /screenshot_([a-zA-Z0-9_\-\.]+)/.exec(step.message)
                          const snapName = match ? match[1] : null
                          if (!snapName) return null
                          const fullUrl = buildApiUrl(`/api/workflow-runs/${selectedRunId}/snapshots/${snapName}`)
                          return (
                            <div
                              key={idx}
                              onClick={() => setPreviewUrl(fullUrl)}
                              className="group cursor-pointer rounded-2xl border border-slate-200 overflow-hidden bg-white shadow-sm hover:shadow-md hover:border-blue-400 transition-all relative aspect-video"
                            >
                              <img src={fullUrl} alt={`快照 ${idx}`} className="w-full h-full object-cover transition-transform duration-500 group-hover:scale-105" />
                              <div className="absolute inset-0 bg-slate-900/60 opacity-0 group-hover:opacity-100 flex items-center justify-center transition-all duration-300">
                                <span className="text-[10px] text-white font-black tracking-widest uppercase flex items-center gap-1">
                                  <Globe size={12} /> 放大检视快照
                                </span>
                              </div>
                              <div className="absolute bottom-2 left-2 bg-slate-900/70 backdrop-blur-sm rounded-lg px-2 py-0.5 text-[8px] font-mono font-bold text-slate-350 pointer-events-none">
                                STEP #{step.step_index}
                              </div>
                            </div>
                          )
                        })}
                    </div>
                  ) : (
                    <div className="flex-grow flex flex-col items-center justify-center p-8 text-center text-slate-400 font-bold border border-dashed border-slate-200 rounded-2xl">
                      未捕获到运行视觉快照 (无头/协议模式不保存快照)
                    </div>
                  )}
                </div>
              ) : (
                // 资产产出 (Generated Accounts)
                <div className="flex-grow overflow-y-auto custom-scrollbar pr-1 flex flex-col gap-3">
                  {accounts.length > 0 ? (
                    accounts.map((acc) => (
                      <div
                        key={acc.id}
                        className="rounded-2xl border border-slate-200 bg-white p-3.5 shadow-sm hover:border-indigo-300 transition-colors flex flex-col gap-2 relative"
                      >
                        <div className="flex items-center justify-between shrink-0">
                          <span className="font-mono text-[11px] font-black text-indigo-700 select-all leading-none">{acc.address}</span>
                          <span className="px-2 py-0.5 rounded-lg border border-emerald-100 bg-emerald-50 text-emerald-600 text-[8px] font-black leading-none">
                            已录入
                          </span>
                        </div>
                        <div className="text-[8px] font-mono text-slate-400 font-bold leading-none select-all truncate">
                          API_KEY: {acc.session_token || acc.access_token || '---'}
                        </div>
                      </div>
                    ))
                  ) : (
                    <div className="flex-grow flex flex-col items-center justify-center p-8 text-center text-slate-400 font-bold border border-dashed border-slate-200 rounded-2xl">
                      当前执行实例尚无高可用账号资产产出
                    </div>
                  )}
                </div>
              )}
            </div>
          ) : (
            <div className="flex-grow flex flex-col items-center justify-center p-8 text-center bg-white border border-dashed border-slate-200 rounded-2xl">
              <Activity className="text-slate-300 animate-bounce mb-3" size={28} />
              <h4 className="text-xs font-black uppercase text-slate-700 tracking-wider">选择执行实例</h4>
              <p className="text-[10px] font-bold text-slate-400 max-w-[240px] leading-relaxed mt-1">
                请在左侧运行记录中任意点击一个实例，此处将立刻同屏展示事件时间树、快照与数据资产。
              </p>
            </div>
          )}
        </div>

      </div>

      {/* 快照预览 Modal */}
      {previewUrl && <SnapshotModal url={previewUrl} onClose={() => setPreviewUrl(null)} />}
    </div>
  )
}

function totalPages(total: number, size: number) {
  return Math.max(1, Math.ceil(total / size))
}

// ==========================================
// 3. OpenAI 并发极速注册 SubPanel 子面板
// ==========================================
interface RegistrationSubPanelProps {
  workflows: WorkflowDefinition[]
  onLoadWorkflows: () => void
  onTriggerRun: (id: string) => void
  refreshIntervalMs: number
}

function RegistrationSubPanel({
  workflows,
  onLoadWorkflows,
  onTriggerRun,
  refreshIntervalMs: _refreshIntervalMs,
}: RegistrationSubPanelProps) {
  const showToast = useToast()
  
  const [activePlatform, setActivePlatform] = useState<'openai' | 'custom'>('openai')
  const [runningId, setRunningId] = useState<string | null>(null)
  const [isSaving, setIsSaving] = useState(false)
  const [showProxyRaw, setShowProxyRaw] = useState(false)
  const [isProxyModalOpen, setIsProxyModalOpen] = useState(false)

  // 极速注册专属配置状态
  const [openaiProxy, setOpenaiProxy] = useState('')
  const [concurrency, setConcurrency] = useState(1)
  const [batchSize, setBatchSize] = useState(1)
  const [accountType, setAccountType] = useState('free')
  const [headless, setHeadless] = useState(true)

  const targetWorkflowId = activePlatform === 'custom' ? 'openai_browser_register' : 'openai_register_default'
  const currentDef = workflows.find((w) => w.id === targetWorkflowId) ?? null

  const loadCurrentConfig = useCallback(() => {
    if (currentDef && currentDef.parameters) {
      setOpenaiProxy(currentDef.parameters.proxy_url || '')
      setConcurrency(currentDef.parameters.concurrency || 1)
      setBatchSize(currentDef.parameters.batch_size || 1)
      setAccountType(currentDef.parameters.account_type || 'free')
      setHeadless(currentDef.parameters.headless !== false)
    }
  }, [currentDef])

  useEffect(() => {
    loadCurrentConfig()
  }, [loadCurrentConfig])

  // 持久化保存
  const handleSaveConfig = async (): Promise<boolean> => {
    if (!currentDef) return false
    setIsSaving(true)
    try {
      const cleanParameters = (params: WorkflowDefinition['parameters']) => {
        const cleaned: WorkflowDefinition['parameters'] = { ...params }
        cleaned.batch_size = Math.floor(Number(batchSize) || 1)
        cleaned.concurrency = Math.floor(Number(concurrency) || 1)
        cleaned.proxy_url = openaiProxy.trim() || undefined
        cleaned.full_name = undefined
        cleaned.account_type = accountType
        cleaned.headless = !!headless
        cleaned.age = undefined
        return cleaned
      }

      await postJson<{ status: string }, WorkflowSavePayload>('/api/workflows/save', {
        id: currentDef.id,
        kind: currentDef.kind,
        title: currentDef.title,
        summary: currentDef.summary,
        status: 'ready',
        parameters_json: JSON.stringify(cleanParameters(currentDef.parameters || {})),
      })
      
      showToast({ title: '注册配置已同步', desc: '极速注册参数已成功持久化至中枢。' })
      emitLog(`已保存工作流参数配置: ${currentDef.id}`, 'success')
      void onLoadWorkflows()
      return true
    } catch (error) {
      const message = error instanceof Error ? error.message : '保存失败'
      showToast({ title: '保存失败', desc: message })
      return false
    } finally {
      setIsSaving(false)
    }
  }

  // 触发指令下发
  const handleTrigger = async () => {
    setRunningId(targetWorkflowId)
    try {
      const saved = await handleSaveConfig()
      if (saved) {
        onTriggerRun(targetWorkflowId)
      }
    } finally {
      setRunningId(null)
    }
  }

  const focusGlowInputStyle = "w-full bg-slate-50 border border-slate-200 rounded-xl px-4 py-2.5 text-xs font-bold outline-none focus:bg-white focus:border-blue-500 focus:ring-4 focus:ring-blue-100 transition-all duration-300 shadow-inner focus:shadow-[0_0_12px_rgba(59,130,246,0.25)]"

  return (
    <div className="flex flex-col lg:flex-row gap-6 relative">
      {/* 左侧：注册模式选择与参数控制表单 (占比 65%) */}
      <div className="flex-[1.8] glass-panel rounded-3xl p-5 border border-slate-200 bg-white shadow-sm flex flex-col gap-4">
        
        {/* 模式选择 Tab */}
        <div className="flex items-center justify-between border-b border-slate-100 pb-3 shrink-0">
          <div className="flex items-center gap-2">
            <span className="flex h-2 w-2 rounded-full bg-emerald-500 animate-ping" />
            <h3 className="text-xs font-black uppercase text-slate-700 tracking-wider">执行模式 (PLATFORM KINDS)</h3>
          </div>

          <div className="flex items-center gap-1.5 bg-slate-100 p-0.5 rounded-xl border border-slate-200/60 shadow-inner">
            <button
              onClick={() => setActivePlatform('openai')}
              className={`flex items-center gap-1 px-4 py-1.5 rounded-lg text-[10px] font-black uppercase tracking-wider transition-all duration-300 ${
                activePlatform === 'openai' ? 'bg-white text-emerald-600 shadow-sm' : 'text-slate-500 hover:text-slate-800'
              }`}
            >
              🚀 极速协议模式
            </button>
            <button
              onClick={() => setActivePlatform('custom')}
              className={`flex items-center gap-1 px-4 py-1.5 rounded-lg text-[10px] font-black uppercase tracking-wider transition-all duration-300 ${
                activePlatform === 'custom' ? 'bg-white text-emerald-600 shadow-sm' : 'text-slate-500 hover:text-slate-800'
              }`}
            >
              🖥️ 模拟器可视化模式
            </button>
          </div>
        </div>

        {/* 表单项 */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div className="space-y-1.5">
            <label className="text-[10px] font-bold text-slate-500 uppercase">并发执行代理 URL (PROXY_NODE)</label>
            <div className="relative">
              <input
                type={showProxyRaw ? 'text' : 'password'}
                placeholder="socks5://user:pass@host:port (选填)"
                value={openaiProxy}
                onChange={(e) => setOpenaiProxy(e.target.value)}
                className={`${focusGlowInputStyle} pr-20 font-mono`}
              />
              <div className="absolute right-2 top-1/2 -translate-y-1/2 flex gap-1 h-7">
                <button
                  type="button"
                  onClick={() => setShowProxyRaw(!showProxyRaw)}
                  className="px-2 rounded-lg bg-slate-200/60 text-slate-500 hover:bg-slate-200 transition-colors text-[9px] font-bold uppercase"
                >
                  {showProxyRaw ? '隐藏' : '明文'}
                </button>
                <button
                  type="button"
                  onClick={() => setIsProxyModalOpen(true)}
                  className="px-2 rounded-lg bg-blue-50 text-blue-600 hover:bg-blue-600 hover:text-white transition-colors text-[9px] font-bold uppercase"
                >
                  管理
                </button>
              </div>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1.5">
              <label className="text-[10px] font-bold text-slate-500 uppercase">并发线程数</label>
              <input
                type="number"
                min="1"
                max="50"
                value={concurrency}
                onChange={(e) => setConcurrency(Math.max(1, Number(e.target.value)))}
                className={focusGlowInputStyle}
              />
            </div>
            <div className="space-y-1.5">
              <label className="text-[10px] font-bold text-slate-500 uppercase">单批次生成数</label>
              <input
                type="number"
                min="1"
                value={batchSize}
                onChange={(e) => setBatchSize(Math.max(1, Number(e.target.value)))}
                className={focusGlowInputStyle}
              />
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1.5">
              <label className="text-[10px] font-bold text-slate-500 uppercase">账号等级</label>
              <select
                value={accountType}
                onChange={(e) => setAccountType(e.target.value)}
                className="w-full bg-slate-50 border border-slate-200 rounded-xl px-4 py-2.5 text-xs font-bold outline-none focus:bg-white focus:border-blue-500 transition-all shadow-inner h-9"
              >
                <option value="free">GPT Free 级别 (免费版)</option>
                <option value="plus">GPT Plus 级别 (高级版)</option>
                <option value="team">GPT Team 团队版</option>
              </select>
            </div>
            <div className="space-y-1.5">
              <label className="text-[10px] font-bold text-slate-500 uppercase">浏览器无头模式 (HEADLESS)</label>
              <div className="flex items-center justify-between border border-slate-200 bg-slate-50 rounded-xl px-4 h-9 shadow-inner">
                <span className="text-[11px] font-bold text-slate-500">
                  {headless ? '后台纯净静默 (推荐)' : '弹出可视化浏览器'}
                </span>
                <input
                  type="checkbox"
                  checked={headless}
                  onChange={(e) => setHeadless(e.target.checked)}
                  className="h-4 w-4 text-blue-600 border-slate-350 focus:ring-blue-100 rounded cursor-pointer"
                />
              </div>
            </div>
          </div>
        </div>

        {/* 代理池管理 Modal */}
        <ProxyModal
          isOpen={isProxyModalOpen}
          value={openaiProxy}
          onChange={(p: string) => {
            setOpenaiProxy(p)
            setIsProxyModalOpen(false)
          }}
          onClose={() => setIsProxyModalOpen(false)}
        />
      </div>

      {/* 右侧：翡翠绿呼吸流光启动卡片 (占比 35%) */}
      <div className="flex-grow flex-[1] glass-panel rounded-3xl p-5 border border-emerald-100 bg-emerald-50/10 flex flex-col relative overflow-hidden group/trigger min-h-[220px]">
        {/* 四角呼吸流光线，富含 WoW 级仪式感 */}
        <div className="absolute inset-0 border border-emerald-500/20 rounded-3xl pointer-events-none group-hover/trigger:border-emerald-500/40 transition-all duration-500" />
        <div className="absolute top-0 left-0 w-24 h-[1px] bg-gradient-to-r from-transparent via-emerald-400 to-transparent animate-pulse" />
        <div className="absolute bottom-0 right-0 w-24 h-[1px] bg-gradient-to-r from-transparent via-emerald-400 to-transparent animate-pulse" />

        <div className="relative z-10 flex flex-col flex-grow">
          <div className="flex items-center gap-2 border-b border-emerald-100/50 pb-3 mb-4 shrink-0">
            <div className="w-8 h-8 rounded-lg bg-emerald-500/10 text-emerald-600 flex items-center justify-center shadow-inner">
              <Shield size={16} />
            </div>
            <div>
              <h4 className="text-xs font-black text-emerald-800 leading-none mb-1">
                中枢极速执行终端
              </h4>
              <span className="font-mono text-[8px] text-emerald-600/70 tracking-widest leading-none uppercase">
                ENGINE CONTROLLER
              </span>
            </div>
          </div>

          <div className="text-[11px] font-bold text-emerald-700/80 leading-relaxed space-y-2.5 mb-5 flex-grow font-sans pr-1">
            <p className="flex items-center gap-2">
              <CheckCircle2 size={13} className="text-emerald-500" /> 
              已就绪工作流：<span className="font-black text-emerald-900">{currentDef?.title || '未配置'}</span>
            </p>
            <p className="flex items-center gap-2">
              <CheckCircle2 size={13} className="text-emerald-500" /> 
              平台执行机制：<span className="font-black text-emerald-900">{activePlatform === 'openai' ? '协议极速注册' : '模拟器可视化'}</span>
            </p>
            <p className="flex items-center gap-2">
              <CheckCircle2 size={13} className="text-emerald-500" /> 
              代理节点掩码：<span className="font-mono font-black text-emerald-900 truncate max-w-[150px]">{openaiProxy ? maskProxyUrl(openaiProxy) : '直连模式'}</span>
            </p>
          </div>

          <div className="grid grid-cols-2 gap-3 shrink-0">
            <button
              onClick={handleSaveConfig}
              disabled={isSaving}
              className="phantom-btn phantom-btn--secondary hover:bg-emerald-50/50 hover:text-emerald-700 border-emerald-200/50 font-black h-11 transition-all rounded-2xl text-xs"
            >
              {isSaving ? '保存中...' : '同步配置'}
            </button>

            <button
              onClick={handleTrigger}
              disabled={Boolean(runningId)}
              className="phantom-btn bg-gradient-to-r from-emerald-500 to-teal-500 hover:from-emerald-600 hover:to-teal-600 text-white border-transparent font-black shadow-lg shadow-emerald-500/25 h-11 transition-all rounded-2xl flex items-center justify-center gap-1.5 text-xs active:scale-[0.98]"
            >
              {runningId ? <Loader2 size={14} className="animate-spin" /> : <Play size={14} />}
              触发极速注册
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}

// ==========================================
// 4. 自动化工作流设计师 SubPanel 子面板
// ==========================================
interface WorkflowDesignerSubPanelProps {
  workflows: WorkflowDefinition[]
  onLoadWorkflows: () => void
  onTriggerRun: (id: string) => void
}

function WorkflowDesignerSubPanel({
  workflows,
  onLoadWorkflows,
  onTriggerRun,
}: WorkflowDesignerSubPanelProps) {
  const showToast = useToast()
  
  const [editingWorkflowId, setEditingWorkflowId] = useState<string | null>(null)
  const [confirmConfig, setConfirmConfig] = useState<{
    title: string
    message: string
    tone?: 'danger' | 'info' | 'warn'
    onConfirm: () => void
  } | null>(null)
  const [savingId, setSavingId] = useState<string | null>(null)
  
  // 局部编辑草稿状态（替代不可用的 setWorkflows 直接修改父级数据）
  const [draftTitle, setDraftTitle] = useState('')
  const [draftSummary, setDraftSummary] = useState('')
  const [draftBatchSize, setDraftBatchSize] = useState(1)
  const [draftAccountDomain, setDraftAccountDomain] = useState('')

  const editingWorkflow = workflows.find((w) => w.id === editingWorkflowId) ?? null

  // 当切换到编辑模式时，将当前工作流的值灌入草稿
  useEffect(() => {
    if (editingWorkflow) {
      setDraftTitle(editingWorkflow.title)
      setDraftSummary(editingWorkflow.summary)
      setDraftBatchSize(editingWorkflow.parameters?.batch_size ?? 1)
      setDraftAccountDomain(editingWorkflow.parameters?.account_domain ?? '')
    }
  }, [editingWorkflowId])

  const kindColors: Record<string, { bg: string; border: string; text: string; badge: string }> = {
    openai_register: {
      bg: 'bg-emerald-50/30 hover:bg-emerald-50/50',
      border: 'border-emerald-100 hover:border-emerald-300',
      text: 'text-emerald-700',
      badge: 'border-emerald-100 bg-emerald-50 text-emerald-600',
    },
    openai_register_browser: {
      bg: 'bg-teal-50/30 hover:bg-teal-50/50',
      border: 'border-teal-100 hover:border-teal-300',
      text: 'text-teal-700',
      badge: 'border-teal-100 bg-teal-50 text-teal-600',
    },
    account_generate: {
      bg: 'bg-blue-50/30 hover:bg-blue-50/50',
      border: 'border-blue-100 hover:border-blue-300',
      text: 'text-blue-700',
      badge: 'border-blue-100 bg-blue-50 text-blue-600',
    },
    data_cleanup: {
      bg: 'bg-rose-50/30 hover:bg-rose-50/50',
      border: 'border-rose-100 hover:border-rose-300',
      text: 'text-rose-700',
      badge: 'border-rose-100 bg-rose-50 text-rose-600',
    },
    status_report: {
      bg: 'bg-amber-50/30 hover:bg-amber-50/50',
      border: 'border-amber-100 hover:border-amber-300',
      text: 'text-amber-700',
      badge: 'border-amber-100 bg-amber-50 text-amber-600',
    },
    environment_check: {
      bg: 'bg-purple-50/30 hover:bg-purple-50/50',
      border: 'border-purple-100 hover:border-purple-300',
      text: 'text-purple-700',
      badge: 'border-purple-100 bg-purple-50 text-purple-600',
    },
  }

  const kindLabels: Record<string, string> = {
    openai_register: '协议注册任务',
    openai_register_browser: '浏览器注册任务',
    account_generate: '账户生产调度',
    data_cleanup: '数据净化清洗',
    status_report: '状态巡检报告',
    environment_check: '环境校验预警',
  }

  // 快捷复制
  // 快捷复制（预留功能）
  // const handleCopy = async (text: string) => { ... }

  // 新建空工作流
  const createDraftWorkflow = (): WorkflowDefinition => ({
    id: `workflow_${Date.now()}`,
    kind: 'account_generate',
    title: '新工作流任务',
    summary: '待补充说明详情',
    status: 'ready',
    builtin: false,
    parameters: {
      batch_size: 10,
      account_domain: 'phantom.local',
    },
  })

  // 创建并开启编辑
  const handleCreate = async () => {
    const draft = createDraftWorkflow()
    try {
      await postJson<{ status: string }, WorkflowSavePayload>('/api/workflows/save', {
        id: draft.id,
        kind: draft.kind,
        title: draft.title,
        summary: draft.summary,
        status: draft.status,
        parameters_json: JSON.stringify(draft.parameters || {}),
      })
      void onLoadWorkflows()
    } catch { /* 忽略 */ }
    setEditingWorkflowId(draft.id)
    emitLog('开启了新工作流编辑', 'info')
  }

  // 保存工作流编辑
  const handleSave = async (id: string, def: WorkflowDefinition) => {
    setSavingId(id)
    try {
      const mergedParams = { ...def.parameters, batch_size: draftBatchSize, account_domain: draftAccountDomain }
      await postJson<{ status: string }, WorkflowSavePayload>('/api/workflows/save', {
        id: def.id,
        kind: def.kind,
        title: draftTitle,
        summary: draftSummary,
        status: def.status,
        parameters_json: JSON.stringify(mergedParams),
      })
      showToast({ title: '保存成功', desc: `工作流 ${draftTitle} 已写入配置库。` })
      emitLog(`保存工作流设计: ${draftTitle}`, 'success')
      setEditingWorkflowId(null)
      void onLoadWorkflows()
    } catch (error) {
      const msg = error instanceof Error ? error.message : '保存失败'
      showToast({ title: '保存失败', desc: msg })
    } finally {
      setSavingId(null)
    }
  }

  // 删除工作流
  const handleDelete = (id: string, title: string) => {
    setConfirmConfig({
      title: '删除工作流设计',
      message: `确定要永久删除工作流 "${title}" 吗？该操作无法恢复。`,
      tone: 'danger',
      onConfirm: async () => {
        setConfirmConfig(null)
        try {
          await deleteJson<{ status: string }>(`/api/workflows/${id}`)
          showToast({ title: '已删除工作流', desc: title })
          emitLog(`删除了工作流设计: ${title}`, 'warn')
          setEditingWorkflowId(null)
          void onLoadWorkflows()
        } catch (error) {
          const msg = error instanceof Error ? error.message : '删除失败'
          showToast({ title: '删除失败', desc: msg })
        }
      },
    })
  }

  const focusGlowInputStyle = "w-full bg-slate-50 border border-slate-200 rounded-xl px-4 py-2.5 text-xs font-bold outline-none focus:bg-white focus:border-blue-500 focus:ring-4 focus:ring-blue-100 transition-all duration-300 shadow-inner focus:shadow-[0_0_12px_rgba(59,130,246,0.25)]"

  return (
    <div className="flex flex-col lg:flex-row gap-6 relative">
      {/* 左侧：工作流卡片网格列表 (占比 65%) */}
      <div className="flex-[1.8] flex flex-col min-w-0 bg-white border border-slate-200 rounded-3xl p-5 shadow-sm gap-4">
        <div className="flex items-center justify-between border-b border-slate-100 pb-3 shrink-0">
          <div className="flex items-center gap-2">
            <span className="h-1.5 w-1.5 rounded-full bg-purple-500 animate-pulse" />
            <h3 className="text-xs font-black uppercase text-slate-700 tracking-wider">可用设计师模板</h3>
          </div>
          <button
            onClick={handleCreate}
            className="phantom-btn phantom-btn--primary phantom-btn--sm flex items-center gap-1.5 h-8 min-h-8 rounded-xl shadow-sm text-[10px]"
          >
            <Plus size={12} />
            创建自定义工作流
          </button>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 max-h-[360px] overflow-y-auto custom-scrollbar pr-1">
          {workflows.map((workflow) => {
            const colors = kindColors[workflow.kind] || kindColors.account_generate
            const isEditing = editingWorkflowId === workflow.id
            return (
              <motion.div
                key={workflow.id}
                whileHover={{ y: -3 }}
                className={`rounded-2xl border p-4 min-h-[150px] transition-all duration-300 flex flex-col gap-3 relative overflow-hidden ${colors.bg} ${colors.border} ${
                  isEditing ? 'ring-2 ring-purple-500 border-transparent shadow-lg' : 'shadow-sm'
                }`}
              >
                <div className="flex items-center justify-between shrink-0">
                  <span className={`px-2 py-0.5 rounded-lg border text-[8px] font-black leading-none ${colors.badge}`}>
                    {kindLabels[workflow.kind]}
                  </span>
                  
                  {workflow.builtin && (
                    <span className="text-[7.5px] font-mono font-bold text-slate-400 bg-slate-100 px-1.5 py-0.5 rounded-lg tracking-widest uppercase select-none">
                      BUILTIN_SYS
                    </span>
                  )}
                </div>

                <div className="flex flex-col min-w-0">
                  <h4 className="text-xs font-black text-slate-800 tracking-tight leading-none mb-1.5 truncate" title={workflow.title}>
                    {workflow.title}
                  </h4>
                  <p className="text-[10px] font-bold text-slate-500 leading-relaxed font-sans line-clamp-2 pr-1">
                    {workflow.summary}
                  </p>
                </div>

                <div className="flex items-center justify-between gap-2 border-t border-slate-200/50 pt-3 mt-auto shrink-0">
                  <div className="flex items-center gap-1.5">
                    <button
                      onClick={() => setEditingWorkflowId(workflow.id)}
                      disabled={isEditing}
                      className="phantom-btn phantom-btn--secondary phantom-btn--sm h-7 min-h-7 px-2 text-[9px]"
                    >
                      编辑参数
                    </button>
                    {!workflow.builtin && (
                      <button
                        onClick={() => void handleDelete(workflow.id, workflow.title)}
                        className="p-1.5 rounded-xl text-slate-400 hover:bg-rose-50 hover:text-rose-600 transition-colors"
                        title="删除该自定义工作流"
                      >
                        <Trash2 size={13} />
                      </button>
                    )}
                  </div>

                  <button
                    onClick={() => onTriggerRun(workflow.id)}
                    className={`phantom-btn phantom-btn--sm h-7 min-h-7 px-3 bg-gradient-to-r from-purple-500 to-indigo-500 hover:from-purple-600 hover:to-indigo-600 text-white font-black border-transparent shadow-md shadow-purple-500/10 flex items-center justify-center gap-1 text-[9px]`}
                  >
                    <Play size={10} />
                    立即调度
                  </button>
                </div>
              </motion.div>
            )
          })}
        </div>
      </div>

      {/* 右侧：工作流参数配置抽屉表单 (占比 35%) */}
      <div className="flex-grow flex-[1] glass-panel rounded-3xl p-5 border border-purple-100 bg-purple-50/10 flex flex-col min-h-[220px]">
        {editingWorkflow ? (
          <div className="flex flex-col flex-grow relative z-10">
            <div className="flex items-center justify-between border-b border-purple-100/50 pb-3 mb-4 shrink-0">
              <div className="flex items-center gap-2">
                <div className="w-8 h-8 rounded-lg bg-purple-500/10 text-purple-600 flex items-center justify-center shadow-inner">
                  <Save size={16} />
                </div>
                <div>
                  <h4 className="text-xs font-black text-purple-800 leading-none mb-1">工作流参数编辑</h4>
                  <span className="font-mono text-[8px] text-purple-600/70 tracking-widest leading-none uppercase">PARAM EDITOR</span>
                </div>
              </div>
              
              <button
                onClick={() => setEditingWorkflowId(null)}
                className="p-1 rounded-lg hover:bg-slate-200 text-slate-400 hover:text-slate-700 transition-colors"
                title="放弃编辑"
              >
                <X size={14} />
              </button>
            </div>

            <div className="flex-grow overflow-y-auto pr-1 space-y-4 max-h-[280px] custom-scrollbar mb-4">
              <div className="space-y-1.5">
                <label className="text-[10px] font-bold text-slate-500 uppercase">工作流标题</label>
                <input
                  type="text"
                  value={draftTitle}
                  onChange={(e) => setDraftTitle(e.target.value)}
                  className={focusGlowInputStyle}
                />
              </div>

              <div className="space-y-1.5">
                <label className="text-[10px] font-bold text-slate-500 uppercase">工作流说明摘要</label>
                <textarea
                  value={draftSummary}
                  onChange={(e) => setDraftSummary(e.target.value)}
                  rows={2}
                  className="w-full bg-slate-50 border border-slate-200 rounded-xl px-4 py-2 text-xs font-bold outline-none focus:bg-white focus:border-blue-500 focus:ring-4 focus:ring-blue-100 transition-all duration-300 shadow-inner"
                />
              </div>

              {/* 核心调度参数配置 */}
              <div className="rounded-2xl border border-slate-200 bg-white p-4 shadow-sm space-y-3.5">
                <span className="text-[9px] font-black text-purple-600 tracking-wider uppercase border-b border-slate-100 pb-1.5 block">
                  核心调度变量 (SCHEDULER VARS)
                </span>
                
                <div className="space-y-2">
                  <div className="flex items-center justify-between text-[11px] font-bold">
                    <span className="text-slate-500">单批次处理容量 (batch_size)</span>
                    <input
                      type="number"
                      min="1"
                      value={draftBatchSize}
                      onChange={(e) => setDraftBatchSize(Math.max(1, Number(e.target.value)))}
                      className="w-20 bg-slate-50 border border-slate-200 rounded-lg px-2 py-1 text-right text-xs outline-none focus:border-blue-500"
                    />
                  </div>

                  <div className="flex items-center justify-between text-[11px] font-bold">
                    <span className="text-slate-500">自愈账号所属分组域</span>
                    <input
                      type="text"
                      placeholder="openai.local"
                      value={draftAccountDomain}
                      onChange={(e) => setDraftAccountDomain(e.target.value)}
                      className="w-28 bg-slate-50 border border-slate-200 rounded-lg px-2 py-1 text-right text-xs outline-none focus:border-blue-500"
                    />
                  </div>
                </div>
              </div>
            </div>

            <div className="grid grid-cols-2 gap-3 shrink-0 mt-auto">
              <button
                onClick={() => setEditingWorkflowId(null)}
                className="phantom-btn phantom-btn--secondary border-purple-250 h-10 min-h-10 text-xs font-black rounded-2xl"
              >
                放弃修改
              </button>
              <button
                onClick={() => void handleSave(editingWorkflow.id, editingWorkflow)}
                disabled={savingId === editingWorkflow.id}
                className="phantom-btn bg-gradient-to-r from-purple-500 to-indigo-500 text-white border-transparent font-black shadow-lg shadow-purple-500/25 h-10 min-h-10 text-xs rounded-2xl flex items-center justify-center gap-1 active:scale-[0.98]"
              >
                {savingId === editingWorkflow.id ? <Loader2 size={13} className="animate-spin" /> : <Save size={13} />}
                持久化配置
              </button>
            </div>
          </div>
        ) : (
          <div className="flex-1 flex flex-col items-center justify-center p-6 text-center h-full relative select-none">
            <Activity className="text-purple-400 animate-pulse mb-3" size={26} />
            <h4 className="text-xs font-black uppercase text-purple-800 tracking-wider leading-none mb-2">未激活设计师参数</h4>
            <p className="text-[10px] font-bold text-slate-400 max-w-[200px] leading-relaxed">
              请点击左侧卡片的“编辑参数”按键，此处将即刻展开高阶调度变量持久化表单。
            </p>
          </div>
        )}
      </div>

      {confirmConfig && (
        <ConfirmModal
          isOpen={true}
          title={confirmConfig.title}
          message={confirmConfig.message}
          tone={confirmConfig.tone}
          onConfirm={confirmConfig.onConfirm}
          onCancel={() => setConfirmConfig(null)}
        />
      )}
    </div>
  )
}
