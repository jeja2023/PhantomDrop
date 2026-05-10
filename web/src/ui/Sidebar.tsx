import { type ReactNode } from 'react'
import { Activity, Globe, Mail, Settings, Shield, Terminal as TerminalIcon, Users, Zap } from 'lucide-react'
import type { AppTab } from '../types'

interface SidebarProps {
  activeTab: AppTab
  onTabChange: (tab: AppTab) => void
}

export default function Sidebar({ activeTab, onTabChange }: SidebarProps) {
  return (
    <nav className="w-16 shrink-0 md:w-64 bg-white border-r border-[#f1f5f9] flex flex-col p-4 z-20 h-screen transition-all select-none shadow-[4px_0_24px_rgba(0,0,0,0.02)]">
      <div className="flex items-center gap-3 mb-10 px-2 mt-4">
        <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-blue-600 to-indigo-600 flex items-center justify-center shadow-xl shadow-blue-500/20 group cursor-pointer transition-transform hover:rotate-12 border border-slate-300">
          <Shield size={22} className="text-slate-900 group-hover:scale-110 transition-transform" />
        </div>
        <div className="flex flex-col hidden md:flex">
          <span className="font-extrabold text-sm tracking-tighter text-slate-900 leading-none">幻影中枢</span>
          <span className="text-[9px] text-slate-700 font-mono tracking-widest mt-1 font-bold">核心节点 0.0.18</span>
        </div>
      </div>

      <div className="space-y-4 flex-grow px-1">
        <SidebarItem icon={<Activity size={22} />} label="实时控制中心" active={activeTab === 'dashboard'} onClick={() => onTabChange('dashboard')} />
        <SidebarItem icon={<Mail size={22} />} label="邮件解析列表" active={activeTab === 'emails'} onClick={() => onTabChange('emails')} />
        <SidebarItem icon={<TerminalIcon size={22} />} label="系统流监控" active={activeTab === 'logs'} onClick={() => onTabChange('logs')} />
        <SidebarItem icon={<Globe size={22} />} label="内网穿透助手" active={activeTab === 'tunnel'} onClick={() => onTabChange('tunnel')} />
        <SidebarItem icon={<Zap size={22} />} label="自动化工作流" active={activeTab === 'auto'} onClick={() => onTabChange('auto')} />
        <SidebarItem icon={<Shield size={22} />} label="网站注册中心" active={activeTab === 'register'} onClick={() => onTabChange('register')} />
        <SidebarItem icon={<Users size={22} />} label="账号管理中心" active={activeTab === 'accounts'} onClick={() => onTabChange('accounts')} />
      </div>

      <div className="mt-auto pt-4 border-t border-slate-200 space-y-4">
        <SidebarItem icon={<Settings size={22} />} label="全局设置" active={activeTab === 'config'} onClick={() => onTabChange('config')} />
        <div className="px-3 py-1 flex items-center gap-2">
          <div className="w-1.5 h-1.5 rounded-full bg-emerald-500 shadow-lg shadow-emerald-500/30" />
          <span className="text-[9px] text-slate-600 font-mono hidden md:block font-bold tracking-tight">节点状态：在线</span>
        </div>
      </div>
    </nav>
  )
}

function SidebarItem({
  active,
  icon,
  label,
  onClick,
}: {
  active: boolean
  icon: ReactNode
  label: string
  onClick: () => void
}) {
  return (
    <button
      onClick={onClick}
      className={`w-full flex items-center gap-3 px-3 py-3 rounded-2xl transition-all group ${
        active
          ? 'bg-slate-900 text-white shadow-xl shadow-slate-900/10'
          : 'text-slate-500 hover:bg-slate-50 hover:text-slate-900'
      }`}
      title={label}
    >
      <span className={active ? 'text-white' : 'text-slate-400 group-hover:text-slate-700'}>{icon}</span>
      <span className="hidden md:block text-xs font-black tracking-tight whitespace-nowrap">{label}</span>
    </button>
  )
}
