import React, { useCallback, useEffect, useState, useRef } from 'react'
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
  Terminal,
  Globe,
  Activity,
  FolderSync,
  ShieldCheck,
  ExternalLink,
} from 'lucide-react'
import { motion, AnimatePresence } from 'framer-motion'
import { createPortal } from 'react-dom'
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
import { AccountDetailModal } from './InboxCenterView'
import { useClipboard } from '../ui/useClipboard'


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
  const [activeMonitorTab, setActiveMonitorTab] = useState<'steps' | 'outputs'>('steps')
  const [selectedAccount, setSelectedAccount] = useState<GeneratedAccountRecord | null>(null)
  const [oauthFolded, setOauthFolded] = useState(true)
  const copy = useClipboard()
  const copyToClipboard = useCallback((text: string) => {
    const message = text.length > 24 ? '数据已复制到剪贴板' : `已复制 ${text}`
    void copy(text, { title: message, desc: text.length > 24 ? `${text.slice(0, 20)}...` : undefined })
  }, [copy])
  const stepsContainerRef = useRef<HTMLDivElement>(null)

  // 解析日志文本中的快照截图标识，生成可一键弹窗放大查看的超链接
  const renderMessageWithScreenshot = useCallback((message: string, runId: string) => {
    const redacted = redactMessage(message)
    // 联合正则表达式，支持传统的 screenshot_ 标识以及新版的 [点击预览](/debug/snap_xxx) 格式
    const regex = /screenshot_([a-zA-Z0-9_\-\.]+)|\[点击预览\]\(\/debug\/([^)]+)\)/g
    const parts: React.ReactNode[] = []
    let lastIndex = 0
    let match

    while ((match = regex.exec(redacted)) !== null) {
      const matchIndex = match.index
      const snapNameFromScreenshot = match[1]
      const snapNameFromDebug = match[2]
      
      const snapName = snapNameFromDebug || snapNameFromScreenshot

      if (matchIndex > lastIndex) {
        parts.push(redacted.substring(lastIndex, matchIndex))
      }

      // 根据不同的匹配来源构建正确的 API URL 路径
      const fullUrl = snapNameFromDebug 
        ? buildApiUrl(`/debug/${snapName}`)
        : buildApiUrl(`/api/workflow-runs/${runId}/snapshots/${snapName}`)

      parts.push(
        <span
          key={matchIndex}
          onClick={(e) => {
            e.stopPropagation()
            setPreviewUrl(fullUrl)
          }}
          className="text-emerald-400 hover:text-emerald-300 underline cursor-pointer font-black inline-flex items-center gap-0.5 mx-1"
          title="点击放大预览快照"
        >
          <Globe size={11} className="inline animate-pulse" />
          点击预览
        </span>
      )

      lastIndex = regex.lastIndex
    }

    if (lastIndex < redacted.length) {
      parts.push(redacted.substring(lastIndex))
    }

    return parts.length > 0 ? parts : redacted
  }, [setPreviewUrl])

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
    <div className="page-shell relative animate-in fade-in duration-700 flex flex-col h-full min-h-0 overflow-hidden pb-0.5">
      {/* 顶部航母级分类大 Tab 栏 */}
      <div className="flex items-center gap-2 border-b border-slate-200 pb-2 mb-1.5 shrink-0">
        <button
          onClick={() => setActiveTab('register')}
          className={`flex items-center gap-2.5 px-5 py-2 rounded-2xl text-xs font-black tracking-widest uppercase transition-all duration-300 ${
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
          className={`flex items-center gap-2.5 px-5 py-2 rounded-2xl text-xs font-black tracking-widest uppercase transition-all duration-300 ${
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
      <div className="mb-1.5 shrink-0">
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
      <div className="flex-grow flex-shrink flex-1 min-h-0 flex flex-col lg:flex-row gap-4 overflow-hidden bg-slate-50/20 rounded-3xl border border-slate-200/60 p-3.5 shadow-sm">
        
        {/* 左栏（38%）：工作流最近执行历史 (Workflow Runs) */}
        <div className="flex-[1.2] flex flex-col min-w-0 overflow-hidden">
          <div className="flex items-center justify-between border-b border-slate-100 pb-2 mb-3 shrink-0">
            <div className="flex items-center gap-2">
              <span className="h-1.5 w-1.5 rounded-full bg-blue-500 animate-pulse" />
              <span className="text-[10px] font-black text-slate-600 tracking-wider uppercase">调度执行历史 (RUN HISTORY)</span>
            </div>
            <span className="text-[8px] font-mono text-slate-400 font-bold uppercase">TOTAL: {runTotal}</span>
          </div>

          <div className="flex-grow overflow-y-auto custom-scrollbar pr-1 space-y-1.5">
            {runs.length > 0 ? (
              runs.map((run) => {
                const isSelected = selectedRunId === run.id
                
                const statusColorMap: Record<string, string> = {
                  running: 'bg-blue-50 text-blue-600 border-blue-150',
                  success: 'bg-emerald-50 text-emerald-600 border-emerald-150',
                  warn: 'bg-amber-50 text-amber-600 border-amber-150',
                  error: 'bg-rose-50 text-rose-600 border-rose-150',
                  cancelled: 'bg-slate-50 text-slate-500 border-slate-200',
                }
                const statusNameMap: Record<string, string> = {
                  running: '运行中',
                  success: '成功',
                  warn: '警告',
                  error: '错误',
                  cancelled: '已取消',
                }
                const statusTone = statusColorMap[run.status] || 'bg-slate-50 text-slate-500 border-slate-200'
                const statusName = statusNameMap[run.status] || run.status

                return (
                  <div
                    key={run.id}
                    onClick={() => setSelectedRunId(run.id)}
                    className={`cursor-pointer rounded-xl border py-2 px-2.5 transition-all duration-300 flex flex-col gap-1 relative ${
                      isSelected
                        ? 'border-blue-500 bg-blue-50/30 shadow-md shadow-blue-500/5'
                        : 'border-slate-200 bg-white hover:border-slate-300'
                    }`}
                  >
                    {isSelected && (
                      <div className="absolute left-0 top-1/2 -translate-y-1/2 w-0.5 h-6 bg-blue-500 rounded-r-full" />
                    )}

                    <div className="flex items-center justify-between gap-2 shrink-0">
                      <span className="text-[11px] font-black text-slate-800 truncate" title={run.workflow_title}>
                        {run.workflow_title}
                      </span>
                      <span className={`rounded border px-1.5 py-0.5 text-[8px] font-black tracking-wider leading-none shrink-0 ${statusTone}`}>
                        {statusName}
                      </span>
                    </div>

                    <div className="flex items-center justify-between text-[9px] font-mono text-slate-400 font-bold shrink-0 mt-0.5">
                      <div className="flex items-center gap-1.5 truncate">
                        <span>{new Date(run.started_at * 1000).toLocaleString()}</span>
                        <span className="text-slate-200">|</span>
                        <span className="truncate" title={run.id}>ID: {run.id.slice(0, 8)}...</span>
                      </div>
                      {run.status === 'running' && (
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation()
                            void handleAbort(run.id)
                          }}
                          className="text-rose-500 hover:text-rose-700 font-black tracking-widest uppercase shrink-0 flex items-center gap-0.5 bg-rose-50 px-1.5 py-0.5 rounded border border-rose-100 text-[8px] leading-none"
                        >
                          <Square size={8} /> 中止
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
                // 紧凑版暗色控制台终端 (Sleek Dark Terminal Console)
                <div
                  ref={stepsContainerRef}
                  className="flex-grow bg-slate-950 border border-slate-800 rounded-2xl p-3.5 font-mono text-[11px] leading-relaxed shadow-inner overflow-y-auto custom-scrollbar flex flex-col gap-1 pr-1 relative"
                >
                  {isStepsLoading && steps.length === 0 ? (
                    <div className="absolute inset-0 flex items-center justify-center p-8 bg-slate-950/80 backdrop-blur-[2px]">
                      <Loader2 className="animate-spin text-blue-400 mr-2" size={16} />
                      <span className="text-[10px] font-bold text-slate-400">正在调取步骤时间树...</span>
                    </div>
                  ) : steps.length > 0 ? (
                    steps.map((step, idx) => {
                      const levelConfig: Record<string, { label: string; textClass: string }> = {
                        info: { label: '信息', textClass: 'text-sky-400' },
                        success: { label: '成功', textClass: 'text-emerald-400' },
                        warn: { label: '警告', textClass: 'text-amber-400' },
                        error: { label: '错误', textClass: 'text-rose-400 font-bold' },
                        running: { label: '运行', textClass: 'text-blue-400 animate-pulse' },
                        cancelled: { label: '取消', textClass: 'text-slate-500' },
                      }
                      const config = levelConfig[step.level] || { label: step.level.toUpperCase(), textClass: 'text-slate-300' }
                      const timeStr = new Date(step.created_at * 1000).toLocaleTimeString()
                      
                      return (
                        <div key={idx} className="flex items-start gap-2 py-0.5 hover:bg-slate-900/60 px-1.5 rounded transition-colors group/row">
                          {/* 时间戳 */}
                          <span className="text-slate-500 select-none shrink-0 font-medium">[{timeStr}]</span>
                          
                          {/* 步骤索引 */}
                          <span className="text-slate-450 select-none shrink-0 font-semibold">[步 #{step.step_index}]</span>
                          
                          {/* 日志级别 */}
                          <span className={`shrink-0 font-black select-none ${config.textClass}`}>[{config.label}]</span>
                          
                          {/* 消息正文 */}
                          <span className="text-slate-200 break-all whitespace-pre-wrap flex-grow leading-tight selection:bg-slate-800">
                            {renderMessageWithScreenshot(step.message, selectedRunId || '')}
                          </span>
                        </div>
                      )
                    })
                  ) : (
                    <div className="h-full flex flex-col items-center justify-center p-8 text-center text-slate-500 font-bold">
                      当前工作流实例暂无事件步骤流入
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
                        onClick={() => setSelectedAccount(acc)}
                        className="cursor-pointer rounded-2xl border border-slate-200 bg-white p-3.5 shadow-sm hover:border-indigo-300 transition-colors flex flex-col gap-2 relative"
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

      {/* 账号详情 Modal */}
      {selectedAccount && (
        <AccountDetailModal
          account={selectedAccount}
          oauthFolded={oauthFolded}
          setOauthFolded={setOauthFolded}
          onClose={() => setSelectedAccount(null)}
          copyToClipboard={copyToClipboard}
        />
      )}
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

  const [activePlatform, setActivePlatform] = useState<'openai' | 'custom' | 'oauth'>('openai')
  const [runningId, setRunningId] = useState<string | null>(null)
  const [isSaving, setIsSaving] = useState(false)
  const [isProxyModalOpen, setIsProxyModalOpen] = useState(false)

  // 极速注册专属配置状态
  const [openaiProxy, setOpenaiProxy] = useState('')
  const [concurrency, setConcurrency] = useState(1)
  const [batchSize, setBatchSize] = useState(1)
  const [accountType, setAccountType] = useState('free')
  const [headless, setHeadless] = useState(true)

  // OAuth 联合提纯注册状态
  const [oauthPlatform, setOauthPlatform] = useState<'cpa' | 'sub2api'>('cpa')
  const [oauthUrl, setOauthUrl] = useState('')
  const [oauthVerifier, setOauthVerifier] = useState('')
  const [oauthCallbackUrl, setOauthCallbackUrl] = useState('')
  const [isGeneratingUrl, setIsGeneratingUrl] = useState(false)
  const [isExchangingToken, setIsExchangingToken] = useState(false)
  const [externalOauthUrl, setExternalOauthUrl] = useState('')
  const [isParsingExternal, setIsParsingExternal] = useState(false)

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

  // 一键解析并重构外部已有的 OAuth 授权链接
  const handleParseExternalOauthUrl = async (inputUrl: string) => {
    if (!inputUrl.trim()) return
    setIsParsingExternal(true)
    try {
      const urlObj = new URL(inputUrl.trim())
      const params = urlObj.searchParams
      const clientId = params.get('client_id')
      const redirectUri = params.get('redirect_uri')

      if (!clientId) {
        showToast({ title: '解析失败', desc: '授权链接中未检测到 client_id 参数', tone: 'error' })
        setIsParsingExternal(false)
        return
      }

      // 根据 redirect_uri 匹配平台
      let detectedPlatform: 'cpa' | 'sub2api' = 'cpa'
      if (redirectUri) {
        if (redirectUri.includes('localhost:1456') || redirectUri.includes('127.0.0.1:1456')) {
          detectedPlatform = 'sub2api'
        } else if (redirectUri.includes('localhost:1455') || redirectUri.includes('127.0.0.1:1455')) {
          detectedPlatform = 'cpa'
        }
      }
      setOauthPlatform(detectedPlatform)

      // 向后端获取对应平台的全新 PKCE 凭证与 code_challenge
      const res = await fetchJson<{ url: string; code_verifier: string }>(
        `/api/oauth/register-url?platform=${detectedPlatform}`
      )
      const ourUrlObj = new URL(res.url)
      const ourChallenge = ourUrlObj.searchParams.get('code_challenge')
      const ourState = ourUrlObj.searchParams.get('state')

      if (ourChallenge && ourState) {
        // 重构第三方授权链接，替换为本地可闭环兑换的 PKCE 对
        urlObj.searchParams.set('code_challenge', ourChallenge)
        urlObj.searchParams.set('state', ourState)
        urlObj.searchParams.set('prompt', 'login')
        
        const reconstructedUrl = urlObj.toString()
        setOauthUrl(reconstructedUrl)
        setOauthVerifier(res.code_verifier)
        setExternalOauthUrl('')

        showToast({
          title: '外部官方授权链接重构成功！',
          desc: `已为您安全替换为本地托管的 PKCE 验证配对，检测平台已切换为: ${detectedPlatform === 'sub2api' ? 'Sub2API' : 'CPA'}`,
        })
      } else {
        showToast({ title: '参数重构失败', desc: '后端生成的 PKCE 对不完整', tone: 'error' })
      }
    } catch {
      showToast({
        title: '解析链接失败',
        desc: '请输入有效的 auth.openai.com 官方授权链接',
        tone: 'error',
      })
    } finally {
      setIsParsingExternal(false)
    }
  }

  // 生成专属 OAuth 注册链接
  const handleGenerateOauthUrl = async () => {
    setIsGeneratingUrl(true)
    try {
      const res = await fetchJson<{ url: string; code_verifier: string }>(
        `/api/oauth/register-url?platform=${oauthPlatform}`
      )
      setOauthUrl(res.url)
      setOauthVerifier(res.code_verifier)
      showToast({ title: '注册链接已生成', desc: '专属 PKCE 密钥已就绪，请点击打开进行注册。' })
    } catch {
      showToast({ title: '生成注册链接失败', desc: '网络请求超时，请检查后端状态。', tone: 'error' })
    } finally {
      setIsGeneratingUrl(false)
    }
  }

  // 粘贴并一键提纯落库
  const handleExchangeOauthCode = async () => {
    if (!oauthCallbackUrl.trim()) {
      showToast({ title: '参数缺失', desc: '请先粘贴注册成功后的回调链接', tone: 'error' })
      return
    }
    if (!oauthVerifier) {
      showToast({ title: '凭证失效', desc: '请重新生成专属链接以初始化 PKCE', tone: 'error' })
      return
    }

    setIsExchangingToken(true)
    try {
      const res = await postJson<
        { status: string; email: string; account_id: string },
        { callback_url: string; code_verifier: string; platform: string }
      >('/api/oauth/register-exchange', {
        callback_url: oauthCallbackUrl.trim(),
        code_verifier: oauthVerifier,
        platform: oauthPlatform,
      })
      if (res.status === 'success') {
        showToast({
          title: '凭证提纯并落库成功！',
          desc: `已成功捕获 Access Token 链，录入账号: ${res.email}`,
        })
        setOauthCallbackUrl('')
        // 重载
        onLoadWorkflows()
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : '请确认回调链接中 code 的时效性'
      showToast({ title: '令牌提纯交换失败', desc: msg, tone: 'error' })
    } finally {
      setIsExchangingToken(false)
    }
  }

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
        cleaned.headless = activePlatform === 'openai' ? true : !!headless
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

  const focusGlowInputStyle =
    'w-full bg-slate-50 border border-slate-200 rounded-xl px-3 py-1.5 text-xs font-bold outline-none focus:bg-white focus:border-blue-500 focus:ring-4 focus:ring-blue-100 transition-all duration-300 shadow-inner focus:shadow-[0_0_12px_rgba(59,130,246,0.25)]'

  return (
    <div className="glass-panel rounded-3xl p-3 border border-slate-200 bg-white shadow-sm flex flex-col gap-3">
      {/* 模式选择 Tab 与 动作控制栏 */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 border-b border-slate-100 pb-2 mb-1 shrink-0">
        <div className="flex items-center gap-1.5 shrink-0">
          <span className="flex h-1.5 w-1.5 rounded-full bg-emerald-500 animate-ping" />
          <h3 className="text-[10px] font-black uppercase text-slate-700 tracking-wider">
            执行模式 (PLATFORM KINDS)
          </h3>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          {/* 极简代理配置按钮 */}
          {activePlatform !== 'oauth' && (
            <button
              type="button"
              onClick={() => setIsProxyModalOpen(true)}
              className={`flex items-center gap-1.5 px-3 py-1 h-7 rounded-lg text-[9px] font-black uppercase tracking-wider transition-all duration-300 border cursor-pointer shrink-0 ${
                openaiProxy
                  ? 'bg-emerald-50 text-emerald-600 border-emerald-250 hover:bg-emerald-100'
                  : 'bg-slate-50 text-slate-500 border-slate-250 hover:bg-slate-100 hover:text-slate-850'
              }`}
              title={openaiProxy ? `已配置代理: ${maskProxyUrl(openaiProxy)}` : '未配置代理'}
            >
              <Globe
                size={11}
                className={openaiProxy ? 'text-emerald-500 animate-pulse' : 'text-slate-400'}
              />
              {openaiProxy ? '代理已就绪' : '配置代理'}
            </button>
          )}

          {/* 执行模式切换 */}
          <div className="flex items-center gap-1 bg-slate-100 p-0.5 rounded-lg border border-slate-200/60 shadow-inner shrink-0">
            <button
              onClick={() => setActivePlatform('openai')}
              className={`flex items-center gap-1 px-2.5 py-1 rounded-md text-[9px] font-black uppercase tracking-wider transition-all duration-300 cursor-pointer ${
                activePlatform === 'openai'
                  ? 'bg-white text-emerald-600 shadow-sm border border-slate-200/20'
                  : 'text-slate-500 hover:text-slate-800'
              }`}
            >
              🚀 极速协议模式
            </button>
            <button
              onClick={() => setActivePlatform('custom')}
              className={`flex items-center gap-1 px-2.5 py-1 rounded-md text-[9px] font-black uppercase tracking-wider transition-all duration-300 cursor-pointer ${
                activePlatform === 'custom'
                  ? 'bg-white text-emerald-600 shadow-sm border border-slate-200/20'
                  : 'text-slate-500 hover:text-slate-800'
              }`}
            >
              🖥️ 模拟器可视化模式
            </button>
            <button
              onClick={() => setActivePlatform('oauth')}
              className={`flex items-center gap-1 px-2.5 py-1 rounded-md text-[9px] font-black uppercase tracking-wider transition-all duration-300 cursor-pointer ${
                activePlatform === 'oauth'
                  ? 'bg-white text-purple-650 shadow-sm border border-slate-200/20'
                  : 'text-slate-500 hover:text-slate-800'
              }`}
            >
              🔌 OAuth 联合提纯模式
            </button>
          </div>

          {/* 垂直分界线与控制按钮仅在非 oauth 下显示 */}
          {activePlatform !== 'oauth' && (
            <>
              <div className="hidden sm:block w-px h-5 bg-slate-200 mx-1 shrink-0" />
              <button
                onClick={handleSaveConfig}
                disabled={isSaving}
                className="px-3.5 py-1 h-7 rounded-lg border border-slate-250 hover:bg-slate-50 text-slate-655 hover:text-slate-855 transition-all font-black text-[9px] uppercase cursor-pointer shrink-0"
              >
                {isSaving ? '保存中...' : '同步配置'}
              </button>
              <button
                onClick={handleTrigger}
                disabled={Boolean(runningId)}
                className="bg-gradient-to-r from-emerald-500 to-teal-500 hover:from-emerald-600 hover:to-teal-600 text-white font-black shadow-sm h-7 px-4 rounded-xl flex items-center justify-center gap-1 text-[10px] uppercase transition-all active:scale-[0.98] cursor-pointer shrink-0"
              >
                {runningId ? <Loader2 size={11} className="animate-spin" /> : <Play size={11} />}
                触发极速注册
              </button>
            </>
          )}
        </div>
      </div>

      {/* 条件展示表单内容 */}
      {activePlatform === 'oauth' ? (
        <div className="flex flex-col gap-3 bg-purple-50/10 border border-purple-100/30 rounded-2xl p-3 animate-in fade-in duration-300">
          
          {/* 解析外部已有的 OAuth 授权链接 */}
          <div className="border-b border-slate-200/40 pb-3.5 flex flex-col sm:flex-row items-stretch sm:items-end gap-3">
            <div className="flex-1 space-y-1">
              <label className="text-[9px] font-bold text-slate-500 uppercase">
                导入 / 解析外部第三方官方 OAuth 授权链接
              </label>
              <input
                type="text"
                placeholder="直接粘贴以 auth.openai.com 开头的官方授权注册链接 (包含 client_id & code_challenge)"
                value={externalOauthUrl}
                onChange={(e) => setExternalOauthUrl(e.target.value)}
                className="w-full bg-slate-50 border border-slate-200 rounded-xl px-3 py-1.5 text-xs font-bold outline-none focus:bg-white focus:border-purple-500 focus:ring-4 focus:ring-purple-100 transition-all shadow-inner h-8 font-mono"
              />
            </div>
            <button
              onClick={() => void handleParseExternalOauthUrl(externalOauthUrl)}
              disabled={isParsingExternal || !externalOauthUrl.trim()}
              className="bg-gradient-to-r from-purple-500 to-indigo-500 hover:from-purple-600 hover:to-indigo-600 text-white font-black h-8 px-4 rounded-xl flex items-center justify-center gap-1 text-[9px] uppercase transition-all shadow-md shadow-purple-500/10 cursor-pointer self-end shrink-0"
            >
              {isParsingExternal ? (
                <Loader2 size={11} className="animate-spin" />
              ) : (
                <FolderSync size={11} />
              )}
              解析并重构链接
            </button>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-3 items-end">
            <div className="space-y-1">
              <label className="text-[9px] font-bold text-slate-500 uppercase">
                OAuth 接入平台
              </label>
              <select
                value={oauthPlatform}
                onChange={(e) => {
                  setOauthPlatform(e.target.value as 'cpa' | 'sub2api')
                  setOauthUrl('')
                  setOauthVerifier('')
                }}
                className="w-full bg-slate-50 border border-slate-200 rounded-xl px-3 py-1.5 text-xs font-bold outline-none focus:bg-white focus:border-purple-500 transition-all h-8 cursor-pointer"
              >
                <option value="cpa">CPA 平台 (http://localhost:1455)</option>
                <option value="sub2api">Sub2API 平台 (http://localhost:1456)</option>
              </select>
            </div>

            <div className="flex flex-wrap items-center gap-2 md:col-span-2">
              <button
                onClick={handleGenerateOauthUrl}
                disabled={isGeneratingUrl}
                className="bg-gradient-to-r from-purple-500 to-indigo-500 hover:from-purple-600 hover:to-indigo-600 text-white font-black h-8 px-4 rounded-xl flex items-center justify-center gap-1 text-[9px] uppercase transition-all shrink-0 cursor-pointer shadow-md shadow-purple-500/10"
              >
                {isGeneratingUrl ? <Loader2 size={11} className="animate-spin" /> : <Plus size={11} />}
                生成专属注册提纯链接
              </button>

              {oauthUrl && (
                <button
                  onClick={() => {
                    void navigator.clipboard.writeText(oauthUrl)
                    window.open(oauthUrl, '_blank')
                    showToast({
                      title: '链接已复制并打开',
                      desc: '请在官方注册页点击 Sign up 完成注册。',
                    })
                  }}
                  className="bg-slate-900 hover:bg-slate-800 text-white font-black h-8 px-4 rounded-xl flex items-center justify-center gap-1.5 text-[9px] uppercase transition-all shrink-0 cursor-pointer shadow-sm"
                >
                  <ExternalLink size={11} />
                  在新页中注册并提取
                </button>
              )}
            </div>
          </div>

          {oauthVerifier && (
            <div className="flex flex-col sm:flex-row items-stretch sm:items-end gap-3 border-t border-slate-200/40 pt-3 animate-in slide-in-from-top-1 duration-200">
              <div className="flex-1 space-y-1">
                <label className="text-[9px] font-bold text-slate-500 uppercase">
                  粘贴重定向后的完整回调链接 (包含 code 参数)
                </label>
                <input
                  type="text"
                  placeholder="例如: http://localhost:1455/auth/callback?code=xxxx&state=yyyy"
                  value={oauthCallbackUrl}
                  onChange={(e) => setOauthCallbackUrl(e.target.value)}
                  className="w-full bg-slate-50 border border-slate-200 rounded-xl px-3 py-1.5 text-xs font-bold outline-none focus:bg-white focus:border-purple-500 focus:ring-4 focus:ring-purple-100 transition-all shadow-inner h-8 font-mono"
                />
              </div>
              <button
                onClick={handleExchangeOauthCode}
                disabled={isExchangingToken || !oauthCallbackUrl}
                className="bg-gradient-to-r from-emerald-500 to-teal-500 hover:from-emerald-600 hover:to-teal-600 text-white font-black h-8 px-4 rounded-xl flex items-center justify-center gap-1 text-[9px] uppercase transition-all shadow-md shadow-emerald-500/10 cursor-pointer self-end"
              >
                {isExchangingToken ? (
                  <Loader2 size={11} className="animate-spin" />
                ) : (
                  <ShieldCheck size={11} />
                )}
                提取凭证并自动录入账号
              </button>
            </div>
          )}
        </div>
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
          <div className="space-y-1">
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
          <div className="space-y-1">
            <label className="text-[10px] font-bold text-slate-500 uppercase">单批次生成数</label>
            <input
              type="number"
              min="1"
              value={batchSize}
              onChange={(e) => setBatchSize(Math.max(1, Number(e.target.value)))}
              className={focusGlowInputStyle}
            />
          </div>

          <div className="space-y-1">
            <label className="text-[10px] font-bold text-slate-500 uppercase">账号等级</label>
            <select
              value={accountType}
              onChange={(e) => setAccountType(e.target.value)}
              className="w-full bg-slate-50 border border-slate-200 rounded-xl px-4 py-1.5 text-xs font-bold outline-none focus:bg-white focus:border-blue-500 transition-all shadow-inner h-8"
            >
              <option value="free">GPT Free 级别 (免费版)</option>
              <option value="plus">GPT Plus 级别 (高级版)</option>
              <option value="team">GPT Team 团队版</option>
            </select>
          </div>
          <div className="space-y-1">
            <label className="text-[10px] font-bold text-slate-500 uppercase">
              浏览器无头模式 (HEADLESS)
            </label>
            <select
              value={activePlatform === 'openai' ? 'true' : String(headless)}
              disabled={activePlatform === 'openai'}
              onChange={(e) => setHeadless(e.target.value === 'true')}
              className={`w-full bg-slate-50 border border-slate-200 rounded-xl px-4 py-1.5 text-xs font-bold outline-none focus:bg-white focus:border-blue-500 transition-all shadow-inner h-8 ${
                activePlatform === 'openai' ? 'opacity-50 cursor-not-allowed bg-slate-100/50' : ''
              }`}
            >
              {activePlatform === 'openai' ? (
                <option value="true">后台纯净静默 (协议强制)</option>
              ) : (
                <>
                  <option value="true">后台纯净静默 (推荐)</option>
                  <option value="false">弹出可视化浏览器</option>
                </>
              )}
            </select>
          </div>
        </div>
      )}

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
  )
}

// ==========================================
// 4. 自动化工作流设计师参数编辑弹窗组件 (Portal Modal)
// ==========================================
interface WorkflowParamModalProps {
  isOpen: boolean
  onClose: () => void
  workflow: WorkflowDefinition
  onSave: (title: string, summary: string, batchSize: number, accountDomain: string) => Promise<void>
  isSaving: boolean
}

function WorkflowParamModal({
  isOpen,
  onClose,
  workflow,
  onSave,
  isSaving,
}: WorkflowParamModalProps) {
  const [draftTitle, setDraftTitle] = useState('')
  const [draftSummary, setDraftSummary] = useState('')
  const [draftBatchSize, setDraftBatchSize] = useState(1)
  const [draftAccountDomain, setDraftAccountDomain] = useState('')

  useEffect(() => {
    if (isOpen && workflow) {
      setDraftTitle(workflow.title)
      setDraftSummary(workflow.summary)
      setDraftBatchSize(workflow.parameters?.batch_size ?? 1)
      setDraftAccountDomain(workflow.parameters?.account_domain ?? '')
    }
  }, [isOpen, workflow])

  if (!isOpen) return null

  const handleSaveClick = () => {
    void onSave(draftTitle, draftSummary, draftBatchSize, draftAccountDomain)
  }

  const focusGlowInputStyle =
    'w-full bg-slate-50 border border-slate-200 rounded-xl px-3 py-1.5 text-xs font-bold outline-none focus:bg-white focus:border-purple-500 focus:ring-4 focus:ring-purple-100 transition-all duration-300 shadow-inner'

  return createPortal(
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        className="fixed inset-0 z-[10000] flex items-center justify-center bg-slate-950/60 p-4 backdrop-blur-sm"
        onClick={onClose}
      >
        <motion.div
          initial={{ scale: 0.95, opacity: 0, y: 15 }}
          animate={{ scale: 1, opacity: 1, y: 0 }}
          exit={{ scale: 0.95, opacity: 0, y: 15 }}
          className="relative max-w-md w-full bg-white rounded-3xl overflow-hidden shadow-2xl border border-slate-200"
          onClick={(e) => e.stopPropagation()}
        >
          {/* 头部 (Header) */}
          <div className="px-6 py-5 border-b border-slate-100 flex items-center justify-between bg-slate-50/50 backdrop-blur-md">
            <div className="flex items-center gap-3">
              <div className="w-9 h-9 rounded-xl bg-purple-600/10 flex items-center justify-center text-purple-600 shadow-inner">
                <Save size={16} />
              </div>
              <div>
                <h3 className="text-sm font-black text-slate-950">工作流参数编辑</h3>
                <p className="text-[10px] text-slate-500 font-mono uppercase tracking-wider">Param Editor</p>
              </div>
            </div>
            <button
              onClick={onClose}
              className="p-1.5 rounded-lg hover:bg-slate-100 text-slate-400 hover:text-slate-700 transition-all"
            >
              <X size={18} />
            </button>
          </div>

          {/* 表单区域 (Form) */}
          <div className="p-6 space-y-4 max-h-[65vh] overflow-y-auto custom-scrollbar">
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
                className="w-full bg-slate-50 border border-slate-200 rounded-xl px-3 py-1.5 text-xs font-bold outline-none focus:bg-white focus:border-purple-500 focus:ring-4 focus:ring-purple-100 transition-all duration-300 shadow-inner"
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
                    className="w-20 bg-slate-50 border border-slate-200 rounded-lg px-2 py-0.5 text-right text-xs outline-none focus:border-purple-500"
                  />
                </div>

                <div className="flex items-center justify-between text-[11px] font-bold">
                  <span className="text-slate-500">自愈账号所属分组域</span>
                  <input
                    type="text"
                    placeholder="openai.local"
                    value={draftAccountDomain}
                    onChange={(e) => setDraftAccountDomain(e.target.value)}
                    className="w-28 bg-slate-50 border border-slate-200 rounded-lg px-2 py-0.5 text-right text-xs outline-none focus:border-purple-500"
                  />
                </div>
              </div>
            </div>
          </div>

          {/* 底部按钮 (Footer) */}
          <div className="px-6 py-4 bg-slate-50 border-t border-slate-100 flex items-center justify-end gap-3">
            <button
              onClick={onClose}
              className="px-4 py-2 rounded-xl text-xs font-bold bg-white border border-slate-200 text-slate-600 hover:bg-slate-50 hover:text-slate-900 transition-all shadow-sm"
            >
              放弃修改
            </button>
            <button
              onClick={handleSaveClick}
              disabled={isSaving}
              className="px-4 py-2 rounded-xl text-xs font-bold flex items-center gap-1.5 text-white bg-gradient-to-r from-purple-500 to-indigo-500 hover:from-purple-600 hover:to-indigo-600 transition-all shadow-md shadow-purple-500/10 active:scale-[0.98]"
            >
              {isSaving ? <Loader2 size={13} className="animate-spin" /> : <Save size={13} />}
              持久化配置
            </button>
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>,
    document.body
  )
}

// ==========================================
// 5. 自动化工作流设计师 SubPanel 子面板
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

  const editingWorkflow = workflows.find((w) => w.id === editingWorkflowId) ?? null

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
    } catch {
      /* 忽略 */
    }
    setEditingWorkflowId(draft.id)
    emitLog('开启了新工作流编辑', 'info')
  }

  // 保存工作流编辑
  const handleSave = async (
    id: string,
    def: WorkflowDefinition,
    newTitle: string,
    newSummary: string,
    newBatchSize: number,
    newAccountDomain: string,
  ) => {
    setSavingId(id)
    try {
      const mergedParams = {
        ...def.parameters,
        batch_size: newBatchSize,
        account_domain: newAccountDomain,
      }
      await postJson<{ status: string }, WorkflowSavePayload>('/api/workflows/save', {
        id: def.id,
        kind: def.kind,
        title: newTitle,
        summary: newSummary,
        status: def.status,
        parameters_json: JSON.stringify(mergedParams),
      })
      showToast({ title: '保存成功', desc: `工作流 ${newTitle} 已写入配置库。` })
      emitLog(`保存工作流设计: ${newTitle}`, 'success')
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
      tone: 'warn',
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

  return (
    <div className="w-full flex flex-col min-h-0 bg-white border border-slate-200 rounded-3xl p-3.5 shadow-sm gap-3">
      {/* 头部标题与新建按钮 */}
      <div className="flex items-center justify-between border-b border-slate-100 pb-2 shrink-0">
        <div className="flex items-center gap-2">
          <span className="h-1.5 w-1.5 rounded-full bg-purple-500 animate-pulse" />
          <h3 className="text-[10px] font-black uppercase text-slate-700 tracking-wider">
            可用设计师模板
          </h3>
        </div>
        <button
          onClick={handleCreate}
          className="phantom-btn phantom-btn--primary phantom-btn--sm flex items-center gap-1.5 h-7 min-h-7 rounded-xl shadow-sm text-[9px]"
        >
          <Plus size={11} />
          创建自定义工作流
        </button>
      </div>

      {/* 紧凑的多列响应式卡片网格列表 */}
      <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 xl:grid-cols-4 gap-3 max-h-[300px] overflow-y-auto custom-scrollbar pr-1">
        {workflows.map((workflow) => {
          const colors = kindColors[workflow.kind] || kindColors.account_generate
          const isEditing = editingWorkflowId === workflow.id
          return (
            <motion.div
              key={workflow.id}
              whileHover={{ y: -2 }}
              className={`rounded-2xl border p-3 transition-all duration-300 flex flex-col gap-2 relative overflow-hidden ${
                colors.bg
              } ${colors.border} ${
                isEditing ? 'ring-2 ring-purple-500 border-transparent shadow-lg' : 'shadow-sm'
              }`}
            >
              <div className="flex items-center justify-between shrink-0">
                <span
                  className={`px-1.5 py-0.5 rounded-md border text-[7.5px] font-black leading-none ${colors.badge}`}
                >
                  {kindLabels[workflow.kind]}
                </span>

                {workflow.builtin && (
                  <span className="text-[7px] font-mono font-bold text-slate-400 bg-slate-100 px-1 py-0.5 rounded-md tracking-wider uppercase select-none">
                    BUILTIN_SYS
                  </span>
                )}
              </div>

              <div className="flex flex-col min-w-0">
                <h4
                  className="text-[11px] font-black text-slate-800 tracking-tight leading-none mb-1 truncate"
                  title={workflow.title}
                >
                  {workflow.title}
                </h4>
                <p className="text-[9px] font-medium text-slate-500 leading-normal font-sans line-clamp-1 pr-1">
                  {workflow.summary}
                </p>
              </div>

              <div className="flex items-center justify-between gap-1.5 border-t border-slate-200/40 pt-2 mt-auto shrink-0">
                <div className="flex items-center gap-1">
                  <button
                    onClick={() => setEditingWorkflowId(workflow.id)}
                    className="phantom-btn phantom-btn--secondary phantom-btn--sm h-6 min-h-6 px-2 text-[8px]"
                  >
                    编辑参数
                  </button>
                  {!workflow.builtin && (
                    <button
                      onClick={() => void handleDelete(workflow.id, workflow.title)}
                      className="p-1 rounded-lg text-slate-400 hover:bg-rose-50 hover:text-rose-600 transition-colors"
                      title="删除该自定义工作流"
                    >
                      <Trash2 size={11} />
                    </button>
                  )}
                </div>

                <button
                  onClick={() => onTriggerRun(workflow.id)}
                  className="phantom-btn phantom-btn--sm h-6 min-h-6 px-2.5 bg-gradient-to-r from-purple-500 to-indigo-500 hover:from-purple-600 hover:to-indigo-600 text-white font-black border-transparent shadow shadow-purple-500/10 flex items-center justify-center gap-0.5 text-[8px]"
                >
                  <Play size={8} />
                  立即调度
                </button>
              </div>
            </motion.div>
          )
        })}
      </div>

      {/* 参数配置 Portal Modal */}
      {editingWorkflow && (
        <WorkflowParamModal
          isOpen={!!editingWorkflow}
          onClose={() => setEditingWorkflowId(null)}
          workflow={editingWorkflow}
          onSave={(t, s, b, d) =>
            handleSave(editingWorkflow.id, editingWorkflow, t, s, b, d)
          }
          isSaving={savingId === editingWorkflow.id}
        />
      )}

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
