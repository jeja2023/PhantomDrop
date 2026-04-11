import { useEffect, useState } from 'react'
import { Shield, CheckCircle2, Loader2, Send, Terminal, Globe, User } from 'lucide-react'
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
  const [openaiThreads, setOpenaiThreads] = useState(1)

  const loadWorkflows = async () => {
    try {
      await fetchJson<WorkflowDefinition[]>('/api/workflows')
    } catch (error) {
      console.error('Failed to load workflows:', error)
    }
  }

  const loadRuns = async (preserveSelection = true) => {
    try {
      const data = await fetchJson<WorkflowRunPageResponse>(`/api/workflow-runs?page=1&page_size=20&status=running`)
      // 过滤出属于注册类型的工作流运行 (通过标题或标识，这里简单筛选)
      const registerRuns = data.items.filter(run => run.workflow_title.includes('注册') || run.workflow_id.includes('register'))
      setRuns(registerRuns)
      
      if (!selectedRunId && registerRuns.length > 0) {
        setSelectedRunId(registerRuns[0].id)
      } else if (preserveSelection && selectedRunId) {
        if (!registerRuns.some(r => r.id === selectedRunId)) {
          // 如果当前选择的不再运行列表中，可能已完成，尝试加载全部记录查询
          const allData = await fetchJson<WorkflowRunPageResponse>(`/api/workflow-runs?page=1&page_size=10`)
          setRuns(() => {
            const combined = [...registerRuns]
            allData.items.forEach(item => {
               if (!combined.some(c => c.id === item.id) && (item.workflow_title.includes('注册') || item.workflow_id.includes('register'))) {
                 combined.push(item)
               }
            })
            return combined.slice(0, 20)
          })
        }
      }
    } catch (error) {
      console.error('Failed to load runs:', error)
    }
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

  const handleTrigger = async (workflowId: string) => {
    setRunningId(workflowId)
    try {
      await postJson('/api/workflows/trigger', { workflow_id: workflowId })
      setToastContent({ title: '注册指令已下发', desc: '正在初始化自动化注册引擎...' })
      setShowToast(true)
      setTimeout(() => setShowToast(false), 3000)
      void loadRuns(false)
    } catch (error) {
      const message = error instanceof Error ? error.message : '启动失败'
      setToastContent({ title: '启动失败', desc: message })
      setShowToast(true)
      setTimeout(() => setShowToast(false), 3000)
    } finally {
      setRunningId(null)
    }
  }

  return (
    <div className="flex flex-col h-full min-w-0 space-y-6 animate-in fade-in slide-in-from-right-4 duration-500">
      {/* Toast Notification */}
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
        <PageHeader
          title=""
          kicker=""
          description=""
        />

        {/* Sub Tabs */}
        <div className="flex gap-2 p-1 bg-slate-100 rounded-2xl w-fit mt-2">
          <button
            onClick={() => setActivePlatform('openai')}
            className={`flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-bold transition-all ${activePlatform === 'openai' ? 'bg-white text-blue-600 shadow-sm' : 'text-slate-500 hover:text-slate-700'}`}
          >
            <User size={14} />
            OpenAI 注册
          </button>
          <button
            disabled
            className="flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-bold text-slate-400 cursor-not-allowed"
          >
            <Globe size={14} />
            更多平台 (开发中)
          </button>
        </div>
      </div>

      <div className="flex-grow grid grid-cols-1 lg:grid-cols-12 gap-6 min-h-0 pb-4">
        {/* Left Column: Configuration */}
        <div className="lg:col-span-5 flex flex-col gap-4 overflow-y-auto pr-2 scrollbar-thin">
          <section className="glass-panel shrink-0 rounded-3xl border border-slate-200 p-5 space-y-4">
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-xl bg-blue-600/10 flex items-center justify-center text-blue-600">
                <Shield size={18} />
              </div>
              <div>
                <h3 className="text-sm font-bold text-slate-900">注册参数配置</h3>
                <p className="text-[10px] text-slate-500 font-mono uppercase tracking-wider">Configuration Profile</p>
              </div>
            </div>

            <div className="space-y-3">
              <div className="space-y-1.5">
                <label className="text-[11px] font-bold text-slate-700 ml-1">代理服务器 (Proxy URL)</label>
                <input
                  type="text"
                  placeholder="http://user:pass@host:port"
                  value={openaiProxy}
                  onChange={e => setOpenaiProxy(e.target.value)}
                  className="w-full bg-slate-50 border border-slate-200 rounded-xl px-4 py-2.5 text-sm font-mono outline-none focus:border-blue-500 transition-colors"
                />
              </div>

              <div className="grid grid-cols-2 gap-4">
                <div className="space-y-1.5">
                  <label className="text-[11px] font-bold text-slate-700 ml-1">并发线程数</label>
                  <select
                    value={openaiThreads}
                    onChange={e => setOpenaiThreads(Number(e.target.value))}
                    className="w-full bg-slate-50 border border-slate-200 rounded-xl px-4 py-2.5 text-sm outline-none focus:border-blue-500 transition-colors"
                  >
                    {[1, 2, 5, 10, 20].map(n => (
                      <option key={n} value={n}>{n} 线程</option>
                    ))}
                  </select>
                </div>
                <div className="space-y-1.5">
                  <label className="text-[11px] font-bold text-slate-700 ml-1">注册行为</label>
                  <select className="w-full bg-slate-50 border border-slate-200 rounded-xl px-4 py-2.5 text-sm outline-none focus:border-blue-500 transition-colors">
                    <option>仅创建账号</option>
                    <option>创建后生成 API Key</option>
                    <option>开启 Plus 订阅 (需外挂)</option>
                  </select>
                </div>
              </div>

              <button
                onClick={() => handleTrigger('openai_register_default')}
                disabled={!!runningId}
                className="w-full phantom-btn phantom-btn--primary py-3 rounded-2xl font-bold flex items-center justify-center gap-2 group shadow-xl shadow-blue-600/20"
              >
                {runningId ? <Loader2 size={18} className="animate-spin" /> : <Send size={18} className="group-hover:translate-x-1 group-hover:-translate-y-1 transition-transform" />}
                {runningId ? '注册流程初始化中...' : '启动 OpenAI 自动化注册'}
              </button>
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
