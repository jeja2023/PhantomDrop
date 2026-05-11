import { useMemo, useState } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { Zap, CheckCircle2, Sparkles, Activity, Database } from 'lucide-react'
import Grid from '../grid/Grid'
import Terminal from '../terminal/Terminal'
import type { AppLog, DashboardStats, EmailItem } from '../types'

interface DashboardViewProps {
  emails: EmailItem[]
  logs: AppLog[]
  stats: DashboardStats | null
  updateRate?: number
}

export default function DashboardView({ emails, logs, stats, updateRate = 1000 }: DashboardViewProps) {
  const [isExpertMode, setIsExpertMode] = useState(false)
  const [showToast, setShowToast] = useState(false)
  const [toastMsg, setToastMsg] = useState({ title: '', desc: '' })

  const metrics = useMemo(() => {
    const codeCount = emails.filter((email) => Boolean(email.code)).length
    const warnCount = logs.filter((log) => log.type === 'warn').length
    const successCount = logs.filter((log) => log.type === 'success').length
    const totalEmails = stats?.total_emails ?? emails.length
    const codeEmails = stats?.code_emails ?? codeCount
    const coverage = totalEmails === 0 ? 0 : Math.round((codeEmails / totalEmails) * 100)
    const successRate = logs.length === 0 ? 100 : Math.round((successCount / logs.length) * 100)
    const alertDensity = logs.length === 0 ? 0 : Math.round((warnCount / logs.length) * 100)

    return {
      totalEmails,
      activeEmails: stats?.active_emails ?? totalEmails,
      archivedEmails: stats?.archived_emails ?? 0,
      codeEmails,
      coverage,
      successRate,
      alertDensity,
      activity: Math.min(100, (stats?.recent_emails_24h ?? emails.length) * 10),
      activeWebhooks: stats?.active_webhooks ?? 0,
      workflowRuns: stats?.workflow_runs_24h ?? 0,
    }
  }, [emails, logs, stats])

  const triggerToast = (title: string, desc: string) => {
    setToastMsg({ title, desc })
    setShowToast(true)
    setTimeout(() => setShowToast(false), 3000)
  }

  const handleExpertMode = () => {
    setIsExpertMode(!isExpertMode)
    triggerToast(
      isExpertMode ? '已切换到常规模式' : '已进入专家模式',
      isExpertMode ? '界面已恢复标准视图。' : '已展示更深层的诊断信息与实时运行指标。',
    )
  }

  return (
    <div className={`page-shell page-shell--full animate-in fade-in duration-700 relative transition-colors duration-500 ${isExpertMode ? 'bg-blue-900/5' : ''}`}>
      <AnimatePresence>
        {showToast ? (
          <motion.div initial={{ y: -50, opacity: 0 }} animate={{ y: 0, opacity: 1 }} exit={{ y: -50, opacity: 0 }} className="fixed top-20 right-10 z-[100]">
            <div className="bg-white border border-blue-100 shadow-2xl shadow-blue-500/10 px-6 py-3 rounded-2xl flex items-center gap-3">
              <CheckCircle2 className="text-blue-500" size={20} />
              <div className="flex flex-col">
                <span className="text-sm font-bold text-slate-800 tracking-tight">{toastMsg.title}</span>
                <span className="text-[10px] text-slate-500 font-mono">{toastMsg.desc}</span>
              </div>
            </div>
          </motion.div>
        ) : null}
      </AnimatePresence>

      <div className="flex items-center justify-between px-2 pt-2 shrink-0">
        <div className="flex items-center gap-6">
          <div className="flex items-center gap-2 rounded-xl border border-emerald-100 bg-emerald-50/50 px-3 py-2 shadow-sm">
            <span className="relative flex h-2 w-2">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
              <span className="relative inline-flex rounded-full h-2 w-2 bg-emerald-500"></span>
            </span>
            <span className="text-[11px] font-black tracking-widest text-emerald-700 uppercase">系统感知：在线</span>
            {isExpertMode ? <Sparkles size={14} className="text-blue-500" /> : null}
          </div>
          
          <div className="h-8 w-px bg-slate-200"></div>
          
          <div className="flex items-center gap-8">
            <MiniStat label="捕获总量" value={metrics.totalEmails.toString()} sub="邮件流集成" color="blue" />
            <MiniStat label="解析覆盖率" value={`${metrics.coverage}%`} sub={`命中 ${metrics.codeEmails} 封`} color="emerald" />
          </div>
        </div>

        <div className="flex items-center gap-3">
          <div className="px-3 py-1.5 rounded-lg bg-slate-100 border border-slate-200 text-[10px] font-mono text-slate-500 flex items-center gap-2">
            <Activity size={12} className="text-blue-500" />
            更新频率：{(1000 / updateRate).toFixed(1)}Hz
          </div>
        </div>
      </div>

      <div className="flex-grow flex flex-col lg:flex-row gap-4 min-h-0">
        <div className="flex-[3] flex flex-col space-y-3 min-w-0">
          <div className="flex items-center justify-between px-2 shrink-0">
            <div className="flex flex-col">
              <div className="flex items-center gap-2 text-slate-800 font-bold text-[11px] tracking-tight leading-none">
                <Zap size={10} className="text-blue-500" />
                流式网格引擎
                {isExpertMode ? <span className="text-blue-500 text-[8px]">诊断覆盖层开启</span> : null}
              </div>
              <div className="text-[8px] text-slate-400 font-mono tracking-[0.2em] mt-0.5 ml-4">实时分布视图</div>
            </div>
            <div className="text-[9px] text-slate-700 font-mono bg-slate-100 px-2 py-0.5 rounded tracking-tighter">图形加速已就绪</div>
          </div>
          <div className="page-panel flex-grow overflow-hidden relative group min-h-0">
            <div className={`absolute inset-0 scan-line pointer-events-none transition-opacity ${isExpertMode ? 'opacity-20' : 'opacity-5'}`}></div>
            <Grid data={emails} />
          </div>
        </div>

        <div className="page-panel flex-grow lg:flex-1 p-5 flex flex-col gap-4 bg-white bg-[radial-gradient(#e2e8f0_1px,transparent_1px)] [background-size:20px_20px] min-h-[360px] overflow-y-auto relative">
          <div className="flex min-h-0 flex-1 flex-col gap-4 relative z-10">
            <div className="flex items-center justify-between shrink-0">
              <div className="flex flex-col">
                <span className="text-[12px] font-black text-indigo-600 tracking-tighter leading-none">决策核心 / CORE</span>
                <span className="text-[8px] text-slate-400 font-mono tracking-widest mt-1 uppercase">运行摘要指标</span>
              </div>
              <span className="flex items-center gap-1.5 text-[9px] text-emerald-500 font-mono bg-emerald-50 px-2 py-0.5 rounded-full border border-emerald-100">
                <div className="w-1 h-1 rounded-full bg-emerald-500 animate-pulse"></div>
                {metrics.activeWebhooks} 活跃网格
              </span>
            </div>

            <div className="grid shrink-0 gap-2.5 pr-1">
              <ProgressItem label="校验引擎覆盖率" percent={metrics.coverage} color="blue" />
              <ProgressItem label="神经元解析成功率" percent={metrics.successRate} color="cyan" />
              <ProgressItem label="异常信号密度" percent={metrics.alertDensity} color="indigo" />
              <ProgressItem label="实时邮件活跃度" percent={metrics.activity} color="amber" />
            </div>

            <div className="shrink-0 rounded-2xl border border-slate-200 bg-white/90 backdrop-blur-sm p-4 shadow-sm">
              <div className="mb-3 flex items-center justify-between">
                <div className="text-[11px] font-bold text-slate-800 tracking-tight">感应层实时统计</div>
                <Database size={12} className="text-slate-400" />
              </div>
              <div className="grid grid-cols-2 gap-x-5 gap-y-3 font-mono text-[11px]">
                <div className="flex min-w-0 flex-col gap-1">
                  <span className="whitespace-nowrap text-[8px] uppercase leading-none text-slate-400">活跃节点</span>
                  <span className="font-bold leading-tight tabular-nums text-slate-700">{metrics.activeEmails}</span>
                </div>
                <div className="flex min-w-0 flex-col gap-1">
                  <span className="whitespace-nowrap text-[8px] uppercase leading-none text-slate-400">归档节点</span>
                  <span className="font-bold leading-tight tabular-nums text-slate-700">{metrics.archivedEmails}</span>
                </div>
                <div className="flex min-w-0 flex-col gap-1">
                  <span className="whitespace-nowrap text-[8px] uppercase leading-none text-slate-400">24H 工作流</span>
                  <span className="font-bold leading-tight tabular-nums text-slate-700">{metrics.workflowRuns}</span>
                </div>
                <div className="flex min-w-0 flex-col gap-1">
                  <span className="whitespace-nowrap text-[8px] uppercase leading-none text-slate-400">今日注入</span>
                  <span className="font-bold leading-tight tabular-nums text-slate-700">{stats?.recent_emails_24h ?? emails.length}</span>
                </div>
              </div>
            </div>
          </div>

          <div className="relative z-10 mt-auto shrink-0 border-t border-slate-100 pt-3">
            <button
              onClick={handleExpertMode}
              className={`flex h-11 w-full items-center justify-center gap-2 rounded-xl px-3 text-[10px] font-black leading-none tracking-[0.08em] transition-all sm:tracking-[0.16em] ${
                isExpertMode
                  ? 'bg-indigo-600 text-white shadow-lg shadow-indigo-200'
                  : 'bg-slate-900 text-white hover:bg-slate-800'
              }`}
            >
              <Zap size={12} />
              {isExpertMode ? '退出专家诊断模式' : '进入专家诊断模式'}
            </button>
          </div>
        </div>
      </div>

      <section className="h-[220px] shrink-0 min-h-0">
        <div className="page-panel h-full overflow-hidden">
          <Terminal logs={logs} />
        </div>
      </section>
    </div>
  )
}

function ProgressItem({ label, percent, isLatency = false, color = 'blue' }: { label: string; percent: number; isLatency?: boolean; color?: string }) {
  const getGradient = () => {
    switch (color) {
      case 'cyan':
        return 'from-cyan-500 to-blue-500'
      case 'indigo':
        return 'from-indigo-500 to-purple-500'
      case 'amber':
        return 'from-amber-500 to-orange-500'
      default:
        return 'from-blue-600 to-indigo-400'
    }
  }

  return (
    <div className="space-y-1 group">
      <div className="flex justify-between text-[9px] font-bold text-slate-600 tracking-tight group-hover:text-slate-700 transition-colors">
        <span>{label}</span>
        <span className="font-mono">{isLatency ? `${percent}毫秒` : `${percent}%`}</span>
      </div>
      <div className="h-1 w-full bg-slate-100 rounded-full overflow-hidden border border-slate-200">
        <motion.div
          animate={{ width: isLatency ? `${Math.min(100, percent * 2)}%` : `${percent}%` }}
          transition={{ duration: 0.8, ease: 'easeInOut' }}
          className={`h-full bg-gradient-to-r ${getGradient()} shadow-[0_0_10px_rgba(59,130,246,0.3)]`}
        />
      </div>
    </div>
  )
}
function MiniStat({ label, value, sub, color }: { label: string; value: string; sub: string; color: 'blue' | 'emerald' }) {
  return (
    <div className="flex flex-col">
      <div className="text-[10px] font-bold text-slate-500 tracking-tight leading-none mb-1 uppercase">{label}</div>
      <div className="flex items-baseline gap-2">
        <span className="text-xl font-black tracking-tighter text-slate-900 leading-none">{value}</span>
        <span className={`text-[9px] font-bold ${color === 'blue' ? 'text-blue-500' : 'text-emerald-500'} tracking-tight`}>{sub}</span>
      </div>
    </div>
  )
}
