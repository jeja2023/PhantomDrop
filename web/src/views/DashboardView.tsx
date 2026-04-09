import { useMemo, useState } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { Zap, CheckCircle2, Sparkles } from 'lucide-react'
import Grid from '../grid/Grid'
import Terminal from '../terminal/Terminal'
import PageHeader from '../ui/PageHeader'
import type { AppLog, DashboardStats, EmailItem } from '../types'

interface DashboardViewProps {
  emails: EmailItem[]
  logs: AppLog[]
  stats: DashboardStats | null
}

export default function DashboardView({ emails, logs, stats }: DashboardViewProps) {
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

      <PageHeader
        title={`实时态势感知${isExpertMode ? ' · 专家模式' : ''}`}
        kicker={isExpertMode ? '实时态势感知（专家模式）' : '实时态势感知'}
        description="统一查看实时邮件摄入、验证码覆盖率、工作流活跃度和原始系统流。"
        status={
          <div className="flex items-center gap-2 rounded-full border border-emerald-100 bg-emerald-50 px-3 py-1.5">
            <span className="relative flex h-1.5 w-1.5">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
              <span className="relative inline-flex rounded-full h-1.5 w-1.5 bg-emerald-500"></span>
            </span>
            <span className="text-[10px] font-black tracking-widest text-emerald-700">本地中枢运行中</span>
            {isExpertMode ? <Sparkles size={14} className="text-blue-500" /> : null}
          </div>
        }
        actions={
          <>
            <StatCard label="已摄入邮件" value={metrics.totalEmails.toString()} trend={`活跃度 ${metrics.activity}%`} color="blue" isMini />
            <StatCard label="验证码覆盖率" value={`${metrics.coverage}%`} trend={`命中 ${metrics.codeEmails} 封`} color="emerald" isMini />
          </>
        }
      />

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

        <div className="page-panel flex-grow lg:flex-1 p-3 flex flex-col justify-between bg-gradient-to-b from-transparent to-blue-50 min-h-0 overflow-hidden">
          <div className="space-y-3 min-h-0">
            <div className="flex items-center justify-between shrink-0">
              <div className="flex flex-col">
                <span className="text-[11px] font-black text-blue-600 tracking-tight leading-none">决策核心</span>
                <span className="text-[8px] text-slate-400 font-mono tracking-widest mt-0.5">运行摘要</span>
              </div>
              <span className="flex items-center gap-1 text-[9px] text-emerald-500 font-mono">
                <div className="w-1 h-1 rounded-full bg-emerald-500 animate-pulse"></div>
                {metrics.activeWebhooks} 个活跃 Webhook
              </span>
            </div>

            <div className="text-[15px] font-black italic tracking-tighter glow-text-blue leading-none">神经中枢已激活</div>
            <div className="h-[1px] w-full bg-gradient-to-r from-blue-500/30 to-transparent"></div>

            <div className="space-y-2 pr-1">
              <ProgressItem label="验证码覆盖率" percent={metrics.coverage} color="blue" />
              <ProgressItem label="日志成功率" percent={metrics.successRate} color="cyan" />
              <ProgressItem label="警报密度" percent={metrics.alertDensity} color="indigo" />
              <ProgressItem label="邮件活跃度" percent={metrics.activity} color="amber" />
            </div>

            <div className="rounded-xl border border-slate-200 bg-white/70 px-3 py-2 text-[10px] text-slate-600">
              <div className="font-bold text-slate-800">后端真实统计</div>
              <div className="mt-1 grid grid-cols-2 gap-x-3 gap-y-1 font-mono">
                <span>活跃邮件：{metrics.activeEmails}</span>
                <span>归档邮件：{metrics.archivedEmails}</span>
                <span>近 24 小时工作流：{metrics.workflowRuns}</span>
                <span>近 24 小时新邮件：{stats?.recent_emails_24h ?? emails.length}</span>
              </div>
            </div>
          </div>

          <div className="pt-3 border-t border-slate-200 shrink-0 mt-2">
            <button
              onClick={handleExpertMode}
              className={`w-full py-2 border rounded-lg text-[9px] font-black tracking-widest transition-all neon-btn cursor-pointer ${
                isExpertMode
                  ? 'bg-blue-600 text-white border-blue-500 shadow-lg shadow-blue-500/40'
                  : 'bg-blue-600/10 border-blue-500/20 text-blue-400 hover:bg-blue-600 hover:text-white'
              }`}
            >
              {isExpertMode ? '退出专家模式' : '进入专家模式'}
            </button>
          </div>
        </div>
      </div>

      <section className="h-[140px] shrink-0 min-h-0">
        <div className="page-panel h-full overflow-hidden">
          <Terminal logs={logs} />
        </div>
      </section>
    </div>
  )
}

function StatCard({
  label,
  value,
  trend,
  color,
  isMini = false,
}: {
  label: string
  value: string
  trend: string
  color: 'blue' | 'emerald'
  isMini?: boolean
}) {
  return (
    <div className={`glass-panel border-slate-200 group hover:border-blue-500/30 transition-all hover:-translate-y-0.5 duration-300 ${isMini ? 'px-4 py-2 flex flex-col justify-center' : 'p-5'}`}>
      <div className="text-[9px] text-slate-600 font-bold tracking-widest mb-0.5">{label}</div>
      <div className="flex items-baseline gap-2">
        <div className="text-lg font-black tracking-tighter text-slate-900">{value}</div>
        <div className={`text-[9px] font-bold ${color === 'blue' ? 'text-blue-500' : 'text-emerald-500'}`}>{trend}</div>
      </div>
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
