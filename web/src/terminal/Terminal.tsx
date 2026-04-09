import { Terminal as TerminalIcon } from 'lucide-react'
import type { AppLog, LogSource } from '../types'

interface TerminalProps {
  logs: AppLog[]
  activeFilter?: 'all' | LogSource
}

export default function Terminal({ logs, activeFilter = 'all' }: TerminalProps) {
  const filteredLogs = activeFilter === 'all' ? logs : logs.filter((log) => log.source === activeFilter)

  return (
    <div className="glass-panel overflow-hidden border border-slate-200 flex flex-col h-full bg-slate-50">
      <div className="px-4 py-2 border-b border-slate-200 bg-slate-50 flex items-center justify-between shrink-0">
        <span className="text-[10px] font-mono flex items-center gap-2 text-slate-600 font-bold tracking-tight">
          <TerminalIcon size={12} className="text-blue-500" />
          系统终端审计
        </span>
        <div className="flex gap-1">
          <div className="w-2.5 h-2.5 rounded-full bg-red-500/50"></div>
          <div className="w-2.5 h-2.5 rounded-full bg-amber-500/50"></div>
          <div className="w-2.5 h-2.5 rounded-full bg-emerald-500/50"></div>
        </div>
      </div>
      <div className="min-w-0 p-5 bg-white font-mono text-xs md:text-sm space-y-2 overflow-y-auto flex-grow custom-scrollbar">
        {filteredLogs.map((log, index) => {
          const previousGroup = index > 0 ? filteredLogs[index - 1].groupLabel : ''
          const showGroupHeader = log.groupLabel && log.groupLabel !== previousGroup

          return (
            <div key={log.id}>
              {showGroupHeader ? <div className="mb-1 mt-3 text-[10px] font-black tracking-widest text-slate-400">{log.groupLabel}</div> : null}
              <div className="flex min-w-0 gap-4 animate-in fade-in slide-in-from-left-2 transition-all">
                <span className="text-slate-400 shrink-0">[{log.time}]</span>
                <span
                  className={`min-w-0 break-words ${
                    log.type === 'success'
                      ? 'text-emerald-600 font-bold'
                      : log.type === 'warn'
                        ? 'text-amber-600 italic'
                        : log.type === 'error'
                          ? 'text-rose-600 font-bold underline decoration-rose-500/20 underline-offset-2'
                          : 'text-blue-600'
                  }`}
                >
                  {log.content}
                </span>
              </div>
            </div>
          )
        })}
        <div className="flex gap-2 text-slate-400 mt-2">
          <span>{'>'}</span>
          <span className="animate-pulse w-2 h-4 bg-slate-400"></span>
        </div>
      </div>
    </div>
  )
}
