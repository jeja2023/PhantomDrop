import { useMemo } from 'react'
import { Zap, Activity, ShieldCheck } from 'lucide-react'
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
  // 计算多项运行状态指标
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
      totalAccounts: stats?.total_accounts ?? 0,
      todayAccounts: stats?.today_accounts_24h ?? 0,
      gatewayRequests: stats?.gateway_requests_24h ?? 0,
      activePoolAccounts: stats?.active_pool_accounts ?? 0,
      coolingAccounts: stats?.cooling_accounts ?? 0,
    }
  }, [emails, logs, stats])

  return (
    /* 外层 page-shell 强制拉满屏幕视口高度，自适应父容器精确分配的剩余高度 */
    <div className="page-shell page-shell--full animate-in fade-in duration-700 relative flex flex-col h-full overflow-hidden bg-slate-50/20">
      
      {/* 顶部控制状态条：融合了感知状态与核心吞吐量 */}
      <div className="flex items-center justify-between px-2 pt-2 shrink-0 mb-3">
        <div className="flex items-center gap-6">
          <div className="flex items-center gap-2.5 rounded-2xl border border-solid border-emerald-100 bg-emerald-50/50 px-3.5 py-2 shadow-[0_4px_15px_rgba(16,185,129,0.06)] backdrop-blur-sm">
            <span className="relative flex h-2 w-2">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-80"></span>
              <span className="relative inline-flex rounded-full h-2 w-2 bg-emerald-500"></span>
            </span>
            <span className="text-[10px] font-black tracking-widest text-emerald-700 uppercase flex items-center gap-1 leading-none">
              中枢感知：常驻在线
            </span>
          </div>
          
          <div className="h-8 w-px bg-slate-200"></div>
          
          <div className="flex items-center gap-8">
            <MiniStat label="捕获总量" value={metrics.totalEmails.toString()} sub="邮件流集成" color="blue" />
            <MiniStat label="解析覆盖率" value={`${metrics.coverage}%`} sub={`命中 ${metrics.codeEmails} 封`} color="emerald" />
          </div>
        </div>

        <div className="flex items-center gap-3">
          <div className="px-3 py-1.5 rounded-lg bg-slate-100 border border-slate-200 text-[10px] font-mono text-slate-500 flex items-center gap-2 shadow-sm">
            <Activity size={12} className="text-blue-500" />
            更新频率：{(1000 / updateRate).toFixed(1)}Hz
          </div>
        </div>
      </div>

      {/* 中部并排联动主网格 */}
      <div className="flex-grow flex flex-col lg:flex-row gap-6 min-h-0 mb-4 overflow-hidden">
        
        {/* 左侧：流式网格引擎（常驻高清晰度科技扫描线） */}
        <div className="flex-[3.2] flex flex-col space-y-3 min-w-0 h-full overflow-hidden">
          <div className="flex items-center justify-between px-2 shrink-0">
            <div className="flex flex-col">
              <div className="flex items-center gap-2 text-slate-800 font-bold text-[11px] tracking-tight leading-none">
                <Zap size={10} className="text-blue-500 animate-pulse" />
                流式网格引擎
                <span className="text-blue-500 text-[8px] font-black uppercase ml-1 opacity-70">Diagnostic layer active</span>
              </div>
              <div className="text-[8px] text-slate-400 font-mono tracking-[0.2em] mt-0.5 ml-4">实时分布式视图</div>
            </div>
            <div className="text-[9px] text-slate-700 font-mono bg-slate-100 px-2 py-0.5 rounded tracking-tighter border border-slate-200">图形加速已就绪</div>
          </div>
          <div className="page-panel flex-grow overflow-hidden relative group min-h-0 rounded-3xl border border-slate-200 bg-white shadow-sm">
            {/* 极客网格扫描线常驻开启为黄金可见度 12%，极富未来美感，无需点击 */}
            <div className="absolute inset-0 scan-line pointer-events-none opacity-[0.12]"></div>
            <Grid data={emails} />
          </div>
        </div>

        {/* 右侧：一体化的自愈与决策大盘（拿掉了多重嵌套边框与顶部切换键，大盘极致舒展） */}
        <div className="page-panel flex-grow lg:flex-[1.2] p-4 flex flex-col gap-4 bg-white bg-[radial-gradient(#e2e8f0_1px,transparent_1px)] [background-size:20px_20px] rounded-3xl border border-slate-200 shadow-sm min-h-0 overflow-hidden relative">
          <div className="flex min-h-0 flex-1 flex-col gap-4 relative z-10">
            
            {/* 顶栏 Header：极致精炼，空间完美延展 */}
            <div className="flex items-center justify-between shrink-0 border-b border-slate-100 pb-3">
              <div className="flex flex-col">
                <span className="text-[12px] font-black text-slate-800 tracking-wider leading-none">决策核心 / DECISION CORE</span>
                <span className="text-[8px] text-slate-400 font-mono tracking-widest mt-1.5 uppercase leading-none">系统运行态诊断大盘</span>
              </div>
              
              <span className="flex items-center gap-1.5 text-[8.5px] text-emerald-500 font-mono bg-emerald-50 px-2.5 py-1 rounded-xl border border-emerald-100 shadow-sm">
                <span className="h-1.5 w-1.5 rounded-full bg-emerald-500 animate-pulse"></span>
                {metrics.activeWebhooks} 监测节点在线
              </span>
            </div>

            {/* 4 个高阶 ProgressItem 核心参数指标：2x2对称排布，大幅节省纵向高度 */}
            <div className="grid grid-cols-2 gap-x-4 gap-y-2 shrink-0 pr-1">
              <ProgressItem label="校验引擎覆盖率" percent={metrics.coverage} color="blue" />
              <ProgressItem label="神经元解析成功率" percent={metrics.successRate} color="cyan" />
              <ProgressItem label="异常信号密度" percent={metrics.alertDensity} color="indigo" />
              <ProgressItem label="实时邮件活跃度" percent={metrics.activity} color="amber" />
            </div>

            {/* 水平分界虚线：高度融入底板 */}
            <div className="border-t border-dashed border-slate-200 my-1 relative shrink-0">
              <span className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 bg-white px-3 text-[7.5px] font-mono font-bold tracking-widest text-slate-400 uppercase select-none leading-none">
                中枢自愈感知网格 (Sentinel System Mesh)
              </span>
            </div>

            {/* 自愈神经 8 宫格：重构为 4x2 极紧凑矩阵，节省大量纵向空间 */}
            <div className="shrink-0">
              <div className="grid grid-cols-4 gap-1.5 font-mono text-[10px]">
                <MiniGridCell label="活跃邮件" value={metrics.activeEmails} sub={`/${metrics.totalEmails}`} />
                <MiniGridCell label="归档节点" value={metrics.archivedEmails} />
                <MiniGridCell label="24H工作流" value={metrics.workflowRuns} />
                <MiniGridCell label="今日数据" value={stats?.recent_emails_24h ?? emails.length} />
                
                <MiniGridCell 
                  label="高可用账号" 
                  value={metrics.activePoolAccounts} 
                  sub={`/${metrics.totalAccounts}`}
                  valueColor="text-indigo-600"
                />
                <MiniGridCell label="网关请求" value={metrics.gatewayRequests} />
                <MiniGridCell label="新增账号" value={metrics.todayAccounts} />
                
                {metrics.coolingAccounts > 0 ? (
                  <MiniGridCell 
                    label="网络自愈" 
                    value={`${metrics.coolingAccounts} 冷却`} 
                    valueColor="text-amber-500 animate-pulse" 
                  />
                ) : (
                  <MiniGridCell 
                    label="网络自愈" 
                    value="链路就绪" 
                    valueColor="text-emerald-600" 
                  />
                )}
              </div>
            </div>

            {/* 极客页脚签名常驻：状态 100% 物理连接 */}
            <div className="mt-auto pt-3.5 border-t border-slate-100 flex items-center justify-between text-[8px] font-mono text-slate-400 tracking-tighter shrink-0 select-none leading-none">
              <span>SYSTEM_NODE_KEY: {stats ? 'VERIFIED' : 'DEFAULT'}</span>
              <span className="animate-pulse flex items-center gap-1 text-emerald-500 font-bold uppercase tracking-widest text-[7.5px]">
                <ShieldCheck size={10} />
                sentinel linkage active
              </span>
            </div>

          </div>
        </div>
      </div>

      {/* 底部终端显示 */}
      <section className="h-[250px] shrink-0 min-h-0">
        <div className="page-panel h-full overflow-hidden rounded-3xl border border-slate-200 bg-white shadow-sm">
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
      <div className="h-1 w-full bg-slate-100 rounded-full overflow-hidden border border-slate-200 shadow-inner">
        <div className="h-full bg-slate-200 rounded-full overflow-hidden">
          <div
            style={{ width: `${percent}%` }}
            className={`h-full bg-gradient-to-r ${getGradient()} shadow-[0_0_10px_rgba(59,130,246,0.25)] transition-all duration-500`}
          />
        </div>
      </div>
    </div>
  )
}

function MiniStat({ label, value, sub, color }: { label: string; value: string; sub: string; color: 'blue' | 'emerald' }) {
  return (
    <div className="flex flex-col">
      <div className="text-[10px] font-bold text-slate-500 tracking-tight leading-none mb-1.5 uppercase">{label}</div>
      <div className="flex items-baseline gap-2">
        <span className="text-xl font-black tracking-tighter text-slate-900 leading-none">{value}</span>
        <span className={`text-[9px] font-bold ${color === 'blue' ? 'text-blue-500' : 'text-emerald-500'} tracking-tight`}>{sub}</span>
      </div>
    </div>
  )
}

function MiniGridCell({ 
  label, 
  value, 
  sub = '', 
  valueColor = 'text-slate-700' 
}: { 
  label: string
  value: string | number
  sub?: string
  valueColor?: string
}) {
  return (
    <div className="flex flex-col gap-0.5 bg-slate-50/50 hover:bg-slate-100/70 p-1.5 rounded-xl border border-slate-100 transition-all duration-300 group/cell shadow-sm">
      <span className="whitespace-nowrap text-[8px] uppercase leading-none text-slate-400 font-bold tracking-tight">
        {label}
      </span>
      <span className={`font-black leading-none tabular-nums flex items-baseline gap-0.5 ${valueColor} text-[10px] tracking-tight`}>
        {value}
        {sub && <span className="text-[7.5px] text-slate-400 font-normal ml-0.5 font-mono">{sub}</span>}
      </span>
    </div>
  )
}
