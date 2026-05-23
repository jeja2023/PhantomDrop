import { useEffect, useRef, useState } from 'react'
import { Terminal as TerminalIcon, ArrowDownCircle, Sliders } from 'lucide-react'
import { redactMessage } from '../lib/utils'
import type { AppLog, LogSource } from '../types'

interface TerminalProps {
  logs: AppLog[]
  activeFilter?: 'all' | LogSource
}

// 定义日志级别映射
const LEVEL_VALUES = {
  info: 0,
  success: 1,
  warn: 2,
  error: 3,
} as const

type LogType = 'info' | 'success' | 'warn' | 'error'

export default function Terminal({ logs, activeFilter = 'all' }: TerminalProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  
  // 状态：自动追底滚动锁定，默认开启
  const [autoScroll, setAutoScroll] = useState(true)
  
  // 状态：日志级别滑条，0=全部，1=成功及以上，2=警告及以上，3=严重错误
  const [minLevelSlider, setMinLevelSlider] = useState(0)

  // 映射滑条数值到日志的 type
  const getMinLevelName = (val: number): LogType => {
    if (val === 0) return 'info'
    if (val === 1) return 'success'
    if (val === 2) return 'warn'
    return 'error'
  }

  const activeLevel = getMinLevelName(minLevelSlider)

  // 1. 过滤日志源 2. 根据高级级别滑条再次微滤
  const filteredLogs = logs.filter((log) => {
    // 过滤日志源
    if (activeFilter !== 'all' && log.source !== activeFilter) return false
    
    // 过滤级别。由于有些 log 的 type 可能是自定义的，我们默认设为 info
    const logType: LogType = (log.type === 'success' || log.type === 'warn' || log.type === 'error' || log.type === 'info') 
      ? log.type 
      : 'info'
      
    return LEVEL_VALUES[logType] >= LEVEL_VALUES[activeLevel]
  })

  // 监听日志流变更，触发平滑自动追底
  useEffect(() => {
    if (autoScroll && containerRef.current) {
      const container = containerRef.current
      container.scrollTo({
        top: container.scrollHeight,
        behavior: 'smooth',
      })
    }
  }, [filteredLogs, autoScroll])

  // 手动滚动检测：如果用户向上翻阅，自动关闭滚动锁定；滚回底部时自动重新开启
  const handleScroll = () => {
    if (!containerRef.current) return
    const { scrollTop, scrollHeight, clientHeight } = containerRef.current
    
    // 差值小于 10 像素代表已经到底部
    const isAtBottom = scrollHeight - scrollTop - clientHeight < 10
    
    if (isAtBottom) {
      if (!autoScroll) setAutoScroll(true)
    } else {
      // 只有在自动滚动开启时才变动，避免重复执行
      if (autoScroll) {
        // 用户向上滚，关闭自动追底
        setAutoScroll(false)
      }
    }
  }

  // 切换追底锁定
  const toggleAutoScroll = () => {
    setAutoScroll((prev) => !prev)
    if (!autoScroll && containerRef.current) {
      const container = containerRef.current
      container.scrollTo({
        top: container.scrollHeight,
        behavior: 'smooth',
      })
    }
  }

  return (
    <div className="flex flex-col h-full bg-slate-950 border border-slate-900 rounded-3xl overflow-hidden shadow-2xl relative select-none">
      {/* 顶部极客审计控制台 */}
      <div className="px-5 py-3 border-b border-slate-900 bg-slate-950/85 flex flex-wrap items-center justify-between shrink-0 gap-3 z-10 backdrop-blur-md">
        <span className="text-[10px] font-mono flex items-center gap-2 text-cyan-400 font-black tracking-widest uppercase">
          <TerminalIcon size={12} className="text-cyan-400 animate-pulse" />
          内核审计终端 (CORE AUDIT)
        </span>
        
        {/* 右侧微调滑块与自动锁定 */}
        <div className="flex items-center gap-5">
          {/* 日志级别滑块面板 */}
          <div className="flex items-center gap-2 bg-slate-900/60 rounded-xl px-3 py-1 border border-slate-800/80">
            <Sliders size={11} className="text-slate-500" />
            <span className="text-[9px] font-bold font-mono text-slate-400 shrink-0">
              最小日志级: <span className="text-cyan-400 uppercase font-black">{activeLevel}</span>
            </span>
            <input
              type="range"
              min={0}
              max={3}
              value={minLevelSlider}
              onChange={(e) => setMinLevelSlider(Number(e.target.value))}
              className="w-16 h-1 bg-slate-800 rounded-lg appearance-none cursor-pointer accent-cyan-400 transition-all hover:bg-slate-700 focus:outline-none"
              title="滑动过滤日志等级"
              aria-label="滑动过滤日志等级"
            />
          </div>

          {/* 自动追底锁定开关 */}
          <button
            type="button"
            onClick={toggleAutoScroll}
            className={`flex items-center gap-1.5 rounded-xl px-2.5 py-1 text-[9px] font-black border transition-all duration-300 ${
              autoScroll 
                ? 'bg-cyan-500/10 border-cyan-500/30 text-cyan-400' 
                : 'bg-slate-900/40 border-slate-800 text-slate-500'
            }`}
            title={autoScroll ? '自动滚动锁定激活中' : '自动滚动已暂停'}
          >
            <ArrowDownCircle size={11} className={autoScroll ? 'animate-bounce' : ''} />
            {autoScroll ? '滚动锁定 ON' : '锁定 OFF'}
          </button>

          {/* 传统三色按键 */}
          <div className="flex gap-1.5">
            <div className="w-2.5 h-2.5 rounded-full bg-rose-500/35 border border-rose-500/20"></div>
            <div className="w-2.5 h-2.5 rounded-full bg-amber-500/35 border border-amber-500/20"></div>
            <div className="w-2.5 h-2.5 rounded-full bg-cyan-500/35 border border-cyan-500/20 animate-pulse"></div>
          </div>
        </div>
      </div>

      {/* 终端正文：Cyberpunk CRT 显像管扫描线纹理背景 */}
      <div 
        ref={containerRef}
        onScroll={handleScroll}
        style={{
          backgroundImage: 'linear-gradient(rgba(18, 16, 16, 0) 50%, rgba(0, 0, 0, 0.25) 50%), linear-gradient(90deg, rgba(56, 189, 248, 0.05), rgba(34, 197, 94, 0.02), rgba(168, 85, 247, 0.05))',
          backgroundSize: '100% 4px, 6px 100%'
        }}
        className="min-w-0 p-5 bg-slate-950 font-mono text-[11px] leading-relaxed space-y-1.5 overflow-y-auto flex-grow custom-scrollbar relative"
      >
        {filteredLogs.length > 0 ? (
          filteredLogs.map((log, index) => {
            const previousGroup = index > 0 ? filteredLogs[index - 1].groupLabel : ''
            const showGroupHeader = log.groupLabel && log.groupLabel !== previousGroup

            // 根据日志级别动态渲染荧光辉光样式
            let logStyle = 'text-cyan-400'
            let glowStyle = {}
            if (log.type === 'success') {
              logStyle = 'text-emerald-400 font-extrabold'
              glowStyle = { textShadow: '0 0 6px rgba(16, 185, 129, 0.4)' }
            } else if (log.type === 'warn') {
              logStyle = 'text-amber-400 font-semibold'
              glowStyle = { textShadow: '0 0 6px rgba(245, 158, 11, 0.4)' }
            } else if (log.type === 'error') {
              logStyle = 'text-rose-400 font-black underline decoration-rose-500/40 underline-offset-4'
              glowStyle = { textShadow: '0 0 8px rgba(239, 68, 68, 0.5)' }
            } else {
              glowStyle = { textShadow: '0 0 4px rgba(56, 189, 248, 0.3)' }
            }

            return (
              <div key={log.id}>
                {showGroupHeader ? (
                  <div className="mb-1 mt-3 text-[9px] font-black tracking-[0.2em] text-purple-400/75 uppercase border-b border-purple-500/10 pb-0.5">
                    {`// [ ${log.groupLabel} ]`}
                  </div>
                ) : null}
                <div className="flex min-w-0 items-start gap-3 hover:bg-white/5 py-0.5 px-1 rounded transition-colors group">
                  <span className="text-slate-600 shrink-0 font-bold font-mono tracking-tight select-none">
                    [{log.time}]
                  </span>
                  <span className="text-slate-500 font-bold shrink-0 select-none">
                    {`[${log.source.replace('_', ' ').toUpperCase()}]`}
                  </span>
                  <span
                    style={glowStyle}
                    className={`min-w-0 break-all ${logStyle}`}
                  >
                    {redactMessage(log.content)}
                  </span>
                </div>
              </div>
            )
          })
        ) : (
          <div className="flex flex-col items-center justify-center py-20 text-slate-600 gap-3">
            <TerminalIcon size={24} className="opacity-20 animate-pulse text-cyan-400" />
            <span className="text-[10px] font-bold font-mono tracking-widest text-slate-500 uppercase">
              没有匹配当前级别的审计日志 流入中...
            </span>
          </div>
        )}

        {/* Cyberpunk 经典荧光绿/青闪烁光标 */}
        <div className="flex items-center gap-2 text-cyan-500/60 mt-3 pl-1 select-none">
          <span className="text-[10px] font-black animate-pulse">PHANTOM_HUB_SYSTEM:~$</span>
          <span className="animate-ping w-2 h-3.5 bg-cyan-400 rounded-sm"></span>
        </div>
      </div>
    </div>
  )
}
