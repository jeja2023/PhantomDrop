import { useEffect, useState } from 'react'
import { Shield, CheckCircle2, Loader2, Send, Terminal, Globe, User, Square } from 'lucide-react'
import { motion } from 'framer-motion'
import { fetchJson, postJson } from '../lib/api'
import PageHeader from '../ui/PageHeader'
import type {
  WorkflowDefinition,
  WorkflowRunPageResponse,
  WorkflowRunRecord,
  WorkflowStepRecord,
} from '../types'

type RegistrationPlatform = 'openai' | 'custom'

export default function RegistrationView({ refreshIntervalMs }: { refreshIntervalMs: number }) {
  const [activePlatform, setActivePlatform] = useState<RegistrationPlatform>('openai')
  const [showToast, setShowToast] = useState(false)
  const [toastContent, setToastContent] = useState({ title: '', desc: '' })
  const [runningId, setRunningId] = useState<string | null>(null)
  
  const [runs, setRuns] = useState<WorkflowRunRecord[]>([])
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null)
  const [steps, setSteps] = useState<WorkflowStepRecord[]>([])
  const [isStepsLoading, setIsStepsLoading] = useState(false)

  // OpenAI 专属配置状态 (模拟)
  const [openaiProxy, setOpenaiProxy] = useState('')
  const [concurrency, setConcurrency] = useState(1)
  const [batchSize, setBatchSize] = useState(1)

  const loadWorkflows = async () => {
    try {
      const data = await fetchJson<WorkflowDefinition[]>('/api/workflows')
      const openaiDef = data.find((w) => w.id === 'openai_register_default')
      if (openaiDef && openaiDef.parameters) {
        setOpenaiProxy(openaiDef.parameters.proxy_url || '')
        setConcurrency(openaiDef.parameters.concurrency || 1)
        setBatchSize(openaiDef.parameters.batch_size || 1)
      }
    } catch (error) {
      console.error('Failed to load workflows:', error)
    }
  }

  const loadRuns = async (preserveSelection = true) => {
    try {
      const data = await fetchJson<WorkflowRunPageResponse>(`/api/workflow-runs?page=1&page_size=20&status=running`)
      const registerRuns = data.items.filter((run) => run.workflow_title.includes('注册') || run.workflow_id.includes('register'))

      let finalRuns = [...registerRuns]

      if (preserveSelection && selectedRunId && !registerRuns.some((r) => r.id === selectedRunId)) {
        const allData = await fetchJson<WorkflowRunPageResponse>(`/api/workflow-runs?page=1&page_size=10`)
        allData.items.forEach((item) => {
          if (!finalRuns.some((c) => c.id === item.id) && (item.workflow_title.includes('注册') || item.workflow_id.includes('register'))) {
            finalRuns.push(item)
          }
        })
        finalRuns = finalRuns.slice(0, 20)
      }

      setRuns((prev) => {
        if (JSON.stringify(prev) === JSON.stringify(finalRuns)) return prev
        return finalRuns
      })

      if (!selectedRunId && registerRuns.length > 0) {
        setSelectedRunId(registerRuns[0].id)
      }
    } catch (error) {
      console.error('Failed to load runs:', error)
    }
  }

  const loadSteps = async (runId: string, silent = false) => {
    if (!silent) setIsStepsLoading(true)
    try {
      const data = await fetchJson<WorkflowStepRecord[]>(`/api/workflow-runs/${runId}/steps`)
      setSteps((prev) => {
        if (JSON.stringify(prev) === JSON.stringify(data)) return prev
        return data
      })
    } finally {
      if (!silent) setIsStepsLoading(false)
    }
  }

  useEffect(() => {
    void loadWorkflows()
    void loadRuns(false)
  }, [])

  useEffect(() => {
    const interval = setInterval(() => {
      void loadRuns(true)
    }, refreshIntervalMs)
    return () => clearInterval(interval)
  }, [refreshIntervalMs, selectedRunId])

  useEffect(() => {
    if (!selectedRunId) return
    void loadSteps(selectedRunId)

    const interval = setInterval(() => {
      void loadSteps(selectedRunId, true)
    }, refreshIntervalMs)
    return () => clearInterval(interval)
  }, [selectedRunId, refreshIntervalMs])

  const handleSaveConfig = async () => {
    try {
      const workflows = await fetchJson<WorkflowDefinition[]>('/api/workflows')
      const currentDef = workflows.find((w) => w.id === 'openai_register_default')

      if (currentDef) {
        const updatedParams = {
          ...currentDef.parameters,
          proxy_url: openaiProxy.trim(),
          batch_size: batchSize,
          concurrency: concurrency,
        }

        await postJson<any, any>('/api/workflows/save', {
          id: currentDef.id,
          kind: 'openai_register',
          title: currentDef.title,
          summary: currentDef.summary,
          status: 'ready',
          parameters_json: JSON.stringify(updatedParams),
        })
        setToastContent({ title: '配置已同步', desc: '参数已成功持久化。' })
        setShowToast(true)
        setTimeout(() => setShowToast(false), 2000)
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : '保存失败'
      setToastContent({ title: '保存失败', desc: message })
      setShowToast(true)
      setTimeout(() => setShowToast(false), 3000)
    }
  }

  const handleTrigger = async (workflowId: string) => {
    setRunningId(workflowId)
    setSelectedRunId(null)
    setSteps([])

    try {
      await handleSaveConfig()

      const res = await postJson<{ run_id: string }, { workflow_id: string }>('/api/workflows/trigger', { workflow_id: workflowId })
      setToastContent({ title: '注册指令已下发', desc: `正在进行 ${batchSize} 个账号的注册流程...` })
      setShowToast(true)
      setTimeout(() => setShowToast(false), 3000)

      if (res.run_id) {
        setSelectedRunId(res.run_id)
      }

      void loadRuns(false)
    } catch (error) {}
    finally {
      setRunningId(null)
    }
  }

  const handleStop = async (runId: string) => {
    try {
      await postJson<any, any>(`/api/workflow-runs/${runId}/stop`, {})
      setToastContent({ title: '停止指令已发送', desc: '正在强制终止当前注册流水线...' })
      setShowToast(true)
      setTimeout(() => setShowToast(false), 2000)
      void loadRuns(true)
    } catch (error) {
      console.error('Failed to stop run:', error)
    }
  }

  const activeRun = runs.find(r => r.status === 'running')

  return (
    <div className="flex flex-col h-full min-w-0 space-y-6 animate-in fade-in slide-in-from-right-4 duration-500">
      <div className={`fixed right-10 top-20 z-[100] transform transition-all duration-500 ${showToast ? 'translate-y-0 opacity-100' : '-translate-y-12 pointer-events-none opacity-0'}`}>
        <div className="flex items-center gap-3 rounded-2xl border border-blue-100 bg-white px-6 py-3 shadow-2xl shadow-blue-500/10">
          <CheckCircle2 className="text-blue-500" size={20} />
          <div className="flex flex-col">
            <span className="text-sm font-bold text-slate-800">{toastContent.title}</span>
            <span className="text-[10px] text-slate-500 font-mono">{toastContent.desc}</span>
          </div>
        </div>
      </div>

      <div className="shrink-0">
        <PageHeader title="" kicker="" description="" />

        <div className="flex gap-2 p-1 bg-slate-100 rounded-2xl w-fit mt-2">
          <button
            onClick={() => setActivePlatform('openai')}
            className={`flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-bold transition-all ${activePlatform === 'openai' ? 'bg-white text-blue-600 shadow-sm' : 'text-slate-500 hover:text-slate-700'}`}
          >
            <User size={14} />
            OpenAI 注册
          </button>
          <button disabled className="flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-bold text-slate-400 cursor-not-allowed">
            <Globe size={14} />
            更多平台 (开发中)
          </button>
        </div>
      </div>

      <div className="flex-grow grid grid-cols-1 lg:grid-cols-12 gap-6 min-h-0 pb-4">
        <div className="lg:col-span-5 flex flex-col gap-4 overflow-y-auto pr-2 scrollbar-thin">
          <section className="glass-panel shrink-0 rounded-3xl border border-slate-200 p-5 space-y-4">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-xl bg-blue-600/10 flex items-center justify-center text-blue-600">
                  <Shield size={18} />
                </div>
                <div>
                  <h3 className="text-sm font-bold text-slate-900">注册参数配置</h3>
                  <p className="text-[10px] text-slate-500 font-mono uppercase tracking-wider">Configuration Profile</p>
                </div>
              </div>
              <button onClick={() => void handleSaveConfig()} className="phantom-btn phantom-btn--sm phantom-btn--secondary">
                <CheckCircle2 size={12} />
                保存配置
              </button>
            </div>

            <div className="space-y-3">
              <div className="space-y-1.5">
                <div className="flex items-center justify-between px-1">
                  <label className="text-[11px] font-bold text-slate-700">代理服务器 (Proxy URL)</label>
                  {openaiProxy && (
                    <button onClick={() => setOpenaiProxy('')} className="text-[10px] text-slate-400 hover:text-rose-500 transition-colors">
                      清空输入
                    </button>
                  )}
                </div>
                <div className="relative group/input">
                  <input
                    type="text"
                    placeholder="http://user:pass@host:port"
                    value={openaiProxy}
                    onChange={(e) => setOpenaiProxy(e.target.value)}
                    className="w-full bg-slate-50 border border-slate-200 rounded-xl px-4 py-2.5 text-sm font-mono outline-none focus:border-blue-500 focus:bg-white transition-all shadow-inner"
                  />
                  <div className="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none opacity-0 group-focus-within/input:opacity-100 transition-opacity">
                    <Globe size={14} className="text-blue-500/50" />
                  </div>
                </div>
                <p className="px-1 text-[9px] text-slate-400 italic">支持 http, https, socks5 协议。</p>
              </div>

              <div className="grid grid-cols-3 gap-3">
                <div className="space-y-1.5">
                  <label className="text-[11px] font-bold text-slate-700 ml-1">注册总量</label>
                  <input
                    type="number"
                    min="1"
                    max="50"
                    value={batchSize}
                    onChange={(e) => setBatchSize(Number(e.target.value))}
                    className="w-full bg-slate-50 border border-slate-200 rounded-xl px-3 py-2 text-sm outline-none focus:border-blue-500 transition-colors"
                  />
                </div>
                <div className="space-y-1.5">
                  <label className="text-[11px] font-bold text-slate-700 ml-1">并发线程</label>
                  <select
                    value={concurrency}
                    onChange={(e) => setConcurrency(Number(e.target.value))}
                    className="w-full bg-slate-50 border border-slate-200 rounded-xl px-3 py-2 text-sm outline-none focus:border-blue-500 transition-colors"
                  >
                    {[1, 2, 5, 8, 10].map((n) => (
                      <option key={n} value={n}>
                        {n} 线程
                      </option>
                    ))}
                  </select>
                </div>
                <div className="space-y-1.5">
                  <label className="text-[11px] font-bold text-slate-700 ml-1">注册行为</label>
                  <select className="w-full bg-slate-50 border border-slate-200 rounded-xl px-3 py-2 text-sm outline-none focus:border-blue-500 transition-colors">
                    <option>仅创建</option>
                    <option>创建+Key</option>
                    <option>开启 Plus</option>
                  </select>
                </div>
              </div>
            </div>

            <div className="flex gap-3">
              <button
                onClick={() => handleTrigger('openai_register_default')}
                disabled={!!runningId || !!activeRun}
                className={`flex-grow phantom-btn phantom-btn--primary py-3 rounded-2xl font-bold flex items-center justify-center gap-2 group shadow-xl ${!!runningId || !!activeRun ? 'opacity-50 cursor-not-allowed grayscale' : 'shadow-blue-600/20'}`}
              >
                {runningId ? <Loader2 size={18} className="animate-spin" /> : <Send size={18} className="group-hover:translate-x-1 group-hover:-translate-y-1 transition-transform" />}
                {runningId ? '注册流程初始化中...' : activeRun ? '任务运行中' : '启动 OpenAI 自动化注册'}
              </button>
              
              {activeRun && (
                <button
                  onClick={() => handleStop(activeRun.id)}
                  className="px-6 rounded-2xl bg-rose-50 border border-rose-100 text-rose-600 hover:bg-rose-100 transition-all flex items-center justify-center group"
                  title="强制停止当前任务"
                >
                  <Square size={18} className="fill-rose-600 group-hover:scale-110 transition-transform" />
                  <span className="ml-2 font-bold text-sm">停止</span>
                </button>
              )}
            </div>
          </section>

          <section className="glass-panel flex-grow rounded-3xl border border-slate-200 p-6 flex flex-col min-h-[240px]">
            <h4 className="shrink-0 text-xs font-bold text-slate-800 mb-4 flex items-center gap-2">
              <Terminal size={14} className="text-slate-400" />
              运行队列监控
            </h4>
            <div className="flex-grow overflow-y-auto space-y-3 pr-1 scrollbar-thin">
              {runs.length === 0 ? (
                <div className="h-full flex items-center justify-center text-slate-400 text-xs font-mono border border-dashed border-slate-200 rounded-2xl">
                  暂无活跃注册任务
                </div>
              ) : (
                runs.map(run => (
                  <button
                    key={run.id}
                    onClick={() => setSelectedRunId(run.id)}
                    className={`w-full p-4 rounded-2xl border transition-all text-left ${selectedRunId === run.id ? 'bg-blue-600 border-blue-700 text-white shadow-lg' : 'bg-slate-50 border-slate-100 text-slate-600 hover:border-slate-200'}`}
                  >
                    <div className="flex justify-between items-start mb-1">
                      <span className="text-[11px] font-bold uppercase tracking-tight">{run.workflow_title}</span>
                      <span className={`text-[9px] font-black px-2 py-0.5 rounded-full ${selectedRunId === run.id ? 'bg-white/20 text-white' : 'bg-blue-100 text-blue-600'}`}>
                        {run.status === 'running' ? '进行中' : '已完成'}
                      </span>
                    </div>
                    <div className={`text-[10px] font-mono truncate ${selectedRunId === run.id ? 'text-blue-100' : 'text-slate-400'}`}>
                      ID: {run.id.slice(0, 13)}...
                    </div>
                  </button>
                ))
              )}
            </div>
          </section>
        </div>

        {/* Right Column: Execution Logs */}
        <div className="lg:col-span-7 flex flex-col min-h-0">
          <section className="glass-panel rounded-3xl border border-slate-200 h-full flex flex-col overflow-hidden bg-white shadow-sm transition-shadow hover:shadow-md">
            <div className="p-6 border-b border-slate-100 flex items-center justify-between bg-white/50 backdrop-blur-md shrink-0">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-xl bg-slate-900 flex items-center justify-center text-white">
                  <Terminal size={16} />
                </div>
                <div>
                  <h3 className="text-sm font-bold text-slate-900">实时执行流 (SSE)</h3>
                  <p className="text-[10px] text-slate-500 font-mono">Live Monitoring</p>
                </div>
              </div>
              {selectedRunId && (
                <div className="flex items-center gap-2">
                  <span className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse"></span>
                  <span className="text-[10px] font-bold text-emerald-600 uppercase tracking-widest">Active Link</span>
                </div>
              )}
            </div>

            <div className="flex-grow p-6 font-mono text-xs overflow-y-auto space-y-4 bg-slate-950 scrollbar-thin scroll-smooth min-h-0">
              {!selectedRunId ? (
                <div className="h-full flex flex-col items-center justify-center text-slate-600 gap-4 opacity-40">
                  <div className="relative">
                    <Terminal size={48} strokeWidth={1} />
                    <motion.div 
                      animate={{ opacity: [0, 1, 0] }}
                      transition={{ duration: 2, repeat: Infinity }}
                      className="absolute bottom-1 right-1 w-2 h-4 bg-slate-500"
                    />
                  </div>
                  <p className="text-[11px] tracking-widest font-bold">请选择左侧任务以查看详细日志流</p>
                </div>
              ) : isStepsLoading && steps.length === 0 ? (
                <div className="flex flex-col items-center justify-center h-full gap-3 text-slate-500">
                  <Loader2 className="animate-spin text-blue-500" size={24} />
                  <span className="text-[10px] uppercase font-bold tracking-widest animate-pulse">Loading Logs...</span>
                </div>
              ) : steps.length === 0 ? (
                <div className="text-slate-500 italic flex items-center gap-2">
                  <span className="w-1 h-1 bg-blue-500 rounded-full animate-ping"></span>
                  等待首个事件上报...
                </div>
              ) : (
                steps.map((step) => (
                  <motion.div
                    initial={{ opacity: 0, x: -10 }}
                    animate={{ opacity: 1, x: 0 }}
                    key={step.id}
                    className="flex gap-4 group items-baseline"
                  >
                    <span className="shrink-0 text-slate-700 w-12 text-[10px]">{new Date(step.created_at * 1000).toLocaleTimeString([], { hour12: false })}</span>
                    <span className={`shrink-0 font-black text-[10px] ${
                      step.level === 'success' ? 'text-emerald-500' : 
                      step.level === 'warn' ? 'text-amber-500' : 
                      step.level === 'error' ? 'text-rose-500' : 'text-blue-400'
                    }`}>
                      [{step.level.toUpperCase()}]
                    </span>
                    <span className="text-slate-300 leading-relaxed group-hover:text-white transition-colors antialiased">{step.message}</span>
                  </motion.div>
                ))
              )}
            </div>

            <div className="p-4 bg-slate-900 border-t border-slate-800 flex items-center justify-between shrink-0">
              <span className="text-[10px] text-slate-500 font-mono tracking-tighter">PHANTOM_CORE::REGISTRATION_ENGINE_V1.2</span>
              <div className="flex gap-1.5">
                <div className="w-1 h-1 rounded-full bg-emerald-500/50"></div>
                <div className="w-1 h-1 rounded-full bg-emerald-500/30"></div>
                <div className="w-1 h-1 rounded-full bg-emerald-500/10"></div>
              </div>
            </div>
          </section>
        </div>
      </div>
    </div>
  )
}
