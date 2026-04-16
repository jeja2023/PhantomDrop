import { useState, useRef, useEffect, type ReactNode } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { Search, Zap, Activity, Settings, Mail, Loader2 } from 'lucide-react'
import { fetchJson, postJson } from '../lib/api'
import type { AppTab, EmailRecordApi, EmailItem, WorkflowDefinition, WorkflowKind } from '../types'

interface CmdProps {
  isOpen: boolean
  onClose: () => void
}

export default function Cmd({ isOpen, onClose }: CmdProps) {
  const [query, setQuery] = useState('')
  const [selectedIndex, setSelectedIndex] = useState(0)
  const [searchResults, setSearchResults] = useState<EmailItem[]>([])
  const [isSearching, setIsSearching] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)

  const handleClose = () => {
    setQuery('')
    setSelectedIndex(0)
    onClose()
  }

  const commands = [
    {
      id: 'gen',
      icon: <Zap size={16} />,
      title: '批量生成邮箱账户',
      shortcut: '/gen [数量]',
      subtitle: '一键触发真实工作流批量生成账户',
      action: (q: string) => handleAction(q.startsWith('/gen') ? q : '/gen 10'),
    },
    {
      id: 'status',
      icon: <Activity size={16} />,
      title: '查看中枢负载报告',
      shortcut: '/status',
      subtitle: '查看数据库和实时流的运行状态',
      action: () => handleAction('/status'),
    },
    {
      id: 'env',
      icon: <Settings size={16} />,
      title: '打开全局设置',
      shortcut: '/env',
      subtitle: '进入设置页面修改密钥、域名和链路配置',
      action: () => openTab('config'),
    },
  ]

  const filteredCommands = query.startsWith('/') ? commands.filter((c) => c.shortcut.startsWith(query.split(' ')[0])) : commands

  useEffect(() => {
    if (isOpen) {
      setTimeout(() => inputRef.current?.focus(), 50)
    }
  }, [isOpen])

  useEffect(() => {
    const normalized = query.trim()
    if (!isOpen || !normalized || normalized.startsWith('/')) {
      setSearchResults([])
      setIsSearching(false)
      return
    }

    const timeoutId = window.setTimeout(async () => {
      setIsSearching(true)
      try {
        const records = await fetchJson<EmailRecordApi[]>(`/api/emails?q=${encodeURIComponent(normalized)}&limit=8`)
        setSearchResults(
          records.map((record) => ({
            id: record.id,
            from: record.from_addr,
            to: record.to_addr,
            subject: record.subject || '无主题',
            time: new Date(record.created_at * 1000).toLocaleString(),
            code: record.extracted_code || '',
            link: record.extracted_link || undefined,
          })),
        )
      } finally {
        setIsSearching(false)
      }
    }, 180)

    return () => window.clearTimeout(timeoutId)
  }, [isOpen, query])

  const handleAction = async (cmd: string) => {
    try {
      let workflowId: string

      if (cmd.startsWith('/gen')) {
        const num = Number(cmd.split(' ')[1] || '10')
        workflowId = await resolveWorkflowId('account_generate')
        if (Number.isFinite(num)) {
          window.dispatchEvent(
            new CustomEvent('phantom-log', {
              detail: {
                msg: `命令面板已切换为真实工作流触发，请在工作流页面确认批量数量配置。当前目标工作流：${workflowId}，请求数量：${num}。`,
                level: 'info',
              },
            }),
          )
        }
      } else if (cmd.includes('/status')) {
        workflowId = await resolveWorkflowId('status_report')
      } else if (cmd.includes('/env')) {
        openTab('config')
        return
      } else {
        throw new Error(`未知指令：${cmd}`)
      }

      await postJson<{ status: string }, { workflow_id: string }>('/api/workflows/trigger', { workflow_id: workflowId })
      handleClose()
    } catch (e) {
      const message = e instanceof Error ? e.message : '后端响应异常'
      window.dispatchEvent(
        new CustomEvent('phantom-log', {
          detail: { msg: `指令执行失败：${message}`, level: 'warn' },
        }),
      )
      handleClose()
    }
  }

  const resolveWorkflowId = async (kind: WorkflowKind): Promise<string> => {
    const workflows = await fetchJson<WorkflowDefinition[]>('/api/workflows')
    const matched = workflows.find((workflow) => workflow.kind === kind && workflow.builtin) ?? workflows.find((workflow) => workflow.kind === kind)

    if (!matched) {
      throw new Error(`未找到可执行的工作流类型：${kind}`)
    }

    return matched.id
  }

  const openTab = (tab: AppTab) => {
    window.dispatchEvent(
      new CustomEvent('phantom-open-tab', {
        detail: { tab },
      }),
    )
    handleClose()
  }

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') handleClose()

    if (e.key === 'ArrowDown') {
      e.preventDefault()
      if (filteredCommands.length > 0) setSelectedIndex((prev) => (prev + 1) % filteredCommands.length)
    }

    if (e.key === 'ArrowUp') {
      e.preventDefault()
      if (filteredCommands.length > 0) setSelectedIndex((prev) => (prev - 1 + filteredCommands.length) % filteredCommands.length)
    }

    if (e.key === 'Enter') {
      e.preventDefault()
      if (query && !query.startsWith('/')) {
        window.dispatchEvent(
          new CustomEvent('phantom-open-emails', {
            detail: { query },
          }),
        )
        handleClose()
      } else {
        const target = filteredCommands[selectedIndex]
        if (target) target.action(query)
      }
    }
  }

  return (
    <AnimatePresence>
      {isOpen ? (
        <>
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={handleClose}
            className="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 transition-all cursor-none"
          />
          <motion.div
            initial={{ opacity: 0, scale: 0.95, y: -20 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.95, y: -20 }}
            className="fixed left-1/2 top-[15%] -translate-x-1/2 w-full max-w-xl z-[60] px-4"
          >
            <div className="bg-white/95 backdrop-blur-md rounded-2xl border border-slate-200 shadow-2xl overflow-hidden ring-1 ring-black/5">
              <div className="p-4 border-b border-slate-100 flex items-center gap-4 px-6 bg-slate-50/50">
                <Search size={20} className={`text-slate-400 ${query ? 'text-blue-500' : 'animate-pulse'}`} />
                <input
                  ref={inputRef}
                  value={query}
                  onChange={(e) => {
                    setQuery(e.target.value)
                    setSelectedIndex(0)
                  }}
                  onKeyDown={onKeyDown}
                  placeholder="输入 /gen 或搜索发件人、主题、验证码..."
                  className="bg-transparent border-none outline-none text-lg w-full text-slate-800 placeholder:text-slate-400 font-medium h-10"
                />
              </div>
              <div className="p-2 space-y-0.5 max-h-[400px] overflow-y-auto">
                <p className="text-[10px] font-bold text-slate-400 tracking-widest px-3 py-2">
                  {query && !query.startsWith('/') ? '数据湖检索' : '快捷指令'}
                </p>
                {!query || query.startsWith('/') ? (
                  filteredCommands.map((cmd, idx) => (
                    <CmdItem
                      key={cmd.id}
                      icon={cmd.icon}
                      title={cmd.title}
                      shortcut={cmd.shortcut}
                      subtitle={cmd.subtitle}
                      active={idx === selectedIndex}
                      onClick={() => cmd.action(query)}
                      onMouseEnter={() => setSelectedIndex(idx)}
                    />
                  ))
                ) : (
                  <div className="space-y-1">
                    {isSearching ? (
                      <div className="p-8 text-center flex flex-col items-center gap-3">
                        <Loader2 size={22} className="animate-spin text-blue-500" />
                        <div className="text-blue-500 font-mono text-xs">正在查询邮件索引...</div>
                        <div className="text-[10px] text-slate-400 mt-2 font-mono">当前关键词：{query}</div>
                      </div>
                    ) : searchResults.length > 0 ? (
                      searchResults.map((email, idx) => (
                        <CmdItem
                          key={email.id}
                          icon={<Mail size={16} />}
                          title={email.from}
                          shortcut={email.code || '无验证码'}
                          subtitle={`${email.subject} · ${email.time}`}
                          active={idx === selectedIndex}
                          onClick={() => {
                            window.dispatchEvent(
                              new CustomEvent('phantom-open-emails', {
                                detail: { query },
                              }),
                            )
                            handleClose()
                          }}
                          onMouseEnter={() => setSelectedIndex(idx)}
                        />
                      ))
                    ) : (
                      <div className="p-8 text-center flex flex-col items-center gap-3">
                        <Mail size={20} className="text-slate-300" />
                        <div className="text-slate-500 text-xs font-mono">未找到匹配邮件</div>
                        <div className="text-[10px] text-slate-400 font-mono">可尝试发件人、主题或验证码关键词</div>
                      </div>
                    )}
                  </div>
                )}
              </div>
              <div className="p-3 bg-slate-50 border-t border-slate-100 flex justify-between items-center px-5">
                <div className="flex gap-4">
                  <span className="text-[10px] flex items-center gap-1.5 text-slate-500 font-bold tracking-tight">
                    <kbd className="bg-white px-1.5 py-0.5 rounded border border-slate-200 text-slate-400 shadow-sm font-mono tracking-tighter text-[9px]">Enter</kbd>
                    选择执行
                  </span>
                  <span className="text-[10px] flex items-center gap-1.5 text-slate-500 font-bold tracking-tight">
                    <kbd className="bg-white px-1.5 py-0.5 rounded border border-slate-200 text-slate-400 shadow-sm font-mono tracking-tighter text-[9px]">↑↓</kbd>
                    快速导航
                  </span>
                </div>
                <span className="text-[10px] text-slate-300 font-mono font-bold tracking-widest leading-none">命令面板</span>
              </div>
            </div>
          </motion.div>
        </>
      ) : null}
    </AnimatePresence>
  )
}

interface CmdItemProps {
  icon: ReactNode
  title: string
  shortcut: string
  subtitle: string
  active?: boolean
  onClick?: () => void
  onMouseEnter?: () => void
}

function CmdItem({ icon, title, shortcut, subtitle, active, onClick, onMouseEnter }: CmdItemProps) {
  return (
    <div
      onClick={onClick}
      onMouseEnter={onMouseEnter}
      className={`flex items-center gap-4 p-3 rounded-xl border transition-all cursor-pointer group ${
        active ? 'bg-blue-500/10 border-blue-500/20 shadow-sm' : 'hover:bg-slate-50 border-transparent'
      }`}
    >
      <div
        className={`w-10 h-10 rounded-xl flex items-center justify-center transition-all shadow-inner border ${
          active ? 'bg-blue-500 text-white border-blue-400' : 'bg-slate-100/50 border-slate-200 text-slate-500 group-hover:text-blue-500 group-hover:bg-blue-50'
        }`}
      >
        {icon}
      </div>
      <div className="flex-grow">
        <div className="flex items-center justify-between">
          <span className={`font-bold text-sm transition-colors tracking-tight ${active ? 'text-blue-700' : 'text-slate-700'}`}>{title}</span>
          <span
            className={`text-[10px] font-mono px-2 py-0.5 rounded-full border transition-all font-bold ${
              active ? 'bg-blue-500 text-white border-blue-400' : 'bg-slate-100 text-slate-500 border-slate-200'
            }`}
          >
            {shortcut}
          </span>
        </div>
        <div className={`text-xs mt-0.5 transition-colors ${active ? 'text-blue-600/70' : 'text-slate-500'}`}>{subtitle}</div>
      </div>
    </div>
  )
}
