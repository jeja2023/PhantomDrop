import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react'
import { createPortal } from 'react-dom'

import {
  Mail,
  Search,
  Download,
  Loader2,
  ExternalLink,
  X,
  ChevronLeft,
  ChevronRight,
  ChevronDown,
  ArchiveRestore,
  Archive,
  Trash2,
  Copy,
  Database,
  RefreshCw,
  Trash,
  Key,
  ShieldCheck,
  Lock,
} from 'lucide-react'
import { deleteJson, fetchJson, postJson } from '../lib/api'
import { useClipboard } from '../ui/useClipboard'
import { useToast } from '../ui/Toast'
import { AccountStatusBadge } from '../ui/StatusBadge'
import { FieldCard } from '../ui/fields'
import ConfirmModal from '../ui/ConfirmModal'
import PromptModal from '../ui/PromptModal'
import type {
  EmailDetailApi,
  EmailItem,
  EmailPageResponse,
  EmailRecordApi,
  GeneratedAccountRecord,
  DashboardStats,
  LogLevel,
} from '../types'

// ==========================================
// 1. 临时收件箱相关类型与格式化辅助函数
// ==========================================
function formatEmail(record: EmailRecordApi): EmailItem {
  return {
    id: record.id,
    from: record.from_addr,
    to: record.to_addr,
    subject: record.subject || '无主题',
    time: new Date(record.created_at * 1000).toLocaleString(),
    code: record.extracted_code || '',
    link: record.extracted_link || undefined,
    isArchived: record.is_archived,
  }
}

function htmlToReadableText(html: string): string {
  if (!html.trim()) return ''

  const withLineBreaks = html
    .replace(/<script[\s\S]*?<\/script>/gi, ' ')
    .replace(/<style[\s\S]*?<\/style>/gi, ' ')
    .replace(/<\/(p|div|tr|h[1-6]|li)>/gi, '\n')
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<li[^>]*>/gi, '- ')
    .replace(/<[^>]+>/g, ' ')

  const textarea = document.createElement('textarea')
  textarea.innerHTML = withLineBreaks

  return textarea.value
    .replace(/\u00a0/g, ' ')
    .replace(/[ \t]+/g, ' ')
    .replace(/\n\s+/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim()
}

// ==========================================
// 2. 已生成账号相关类型定义
// ==========================================
interface AccountPageResponse {
  items: GeneratedAccountRecord[]
  limit: number
  offset: number
  total: number
}

type EmptyBody = Record<string, never>
type IdsBody = { ids: string[] }
type AccountIdsResponse = { status: string; ids: string[] }
type CheckStatusResponse = { status: string; account_status: string }
type BatchCheckStatusResponse = { status: string; results: Array<{ id: string; status: string }> }
type MessageResponse = { status: string; message: string }
type CleanupResponse = { status: string; deleted: number }

interface OAuthExportResponse {
  exported_at: string
  proxies: string[]
  accounts: Array<{
    name: string
    platform: string
    type: string
    credentials: Record<string, unknown>
    extra: {
      email: string
      privacy_mode: string
    }
    concurrency: number
    priority: number
    rate_multiplier: number
    auto_pause_on_expired: boolean
  }>
}

function emitLog(msg: string, level: LogLevel = 'info') {
  window.dispatchEvent(new CustomEvent('phantom-log', { detail: { msg, level } }))
}

// ==========================================
// 3. 主大一统数据资产收件箱中心组件
// ==========================================
export default function InboxCenterView({ defaultSearchQuery }: { defaultSearchQuery?: string }) {
  const [activeTab, setActiveTab] = useState<'emails' | 'accounts'>('emails')

  return (
    <div className="page-shell relative animate-in fade-in duration-700 flex flex-col min-h-full pb-8">
      {/* 顶部航母级分类大 Tab 栏 */}
      <div className="flex items-center gap-2 border-b border-slate-200 pb-3 mb-5 shrink-0">
        <button
          onClick={() => setActiveTab('emails')}
          className={`flex items-center gap-2.5 px-5 py-2.5 rounded-2xl text-xs font-black tracking-widest uppercase transition-all duration-300 ${
            activeTab === 'emails'
              ? 'bg-blue-600 text-white shadow-lg shadow-blue-500/20 border-transparent'
              : 'bg-slate-100 hover:bg-slate-200 text-slate-500 border border-slate-200/50'
          }`}
        >
          <Mail size={14} />
          📬 临时邮件捕获流 (Active Inbox)
        </button>

        <button
          onClick={() => setActiveTab('accounts')}
          className={`flex items-center gap-2.5 px-5 py-2.5 rounded-2xl text-xs font-black tracking-widest uppercase transition-all duration-300 ${
            activeTab === 'accounts'
              ? 'bg-indigo-600 text-white shadow-lg shadow-indigo-500/20 border-transparent'
              : 'bg-slate-100 hover:bg-slate-200 text-slate-500 border border-slate-200/50'
          }`}
        >
          <Database size={14} />
          🔑 已生成账户资产 (Generated Accounts)
        </button>
      </div>

      {/* 具体板块内容呈现 */}
      {activeTab === 'emails' ? <EmailListTab defaultSearchQuery={defaultSearchQuery} /> : <AccountListTab />}
    </div>
  )
}

// ==========================================
// 4. 临时收件箱 Tab 页面组件
// ==========================================
function EmailListTab({ defaultSearchQuery }: { defaultSearchQuery?: string }) {
  const showToast = useToast()
  const copy = useClipboard()
  const [emails, setEmails] = useState<EmailItem[]>([])
  const [isExporting, setIsExporting] = useState(false)
  const [query, setQuery] = useState(defaultSearchQuery || '')
  const [searching, setSearching] = useState(false)
  const [searchResults, setSearchResults] = useState<EmailItem[]>([])
  const [archivedFilter, setArchivedFilter] = useState<'all' | 'active' | 'archived'>('all')
  const [page, setPage] = useState(1)
  const [pageSize, setPageSize] = useState(10)
  const [total, setTotal] = useState(0)
  const [selectedIds, setSelectedIds] = useState<string[]>([])
  const [selectedEmail, setSelectedEmail] = useState<EmailDetailApi | null>(null)
  const [loadingDetail, setLoadingDetail] = useState(false)
  const [copiedField, setCopiedField] = useState<string | null>(null)

  // 初始加载历史邮件
  const loadInitialEmails = async () => {
    try {
      const data = await fetchJson<EmailRecordApi[]>('/api/emails')
      setEmails(data.map(formatEmail))
      if (query.trim() === '') {
        setTotal(data.length)
      }
    } catch (error) {
      console.error('Failed to load initial emails:', error)
    }
  }

  useEffect(() => {
    void loadInitialEmails()
  }, [])

  // 查询与检索过滤
  useEffect(() => {
    const timeoutId = window.setTimeout(async () => {
      setSearching(true)
      try {
        const normalized = query.trim()
        const archivedParam = archivedFilter === 'all' ? '' : `&archived=${archivedFilter === 'archived'}`
        const result = await fetchJson<EmailPageResponse>(
          `/api/emails/query?q=${encodeURIComponent(normalized)}&page=${page}&page_size=${pageSize}${archivedParam}`,
        )
        setSearchResults(result.items.map(formatEmail))
        setTotal(result.total)
      } catch (error) {
        console.error('Failed to query emails:', error)
      } finally {
        setSearching(false)
      }
    }, 220)

    return () => window.clearTimeout(timeoutId)
  }, [archivedFilter, page, pageSize, query])

  const visibleEmails = useMemo(() => {
    if (query.trim() !== '' || archivedFilter !== 'all') {
      return searchResults
    }
    // 未检索时使用内存加载的前十条或指定分页
    return searchResults.length > 0 ? searchResults : emails.slice((page - 1) * pageSize, page * pageSize)
  }, [emails, page, pageSize, query, searchResults, archivedFilter])

  // 提取正文内容
  const selectedEmailBody = useMemo(() => {
    const bodyText = selectedEmail?.body_text?.trim() || ''
    const bodyHtml = selectedEmail?.body_html?.trim() || ''

    if (bodyText) {
      return {
        displayText: bodyText,
        source: '纯文本正文',
        rawHtml: bodyHtml,
      }
    }

    if (bodyHtml) {
      return {
        displayText: htmlToReadableText(bodyHtml) || bodyHtml,
        source: 'HTML 正文已转文本',
        rawHtml: bodyHtml,
      }
    }

    return {
      displayText: '',
      source: '无正文',
      rawHtml: '',
    }
  }, [selectedEmail])

  const totalPages = Math.max(1, Math.ceil(total / pageSize))
  const allVisibleSelected = visibleEmails.length > 0 && visibleEmails.every((email) => selectedIds.includes(email.id))
  const archivedVisibleCount = visibleEmails.filter((email) => email.isArchived).length

  const refreshQueryUrl = `/api/emails/query?q=${encodeURIComponent(query.trim())}&page=${page}&page_size=${pageSize}${
    archivedFilter === 'all' ? '' : `&archived=${archivedFilter === 'archived'}`
  }`

  // 加载单封邮件深度解析
  const openEmailDetail = async (emailId: string) => {
    setLoadingDetail(true)
    try {
      const detail = await fetchJson<EmailDetailApi>(`/api/emails/${emailId}`)
      setSelectedEmail(detail)
    } catch (error) {
      const message = error instanceof Error ? error.message : '读取详情失败'
      emitLog(`读取邮件详情失败: ${message}`, 'warn')
    } finally {
      setLoadingDetail(false)
    }
  }

  // 归档或取消归档操作
  const toggleArchive = async (emailId: string, archived: boolean) => {
    await postJson<{ status: string }, { archived: boolean }>(`/api/emails/${emailId}/archive`, { archived })
    window.dispatchEvent(
      new CustomEvent('phantom-email-updated', {
        detail: { id: emailId, archived },
      }),
    )
    setSelectedEmail((current) => (current && current.id === emailId ? { ...current, is_archived: archived } : current))
    
    const refresh = await fetchJson<EmailPageResponse>(refreshQueryUrl)
    setSearchResults(refresh.items.map(formatEmail))
    setTotal(refresh.total)
    void loadInitialEmails()
  }

  // 选中项变更
  const toggleSelect = (emailId: string) => {
    setSelectedIds((current) => (current.includes(emailId) ? current.filter((id) => id !== emailId) : [...current, emailId]))
  }

  const toggleSelectAll = () => {
    if (allVisibleSelected) {
      setSelectedIds((current) => current.filter((id) => !visibleEmails.some((email) => email.id === id)))
    } else {
      setSelectedIds((current) => Array.from(new Set([...current, ...visibleEmails.map((email) => email.id)])))
    }
  }

  const refreshCurrentPage = async () => {
    const refresh = await fetchJson<EmailPageResponse>(refreshQueryUrl)
    if (refresh.items.length === 0 && refresh.total > 0 && page > 1) {
      setPage((current) => Math.max(1, current - 1))
      return
    }
    setSearchResults(refresh.items.map(formatEmail))
    setTotal(refresh.total)
  }

  const deleteEmail = async (emailId: string) => {
    await deleteJson<{ status: string }>(`/api/emails/${emailId}`)
    window.dispatchEvent(
      new CustomEvent('phantom-email-deleted', {
        detail: { id: emailId },
      }),
    )
    emitLog(`邮件已删除: ${emailId}`, 'success')
    setSelectedIds((current) => current.filter((id) => id !== emailId))
    setSelectedEmail((current) => (current && current.id === emailId ? null : current))
    await refreshCurrentPage()
    void loadInitialEmails()
  }

  const archiveSelected = async (archived: boolean) => {
    if (selectedIds.length === 0) return

    await postJson<{ status: string }, { ids: string[]; archived: boolean }>('/api/emails/batch/archive', {
      ids: selectedIds,
      archived,
    })
    for (const id of selectedIds) {
      window.dispatchEvent(
        new CustomEvent('phantom-email-updated', {
          detail: { id, archived },
        }),
      )
    }
    emitLog(`批量归档完成，共处理 ${selectedIds.length} 封邮件`, 'success')
    setSelectedIds([])
    await refreshCurrentPage()
    void loadInitialEmails()
  }

  const deleteSelected = async () => {
    if (selectedIds.length === 0) return

    await fetchJson<{ status: string }>('/api/emails/batch', {
      method: 'DELETE',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ ids: selectedIds }),
    })
    for (const id of selectedIds) {
      window.dispatchEvent(
        new CustomEvent('phantom-email-deleted', {
          detail: { id },
        }),
      )
    }
    emitLog(`批量删除完成，共处理 ${selectedIds.length} 封邮件`, 'success')
    setSelectedIds([])
    setSelectedEmail(null)
    await refreshCurrentPage()
    void loadInitialEmails()
  }

  const handleExport = () => {
    if (visibleEmails.length === 0) {
      emitLog('没有可导出的数据', 'info')
      return
    }

    setIsExporting(true)
    try {
      const headers = ['编号', '状态', '发件人', '收件人', '主题', '捕获时间', '验证码']
      const csvRows = [headers.join(',')]

      for (const email of visibleEmails) {
        const escapeCsv = (value: string) => `"${String(value).replace(/"/g, '""')}"`
        csvRows.push(
          [
            escapeCsv(email.id),
            escapeCsv(email.isArchived ? '已归档' : '已解析'),
            escapeCsv(email.from),
            escapeCsv(email.to),
            escapeCsv(email.subject),
            escapeCsv(email.time),
            escapeCsv(email.code || ''),
          ].join(','),
        )
      }

      const csvString = csvRows.join('\n')
      const blob = new Blob([new Uint8Array([0xef, 0xbb, 0xbf]), csvString], { type: 'text/csv;charset=utf-8;' })
      const link = document.createElement('a')
      const url = URL.createObjectURL(blob)
      link.setAttribute('href', url)
      link.setAttribute('download', `邮件导出_${Date.now()}.csv`)
      link.style.visibility = 'hidden'
      document.body.appendChild(link)
      link.click()
      document.body.removeChild(link)

      showToast({ title: '导出成功', desc: '邮件解析结果已从当前列表导出。' })
    } finally {
      setIsExporting(false)
    }
  }

  const copyField = async (label: string, value: string | null | undefined) => {
    if (!value) return
    await copy(value, { title: `${label} 已复制` })
    setCopiedField(label)
    setTimeout(() => setCopiedField((current) => (current === label ? null : current)), 1200)
  }

  const renderHighlightedBodyText = (text: string, code: string) => {
    if (!text) return <span className="text-slate-400 italic">未检测到邮件正文内容</span>
    if (!code || !text.includes(code)) return text

    const parts = text.split(code)
    return (
      <>
        {parts.map((part, index) => (
          <span key={index}>
            {part}
            {index < parts.length - 1 && (
              <span className="relative inline-block px-1.5 py-0.5 rounded-lg bg-blue-100 font-extrabold text-blue-800 shadow-sm mx-0.5 font-mono group/highlight overflow-hidden transition-all duration-300">
                <span className="absolute inset-0 bg-gradient-to-r from-blue-400/20 to-indigo-400/20 animate-pulse" />
                <span className="relative z-10">{code}</span>
              </span>
            )}
          </span>
        ))}
      </>
    )
  }

  return (
    <div className="flex flex-col flex-grow min-h-0">
      {/* 顶部控制栏与检索过滤 */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 shrink-0 mb-4">
        <div className="flex items-center gap-2">
          <div className="flex items-center gap-2 rounded-full border border-emerald-100 bg-emerald-50 px-3 py-1 shadow-sm">
            <div className="h-1.5 w-1.5 rounded-full bg-emerald-500 animate-ping"></div>
            <span className="text-[10px] font-black tracking-widest text-emerald-700">{query.trim() ? '检索中枢在线' : '中枢捕获同步中'}</span>
          </div>
        </div>

        <div className="flex items-center gap-2.5">
          <select
            value={archivedFilter}
            onChange={(event) => {
              setArchivedFilter(event.target.value as 'all' | 'active' | 'archived')
              setPage(1)
            }}
            className="phantom-select phantom-btn--sm"
            aria-label="筛选归档状态"
            title="筛选归档状态"
          >
            <option value="all">全部邮件</option>
            <option value="active">活跃邮件</option>
            <option value="archived">已归档邮件</option>
          </select>
          <div className="group flex items-center gap-2 rounded-lg border border-slate-200 bg-slate-100 px-3 py-1 transition-all focus-within:border-blue-400 focus-within:bg-white shadow-sm">
            <Search size={14} className="text-slate-600 group-focus-within:text-blue-500" />
            <input
              placeholder="快速检索发件人、主题、提取数据..."
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              className="w-48 bg-transparent border-none text-[10px] font-bold text-slate-900 placeholder:text-slate-600 outline-none"
            />
            {searching ? <Loader2 size={14} className="animate-spin text-slate-400" /> : null}
          </div>
          <button
            onClick={handleExport}
            disabled={isExporting}
            className={`phantom-btn phantom-btn--sm ${isExporting ? 'phantom-btn--muted' : 'phantom-btn--primary'} shadow-sm`}
          >
            <span className="flex items-center gap-2">
              {isExporting ? <Loader2 size={14} className="animate-spin" /> : <Download size={14} />}
              {isExporting ? '导出中...' : '列表 CSV 导出'}
            </span>
          </button>
        </div>
      </div>

      {/* 批量操作工具条 */}
      <div className="flex items-center justify-between rounded-2xl border border-slate-200 bg-white px-4 py-3 text-xs mb-4 shadow-sm shrink-0">
        <div className="font-bold text-slate-600 flex items-center gap-2">
          <span className="inline-block h-2 w-2 rounded-full bg-blue-500 animate-pulse" />
          已选择 <span className="text-blue-600 font-black font-mono">{selectedIds.length}</span> 项邮件数据
        </div>
        <div className="flex items-center gap-2">
          <button type="button" onClick={() => void archiveSelected(true)} disabled={selectedIds.length === 0} className="phantom-btn phantom-btn--sm phantom-btn--secondary">
            批量归档
          </button>
          <button type="button" onClick={() => void archiveSelected(false)} disabled={selectedIds.length === 0} className="phantom-btn phantom-btn--sm phantom-btn--secondary">
            取消归档
          </button>
          <button type="button" onClick={() => void deleteSelected()} disabled={selectedIds.length === 0} className="phantom-btn phantom-btn--sm phantom-btn--danger">
            批量删除
          </button>
        </div>
      </div>

      {/* 单栏满宽大网格表格 */}
      <div className="flex flex-col flex-grow min-h-0 bg-white rounded-3xl border border-slate-200 overflow-hidden shadow-sm mb-4">
        <div className="min-h-0 flex-grow overflow-auto custom-scrollbar">
          <table className="phantom-table">
            <thead className="sticky top-0 z-20">
              <tr>
                <th className="w-[56px] text-center text-[10px] font-bold">
                  <input type="checkbox" aria-label="全选当前页邮件" title="全选当前页邮件" checked={allVisibleSelected} onChange={toggleSelectAll} />
                </th>
                <th className="w-[80px] text-center text-[10px] font-bold">状态</th>
                <th className="w-[200px] text-left text-[10px] font-bold">发件人</th>
                <th className="w-[200px] text-left text-[10px] font-bold">收件人</th>
                <th className="w-[320px] text-left text-[10px] font-bold">主题摘要</th>
                <th className="w-[160px] text-left text-[10px] font-bold">捕获时间</th>
                <th className="w-[120px] text-right text-[10px] font-bold">提取数据</th>
              </tr>
            </thead>
            <tbody>
              {visibleEmails.length > 0 ? (
                visibleEmails.map((email) => {
                  const isRowSelected = selectedEmail?.id === email.id
                  return (
                    <tr
                      key={email.id}
                      className={`cursor-pointer hover:bg-slate-50/80 transition-colors ${
                        isRowSelected ? 'bg-blue-50/40 hover:bg-blue-50/60' : ''
                      }`}
                      onClick={() => void openEmailDetail(email.id)}
                    >
                      <td className="text-center" onClick={(event) => event.stopPropagation()}>
                        <input
                          type="checkbox"
                          aria-label={`选择邮件 ${email.id}`}
                          title={`选择邮件 ${email.id}`}
                          checked={selectedIds.includes(email.id)}
                          onChange={() => toggleSelect(email.id)}
                        />
                      </td>
                      <td className="text-center">
                        <span className={`whitespace-nowrap rounded-full px-1.5 py-0.5 text-[9px] font-black border ${
                          email.isArchived ? 'border-slate-200 bg-slate-100 text-slate-500' : 'border-emerald-100 bg-emerald-50 text-emerald-600'
                        }`}>
                          {email.isArchived ? '已归档' : '已解析'}
                        </span>
                      </td>
                      <td className="font-mono text-[11px] font-bold text-slate-800 truncate max-w-[200px]" title={email.from}>
                        {email.from}
                      </td>
                      <td className="font-mono text-[11px] font-bold text-blue-600 truncate max-w-[200px]" title={email.to}>
                        {email.to}
                      </td>
                      <td className="text-[11px] font-medium text-slate-600 truncate max-w-[320px]" title={email.subject}>
                        {email.subject}
                      </td>
                      <td className="text-[10px] font-mono text-slate-500">{email.time}</td>
                      <td className="text-right">
                        <div className="flex items-center justify-end gap-2">
                          <div className="font-mono text-xs font-black tracking-widest text-blue-600 bg-blue-50/80 px-1.5 py-0.5 rounded">
                            {email.code || '---'}
                          </div>
                          <button
                            type="button"
                            aria-label={email.isArchived ? '取消归档邮件' : '归档邮件'}
                            title={email.isArchived ? '取消归档邮件' : '归档邮件'}
                            onClick={(event) => {
                              event.stopPropagation()
                              void toggleArchive(email.id, !email.isArchived)
                            }}
                            className="rounded-lg p-1 text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-700"
                          >
                            {email.isArchived ? <ArchiveRestore size={14} /> : <Archive size={14} />}
                          </button>
                          <button
                            type="button"
                            aria-label="删除邮件"
                            title="删除邮件"
                            onClick={(event) => {
                              event.stopPropagation()
                              void deleteEmail(email.id)
                            }}
                            className="rounded-lg p-1 text-slate-400 transition-colors hover:bg-rose-50 hover:text-rose-600"
                          >
                            <Trash2 size={14} />
                          </button>
                        </div>
                      </td>
                    </tr>
                  )
                })
              ) : (
                <tr>
                  <td colSpan={7} className="py-20 text-center text-slate-700">
                    <div className="flex flex-col items-center gap-4 opacity-30">
                      <Mail size={32} />
                      <p className="text-[10px] font-black tracking-[0.4em]">{query.trim() ? '没有搜索结果' : '暂无邮件数据'}</p>
                    </div>
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>

        <div className="flex h-8 items-center justify-between border-t border-slate-200 bg-slate-50 px-4 text-[9px] font-bold tracking-widest text-slate-700 shrink-0">
          <div>总计邮件：{total}</div>
          <div className="flex items-center gap-4">
            <span>已归档：{archivedVisibleCount}</span>
            <span className="animate-pulse text-blue-500/40">当前页：{visibleEmails.length}</span>
            <span>页码：{page}/{totalPages}</span>
          </div>
        </div>
      </div>

      {/* 分页控制台 */}
      <div className="flex items-center justify-end gap-3 shrink-0 pt-2 border-t border-slate-100">
        <div className="mr-auto flex items-center gap-2">
          <span className="text-[10px] font-bold text-slate-500">每页条数</span>
          <select
            value={pageSize}
            onChange={(e) => {
              setPageSize(Number(e.target.value))
              setPage(1)
            }}
            className="phantom-select phantom-btn--sm py-0 h-8 shadow-sm"
            aria-label="选择每页显示条数"
          >
            {[10, 20, 50, 100].map((size) => (
              <option key={size} value={size}>{size} 条 / 页</option>
            ))}
          </select>
        </div>
        <button
          type="button"
          onClick={() => setPage((current) => Math.max(1, current - 1))}
          disabled={page <= 1 || searching}
          className="phantom-btn phantom-btn--sm phantom-btn--secondary shadow-sm"
        >
          <ChevronLeft size={14} />
          上一页
        </button>
        <button
          type="button"
          onClick={() => setPage((current) => Math.min(totalPages, current + 1))}
          disabled={page >= totalPages || searching}
          className="phantom-btn phantom-btn--sm phantom-btn--secondary shadow-sm"
        >
          下一页
          <ChevronRight size={14} />
        </button>
      </div>

      {/* 邮件深度解析超级磨砂 Modal 弹窗 */}
      {(selectedEmail || loadingDetail) && (
        <div
          className="fixed inset-0 z-[10000] flex items-center justify-center bg-slate-900/60 backdrop-blur-md animate-in fade-in duration-200"
          onClick={() => { if (!loadingDetail) setSelectedEmail(null) }}
        >
          <div
            className="w-[640px] max-w-[95vw] rounded-3xl border border-slate-200 bg-white shadow-2xl flex flex-col max-h-[85vh] overflow-hidden animate-in fade-in zoom-in-95 duration-200 relative"
            onClick={(e) => e.stopPropagation()}
          >
            {loadingDetail ? (
              <div className="h-[400px] flex flex-col items-center justify-center p-8 transition-all">
                <Loader2 size={36} className="animate-spin text-blue-500 mb-4" />
                <span className="font-bold tracking-widest text-xs text-blue-600 animate-pulse">正在解析邮件，智能捕获上下文验证码中...</span>
              </div>
            ) : selectedEmail ? (
              <>
                <div className="p-5 border-b border-slate-100 bg-gradient-to-br from-blue-50/50 via-indigo-50/30 to-purple-50/40 relative overflow-hidden shrink-0">
                  <div className="absolute inset-0 bg-white/30 backdrop-blur-[1px]" />
                  <div className="relative z-10 flex flex-col gap-3">
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2">
                        <span className="flex h-2.5 w-2.5 rounded-full bg-blue-500 animate-pulse" />
                        <span className="text-[10px] font-black tracking-widest text-slate-500 uppercase">邮件深度解析中枢</span>
                      </div>
                      <button
                        type="button"
                        onClick={() => setSelectedEmail(null)}
                        className="h-8 w-8 flex items-center justify-center rounded-xl text-slate-400 hover:bg-slate-100 hover:text-slate-800 transition-colors shadow-sm bg-white"
                        title="关闭详情"
                        aria-label="关闭详情"
                      >
                        <X size={14} />
                      </button>
                    </div>

                    <div className="grid grid-cols-2 gap-3">
                      <div className="group relative rounded-2xl border border-blue-100/80 bg-white/80 p-4 transition-all hover:border-blue-300 hover:shadow-lg hover:shadow-blue-500/5">
                        <div className="text-[9px] font-black text-blue-500 uppercase tracking-widest mb-1.5">验证码 / OTP</div>
                        <div className="flex items-center justify-between">
                          <span className="text-2xl font-black tracking-widest text-blue-600 font-mono">
                            {selectedEmail.extracted_code || '------'}
                          </span>
                          {selectedEmail.extracted_code && (
                            <button
                              type="button"
                              onClick={() => void copyField('code', selectedEmail.extracted_code)}
                              className="h-8 w-8 flex items-center justify-center rounded-xl bg-blue-50 text-blue-600 hover:bg-blue-600 hover:text-white transition-all shadow-sm"
                              title="复制验证码"
                              aria-label="复制验证码"
                            >
                              <Copy size={14} />
                            </button>
                          )}
                        </div>
                        {copiedField === 'code' && (
                          <div className="absolute top-2 right-3 text-[9px] font-black text-emerald-600 animate-in fade-in">已复制</div>
                        )}
                      </div>

                      <div className="group relative rounded-2xl border border-indigo-100/80 bg-white/80 p-4 transition-all hover:border-indigo-300 hover:shadow-lg hover:shadow-indigo-500/5">
                        <div className="text-[9px] font-black text-indigo-500 uppercase tracking-widest mb-1.5">跳转链接 / URL</div>
                        <div className="flex items-center justify-between">
                          <span className="text-xs font-bold text-slate-700 truncate max-w-[150px] font-mono">
                            {selectedEmail.extracted_link ? '包含激活链接' : '未检测到链接'}
                          </span>
                          {selectedEmail.extracted_link && (
                            <div className="flex gap-1 shrink-0">
                              <button
                                type="button"
                                onClick={() => void copyField('link', selectedEmail.extracted_link)}
                                className="h-8 w-8 flex items-center justify-center rounded-xl bg-indigo-50 text-indigo-600 hover:bg-indigo-600 hover:text-white transition-all shadow-sm"
                                title="复制链接"
                                aria-label="复制链接"
                              >
                                <Copy size={14} />
                              </button>
                              <a
                                href={selectedEmail.extracted_link}
                                target="_blank"
                                rel="noreferrer"
                                className="h-8 w-8 flex items-center justify-center rounded-xl bg-indigo-50 text-indigo-600 hover:bg-indigo-600 hover:text-white transition-all shadow-sm"
                                title="跳转访问"
                                aria-label="跳转访问"
                              >
                                <ExternalLink size={14} />
                              </a>
                            </div>
                          )}
                        </div>
                        {copiedField === 'link' && (
                          <div className="absolute top-2 right-3 text-[9px] font-black text-emerald-600 animate-in fade-in">已复制</div>
                        )}
                      </div>
                    </div>
                  </div>
                </div>

                <div className="flex-1 overflow-y-auto p-6 custom-scrollbar space-y-4 bg-slate-50/30">
                  <div className="grid grid-cols-2 gap-3">
                    <InfoCard label="捕获时间" value={new Date(selectedEmail.created_at * 1000).toLocaleString()} />
                    <InfoCard
                      label="归档状态"
                      value={selectedEmail.is_archived ? '已归档' : '活跃接收中'}
                      valueClassName={selectedEmail.is_archived ? 'text-slate-500' : 'text-emerald-600'}
                    />
                  </div>

                  <div className="space-y-3">
                    <InfoCard label="发件人 (FROM)" value={selectedEmail.from_addr} />
                    <InfoCard label="收件人 (TO)" value={selectedEmail.to_addr} />
                    <InfoCard label="主题 (SUBJECT)" value={selectedEmail.subject || '无主题'} />

                    <div className="group relative rounded-2xl border border-slate-200 bg-white p-5 transition-all hover:border-blue-200 hover:shadow-lg hover:shadow-blue-500/5">
                      <div className="flex items-center justify-between mb-3 border-b border-slate-100 pb-2">
                        <div>
                          <div className="text-[9px] font-black tracking-widest text-slate-400 uppercase">邮件正文 / MESSAGE BODY</div>
                          <div className="mt-0.5 text-[9px] font-black text-blue-500 uppercase">{selectedEmailBody.source}</div>
                        </div>
                        <button
                          type="button"
                          onClick={() => void copyField('text', selectedEmailBody.displayText)}
                          className="text-slate-400 hover:text-blue-500 transition-colors"
                          title="复制邮件正文"
                          aria-label="复制邮件正文"
                        >
                          <Copy size={13} />
                        </button>
                      </div>
                      {copiedField === 'text' && (
                        <div className="absolute top-4 right-10 text-[9px] font-black text-emerald-600 animate-in fade-in">已复制</div>
                      )}
                      <div className="max-h-[280px] overflow-y-auto custom-scrollbar mt-2 pr-1">
                        <pre className="whitespace-pre-wrap break-words text-[11px] leading-relaxed font-bold text-slate-700 selection:bg-blue-100">
                          {renderHighlightedBodyText(selectedEmailBody.displayText, selectedEmail.extracted_code || '')}
                        </pre>
                      </div>
                    </div>

                    {selectedEmailBody.rawHtml ? (
                      <div className="group relative rounded-2xl border border-slate-200 bg-white p-5 transition-all hover:border-slate-300">
                        <div className="flex items-center justify-between mb-2">
                          <div>
                            <div className="text-[9px] font-black tracking-widest text-slate-400 uppercase">原始 HTML / SOURCE CODE</div>
                            <div className="text-[9px] font-bold text-slate-400">用于排查高阶复杂模版</div>
                          </div>
                          <button
                            type="button"
                            onClick={() => void copyField('html', selectedEmailBody.rawHtml)}
                            className="text-slate-400 hover:text-blue-500 transition-colors"
                            title="复制原始 HTML"
                            aria-label="复制原始 HTML"
                          >
                            <Copy size={13} />
                          </button>
                        </div>
                        {copiedField === 'html' && (
                          <div className="absolute top-4 right-10 text-[9px] font-black text-emerald-600 animate-in fade-in">已复制</div>
                        )}
                        <div className="max-h-[160px] overflow-y-auto custom-scrollbar mt-2">
                          <pre className="whitespace-pre-wrap break-words text-[9px] leading-relaxed font-mono text-slate-500 selection:bg-blue-100 bg-slate-50 p-3 rounded-xl border border-slate-100">
                            {selectedEmailBody.rawHtml}
                          </pre>
                        </div>
                      </div>
                    ) : null}
                  </div>
                </div>
              </>
            ) : null}
          </div>
        </div>
      )}
    </div>
  )
}

function InfoCard({
  label,
  value,
  emphasize = false,
  action,
  actionLabel,
  valueClassName = '',
}: {
  label: string
  value: string
  emphasize?: boolean
  action?: ReactNode
  actionLabel?: string
  valueClassName?: string
}) {
  return (
    <div className="group rounded-2xl border border-slate-200 bg-white p-4 transition-all hover:border-slate-300 hover:bg-slate-50/50 shadow-sm flex-1">
      <div className="flex items-center justify-between gap-2 text-[9px] font-black tracking-widest text-slate-400 uppercase mb-1.5">
        <span>{label}</span>
        <div className="flex items-center gap-2">
          {actionLabel ? <span className="text-[10px] text-emerald-600 font-bold">{actionLabel}</span> : null}
          {action}
        </div>
      </div>
      <div className={`break-all leading-snug ${emphasize ? 'font-mono text-lg font-black tracking-widest text-blue-600' : 'text-[11px] font-bold text-slate-800'} ${valueClassName}`}>
        {value}
      </div>
    </div>
  )
}

// ==========================================
// 5. 已生成账号 Tab 页面组件
// ==========================================
function AccountListTab() {
  const [accounts, setAccounts] = useState<GeneratedAccountRecord[]>([])
  const [loading, setLoading] = useState(true)
  const [searchQuery, setSearchQuery] = useState('')
  const [search, setSearch] = useState('')
  const [page, setPage] = useState(1)
  const [pageSize, setPageSize] = useState(10)
  const [total, setTotal] = useState(0)
  const [selectedIds, setSelectedIds] = useState<string[]>([])
  const [checkingIds, setCheckingIds] = useState<string[]>([])
  const [selectedAccount, setSelectedAccount] = useState<GeneratedAccountRecord | null>(null)
  const [stats, setStats] = useState<DashboardStats | null>(null)
  const [oauthFolded, setOauthFolded] = useState(true)

  const [confirmConfig, setConfirmConfig] = useState<{
    title: string
    message: string
    tone?: 'danger' | 'info' | 'warn'
    onConfirm: () => void
  } | null>(null)

  const [promptConfig, setPromptConfig] = useState<{
    title: string
    message: string
    placeholder?: string
    defaultValue?: string
    onConfirm: (value: string) => void
  } | null>(null)

  useEffect(() => {
    const handler = setTimeout(() => {
      setSearch(searchQuery)
    }, 350)
    return () => clearTimeout(handler)
  }, [searchQuery])

  const showToast = useToast()
  const copy = useClipboard()

  const loadAccounts = useCallback(async () => {
    setLoading(true)
    try {
      const offset = (page - 1) * pageSize
      const queryParam = search ? `&q=${encodeURIComponent(search)}` : ''
      const data = await fetchJson<AccountPageResponse>(`/api/accounts?limit=${pageSize}&offset=${offset}${queryParam}`)
      setAccounts(data.items)
      setTotal(data.total)

      const statsData = await fetchJson<DashboardStats>('/api/stats')
      setStats(statsData)
    } catch (error) {
      console.error('Failed to load accounts:', error)
      emitLog(`加载账号列表失败: ${getErrorMessage(error)}`, 'error')
    } finally {
      setLoading(false)
    }
  }, [page, pageSize, search])

  useEffect(() => {
    void loadAccounts()
  }, [loadAccounts])

  const handleExport = () => {
    window.open('/api/workflow-runs/all/accounts/export', '_blank')
  }

  const copyToClipboard = (text: string) => {
    const message = text.length > 24 ? '数据已复制到剪贴板' : `已复制 ${text}`
    void copy(text, { title: message, desc: text.length > 24 ? `${text.slice(0, 20)}...` : undefined })
    emitLog(`用户复制了凭证数据: ${text.slice(0, 20)}...`)
  }

  const handleDelete = (id: string) => {
    setConfirmConfig({
      title: '删除账号记录',
      message: '确定要永久删除这条账号记录吗？此操作无法撤销。',
      tone: 'danger',
      onConfirm: async () => {
        setConfirmConfig(null)
        try {
          await deleteJson<MessageResponse>(`/api/accounts/${id}`)
          setAccounts((prev) => prev.filter((account) => account.id !== id))
          setSelectedIds((prev) => prev.filter((item) => item !== id))
          setTotal((prev) => Math.max(0, prev - 1))
          emitLog('账号记录已删除', 'success')
        } catch (error) {
          console.error('Failed to delete account:', error)
          emitLog(`删除失败: ${getErrorMessage(error)}`, 'error')
        }
      },
    })
  }

  const handleBatchDelete = () => {
    if (selectedIds.length === 0) return
    setConfirmConfig({
      title: '批量删除账号记录',
      message: `确定要永久删除选中的 ${selectedIds.length} 条账号记录吗？此操作无法撤销。`,
      tone: 'danger',
      onConfirm: async () => {
        setConfirmConfig(null)
        try {
          await fetchJson<MessageResponse>('/api/accounts/batch', {
            method: 'DELETE',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ ids: selectedIds }),
          })

          setAccounts((prev) => prev.filter((account) => !selectedIds.includes(account.id)))
          setTotal((prev) => Math.max(0, prev - selectedIds.length))
          setSelectedIds([])
          emitLog('批量删除完成', 'success')
        } catch (error) {
          console.error('Failed to batch delete:', error)
          emitLog(`批量删除失败: ${getErrorMessage(error)}`, 'error')
        }
      },
    })
  }

  const toggleSelect = (id: string) => {
    setSelectedIds((prev) => (prev.includes(id) ? prev.filter((item) => item !== id) : [...prev, id]))
  }

  const toggleSelectAll = () => {
    const allOnPageSelected = accounts.length > 0 && accounts.every((account) => selectedIds.includes(account.id))
    if (allOnPageSelected) {
      setSelectedIds((prev) => prev.filter((id) => !accounts.some((account) => account.id === id)))
      return
    }

    const pageIds = accounts.map((account) => account.id)
    setSelectedIds((prev) => Array.from(new Set([...prev, ...pageIds])))
  }

  const handleSelectAllAcrossPages = async () => {
    setLoading(true)
    try {
      const queryParam = search ? `?q=${encodeURIComponent(search)}` : ''
      const res = await fetchJson<AccountIdsResponse>(`/api/accounts/ids${queryParam}`)
      if (res.status === 'success') {
        setSelectedIds(res.ids)
        emitLog(`已选择当前筛选下的 ${res.ids.length} 条账号`, 'info')
      }
    } catch (error) {
      console.error('Failed to select all cross page:', error)
      emitLog(`跨页全选失败: ${getErrorMessage(error)}`, 'error')
    } finally {
      setLoading(false)
    }
  }

  const handleCheckStatus = async (id: string) => {
    if (checkingIds.includes(id)) return

    setCheckingIds((prev) => [...prev, id])
    try {
      const res = await postJson<CheckStatusResponse, EmptyBody>(`/api/accounts/${id}/check-status`, {})
      if (res.status === 'success') {
        setAccounts((prev) =>
          prev.map((account) => (account.id === id ? { ...account, status: res.account_status } : account)),
        )
        emitLog(`状态检查完成: ${res.account_status}`, 'success')
      }
    } catch (error) {
      console.error('Failed to check status:', error)
      emitLog(`状态检查失败: ${getErrorMessage(error)}`, 'error')
    } finally {
      setCheckingIds((prev) => prev.filter((item) => item !== id))
    }
  }

  const handleBatchCheckStatus = async () => {
    if (selectedIds.length === 0) return

    setLoading(true)
    try {
      const res = await postJson<BatchCheckStatusResponse, IdsBody>('/api/accounts/batch/check-status', { ids: selectedIds })
      if (res.status === 'success') {
        setAccounts((prev) =>
          prev.map((account) => {
            const result = res.results.find((item) => item.id === account.id)
            return result ? { ...account, status: result.status } : account
          }),
        )
        const message = `批量状态检查完成，共 ${res.results.length} 条`
        showToast(message)
        emitLog(message, 'success')
      }
    } catch (error) {
      console.error('Failed to batch check status:', error)
      emitLog(`批量状态检查失败: ${getErrorMessage(error)}`, 'error')
    } finally {
      setLoading(false)
    }
  }

  const handleBatchUploadCpa = async () => {
    if (selectedIds.length === 0) return

    setLoading(true)
    try {
      const res = await postJson<MessageResponse, IdsBody>('/api/accounts/batch/upload-cpa', { ids: selectedIds })
      if (res.status === 'success') {
        const message = `CPA 上传完成: ${res.message}`
        showToast(message)
        emitLog(message, 'success')
      }
    } catch (error) {
      console.error('Failed to upload CPA:', error)
      emitLog(`CPA 上传失败: ${getErrorMessage(error)}`, 'error')
    } finally {
      setLoading(false)
    }
  }

  const handleBatchUpdatePool = () => {
    if (selectedIds.length === 0) return

    setPromptConfig({
      title: '批量修改分组池',
      message: '请输入新的分组池标签（例如: vip、default、free-4o）:',
      placeholder: '分组池标签',
      defaultValue: '',
      onConfirm: async (newTag) => {
        const trimmed = newTag.trim()
        if (!trimmed) {
          showToast({ title: '输入验证失败', desc: '分池标签不能为空', tone: 'error' })
          return
        }

        setPromptConfig(null)
        setLoading(true)
        try {
          const res = await postJson<{ status: string; message: string }, { ids: string[]; pool_tag: string }>(
            '/api/accounts/batch/update-pool',
            { ids: selectedIds, pool_tag: trimmed },
          )
          if (res.status === 'success') {
            showToast(res.message)
            emitLog(res.message, 'success')
            void loadAccounts()
            setSelectedIds([])
          }
        } catch (error) {
          console.error('Failed to update pool tag:', error)
          emitLog(`批量设置分组失败: ${getErrorMessage(error)}`, 'error')
        } finally {
          setLoading(false)
        }
      },
    })
  }

  const handleCleanupFailures = () => {
    setConfirmConfig({
      title: '清理失败账号记录',
      message: '确定要清理非 Registered/Success 状态的失败账号吗？此操作无法撤销。',
      tone: 'warn',
      onConfirm: async () => {
        setConfirmConfig(null)
        try {
          const res = await postJson<CleanupResponse, EmptyBody>('/api/accounts/cleanup-failures', {})
          emitLog(`清理完成，删除 ${res.deleted || 0} 条失败账号`, 'success')
          void loadAccounts()
        } catch (error) {
          console.error('Failed to cleanup failures:', error)
          emitLog(`清理失败账号失败: ${getErrorMessage(error)}`, 'error')
        }
      },
    })
  }

  const handleBatchExportJson = async () => {
    if (selectedIds.length === 0) return

    setLoading(true)
    try {
      const res = await postJson<GeneratedAccountRecord[], IdsBody>('/api/accounts/batch/export', { ids: selectedIds })
      const blob = new Blob([JSON.stringify(res, null, 2)], { type: 'application/json' })
      const url = URL.createObjectURL(blob)
      const link = document.createElement('a')
      link.href = url
      link.download = `accounts_export_${Date.now()}.json`
      link.click()
      URL.revokeObjectURL(url)
      emitLog(`已导出 ${res.length} 条账号为 JSON`, 'success')
    } catch (error) {
      console.error('Failed to export JSON:', error)
      emitLog(`JSON 导出失败: ${getErrorMessage(error)}`, 'error')
    } finally {
      setLoading(false)
    }
  }

  const handleBatchExportOauthJson = async () => {
    if (selectedIds.length === 0) return

    setLoading(true)
    try {
      const res = await postJson<OAuthExportResponse, IdsBody>('/api/accounts/batch/export?format=oauth', { ids: selectedIds })
      const blob = new Blob([JSON.stringify(res, null, 2)], { type: 'application/json' })
      const url = URL.createObjectURL(blob)
      const link = document.createElement('a')
      link.href = url
      link.download = `oauth_accounts_export_${Date.now()}.json`
      link.click()
      URL.revokeObjectURL(url)
      emitLog(`已成功导出 ${res.accounts?.length || 0} 条 OAuth 格式账号为 JSON`, 'success')
    } catch (error) {
      console.error('Failed to export OAuth JSON:', error)
      emitLog(`OAuth JSON 导出失败: ${getErrorMessage(error)}`, 'error')
    } finally {
      setLoading(false)
    }
  }

  const handleBatchExportSub2apiJson = async () => {
    if (selectedIds.length === 0) return

    setLoading(true)
    try {
      const res = await postJson<Record<string, unknown>[], IdsBody>('/api/accounts/batch/export?format=sub2api', { ids: selectedIds })
      const blob = new Blob([JSON.stringify(res, null, 2)], { type: 'application/json' })
      const url = URL.createObjectURL(blob)
      const link = document.createElement('a')
      link.href = url
      link.download = `sub2api_accounts_export_${Date.now()}.json`
      link.click()
      URL.revokeObjectURL(url)
      emitLog(`已成功导出 ${res.length} 条 Sub2API 格式账号为 JSON`, 'success')
    } catch (error) {
      console.error('Failed to export Sub2API:', error)
      emitLog(`Sub2API JSON 导出失败: ${getErrorMessage(error)}`, 'error')
    } finally {
      setLoading(false)
    }
  }

  const getErrorMessage = (error: unknown, fallback = '网络请求失败'): string => {
    return error instanceof Error ? error.message : fallback
  }

  const totalPages = Math.max(1, Math.ceil(total / pageSize))
  const allOnPageSelected = accounts.length > 0 && accounts.every((account) => selectedIds.includes(account.id))

  return (
    <div className="flex flex-col flex-grow min-h-0">
      {/* 顶栏控制台与全量操作 */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 shrink-0 mb-4">
        <div className="flex items-center gap-2">
          <div className="flex items-center gap-2.5 rounded-full border border-indigo-100 bg-indigo-50 px-3.5 py-1 shadow-sm">
            <div className="h-1.5 w-1.5 rounded-full bg-indigo-500 animate-ping mr-1"></div>
            <span className="text-[10px] font-black tracking-widest text-indigo-700 uppercase">资产就绪</span>
          </div>
        </div>

        <div className="flex items-center gap-2.5">
          <button
            onClick={() => void loadAccounts()}
            className="phantom-btn phantom-btn--secondary phantom-btn--sm flex items-center gap-1.5 shadow-sm"
            disabled={loading}
          >
            <RefreshCw size={12} className={loading ? 'animate-spin' : ''} />
            刷新资产
          </button>
          <button
            onClick={() => void handleCleanupFailures()}
            className="phantom-btn phantom-btn--secondary phantom-btn--sm hover:text-rose-600 flex items-center gap-1.5 shadow-sm"
            title="清理非 Registered / Success 的失败账号"
          >
            <Trash size={12} />
            清理失败
          </button>
          <button
            onClick={handleExport}
            className="phantom-btn phantom-btn--primary phantom-btn--sm flex items-center gap-1.5 shadow-sm"
            title="导出全部账号为 CSV 文件"
          >
            <Download size={12} />
            全量 CSV 导出
          </button>
        </div>
      </div>

      {/* 搜索控制条 */}
      <div className="grid grid-cols-1 lg:grid-cols-4 gap-4 mb-4 shrink-0">
        <div className="lg:col-span-2 glass-panel rounded-2xl py-2 px-4 border border-slate-200 shadow-sm flex items-center gap-4 h-12">
          <div className="flex items-center gap-2 text-indigo-600 shrink-0">
            <Search size={14} />
            <h3 className="text-xs font-black uppercase tracking-wider">资产检索</h3>
          </div>
          <div className="relative flex-grow group">
            <input
              type="text"
              placeholder="防抖模糊检索邮箱 / 状态 / RunID..."
              value={searchQuery}
              onChange={(event) => {
                setSearchQuery(event.target.value)
                setPage(1)
              }}
              className="w-full bg-slate-50 border border-slate-200 rounded-xl px-4 py-2 text-xs font-bold outline-none focus:border-indigo-500 focus:bg-white transition-all pl-9 pr-9 h-8 shadow-inner"
            />
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400 group-focus-within:text-indigo-500 transition-colors" size={13} />
            {searchQuery && (
              <button
                type="button"
                onClick={() => {
                  setSearchQuery('')
                  setSearch('')
                  setPage(1)
                }}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-600 transition-colors cursor-pointer p-0.5 rounded-full hover:bg-slate-200 flex items-center justify-center h-5 w-5"
                title="清空搜索"
              >
                <X size={12} />
              </button>
            )}
          </div>
        </div>

        {/* 聚合资产微指标 */}
        <div className="lg:col-span-2 grid grid-cols-3 gap-3">
          <div className="glass-panel rounded-2xl py-2 px-4 border border-slate-200 shadow-sm flex flex-col justify-center h-12 bg-white">
            <span className="text-[8px] font-black text-slate-400 uppercase tracking-widest leading-none mb-1">已捕获总数</span>
            <span className="text-sm font-black text-slate-800 leading-none tabular-nums tracking-tight">
              {stats?.total_accounts ?? total} 个
            </span>
          </div>
          <div className="glass-panel rounded-2xl py-2 px-4 border border-slate-200 shadow-sm flex flex-col justify-center h-12 bg-white">
            <span className="text-[8px] font-black text-slate-400 uppercase tracking-widest leading-none mb-1">今日新增</span>
            <span className="text-sm font-black text-emerald-600 leading-none tabular-nums tracking-tight">
              +{stats?.today_accounts_24h ?? 0} 个
            </span>
          </div>
          <div className="glass-panel rounded-2xl py-2 px-4 border border-slate-200 shadow-sm flex flex-col justify-center h-12 bg-white">
            <span className="text-[8px] font-black text-slate-400 uppercase tracking-widest leading-none mb-1">池账号</span>
            <span className="text-sm font-black text-indigo-600 leading-none tabular-nums tracking-tight">
              {stats?.active_pool_accounts ?? 0} 挂载
            </span>
          </div>
        </div>
      </div>

      {/* 批量操作工具条 */}
      <div className="flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-slate-200 bg-white px-4 py-3 text-xs mb-4 shadow-sm shrink-0">
        <div className="font-bold text-slate-600 flex flex-wrap items-center gap-2">
          <span className="inline-block h-2 w-2 rounded-full bg-indigo-500 animate-pulse" />
          已选择 <span className="text-indigo-600 font-black font-mono">{selectedIds.length}</span> 项账户资产
          {selectedIds.length > 0 && (
            <>
              <span className="text-slate-300">|</span>
              <button
                onClick={handleSelectAllAcrossPages}
                className="text-indigo-600 hover:text-indigo-800 font-black tracking-wider transition-colors cursor-pointer text-[10px] uppercase"
              >
                (跨页全选所有检索项)
              </button>
            </>
          )}
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <button
            onClick={handleBatchCheckStatus}
            disabled={selectedIds.length === 0}
            className="phantom-btn phantom-btn--sm phantom-btn--secondary"
            title="对选中的账号批量发起中枢底层状态健康检查"
          >
            批量探活
          </button>
          <button
            onClick={handleBatchUploadCpa}
            disabled={selectedIds.length === 0}
            className="phantom-btn phantom-btn--sm phantom-btn--secondary"
            title="将选中账号批量同步至CPA管理系统"
          >
            批量同步CPA
          </button>
          <button
            onClick={handleBatchUpdatePool}
            disabled={selectedIds.length === 0}
            className="phantom-btn phantom-btn--sm phantom-btn--secondary"
            title="将选中账号批量划分至指定分组或功能池"
          >
            批量分池
          </button>
          <div className="h-4 w-px bg-slate-200" />
          <button
            onClick={handleBatchExportJson}
            disabled={selectedIds.length === 0}
            className="phantom-btn phantom-btn--sm phantom-btn--secondary text-indigo-600 hover:text-indigo-800"
            title="导出为标准 JSON 文件"
          >
            JSON 导出
          </button>
          <button
            onClick={handleBatchExportOauthJson}
            disabled={selectedIds.length === 0}
            className="phantom-btn phantom-btn--sm phantom-btn--secondary text-indigo-600 hover:text-indigo-800"
            title="导出为标准 OAuth JSON 配置"
          >
            OAuth 导出
          </button>
          <button
            onClick={handleBatchExportSub2apiJson}
            disabled={selectedIds.length === 0}
            className="phantom-btn phantom-btn--sm phantom-btn--secondary text-indigo-600 hover:text-indigo-800"
            title="导出为 Sub2API JSON 凭证格式"
          >
            Sub2API 导出
          </button>
          <div className="h-4 w-px bg-slate-200" />
          <button
            onClick={handleBatchDelete}
            disabled={selectedIds.length === 0}
            className="phantom-btn phantom-btn--sm phantom-btn--danger"
          >
            批量删除
          </button>
        </div>
      </div>

      {/* 数据表格网格 */}
      <div className="flex flex-col flex-grow min-h-0 bg-white rounded-3xl border border-slate-200 overflow-hidden shadow-sm mb-4">
        <div className="min-h-0 flex-grow overflow-auto custom-scrollbar">
          <table className="phantom-table">
            <thead className="sticky top-0 z-20">
              <tr>
                <th className="w-[56px] text-center text-[10px] font-bold">
                  <input
                    type="checkbox"
                    aria-label="全选当前页账号"
                    title="全选当前页账号"
                    checked={allOnPageSelected}
                    onChange={toggleSelectAll}
                  />
                </th>
                <th className="w-[140px] text-center text-[10px] font-bold">账号状态</th>
                <th className="w-[260px] text-left text-[10px] font-bold">注册邮箱 (Email)</th>
                <th className="w-[110px] text-left text-[10px] font-bold">分池标签</th>
                <th className="w-[180px] text-left text-[10px] font-bold">关联工作流任务</th>
                <th className="w-[140px] text-left text-[10px] font-bold">注册时间</th>
                <th className="w-[100px] text-right text-[10px] font-bold">数据探活</th>
              </tr>
            </thead>
            <tbody>
              {accounts.length > 0 ? (
                accounts.map((account) => {
                  const isChecked = selectedIds.includes(account.id)
                  const isChecking = checkingIds.includes(account.id)
                  return (
                    <tr
                      key={account.id}
                      onClick={() => setSelectedAccount(account)}
                      className={`cursor-pointer transition-all duration-300 hover:bg-slate-50/80 ${
                        selectedAccount?.id === account.id ? 'bg-indigo-50/30' : ''
                      }`}
                    >
                      <td className="text-center" onClick={(e) => e.stopPropagation()}>
                        <input
                          type="checkbox"
                          aria-label={`选择账号 ${account.id}`}
                          title={`选择账号 ${account.id}`}
                          checked={isChecked}
                          onChange={() => toggleSelect(account.id)}
                        />
                      </td>
                      <td className="text-center">
                        <AccountStatusBadge status={account.status} />
                      </td>
                      <td className="font-mono text-[11px] font-bold text-slate-800 break-all select-all">
                        {account.address}
                      </td>
                      <td>
                        <span className="px-2 py-0.5 rounded-lg border border-indigo-50 bg-indigo-50/40 text-indigo-600 text-[10px] font-black tracking-wide font-mono">
                          {account.pool_tag || 'default'}
                        </span>
                      </td>
                      <td className="font-mono text-[10px] text-slate-500 font-bold truncate max-w-[180px]" title={account.run_id}>
                        {account.run_id}
                      </td>
                      <td className="text-[10px] font-mono text-slate-400 font-bold">
                        {new Date(account.created_at * 1000).toLocaleString()}
                      </td>
                      <td className="text-right" onClick={(e) => e.stopPropagation()}>
                        <div className="flex items-center justify-end gap-1.5">
                          <button
                            onClick={() => void handleCheckStatus(account.id)}
                            disabled={isChecking}
                            className="phantom-btn phantom-btn--secondary phantom-btn--sm min-h-7 h-7 font-black text-[10px] px-2 shadow-sm flex items-center justify-center gap-1 shrink-0"
                            title="对该账号发起探活"
                          >
                            <RefreshCw size={10} className={isChecking ? 'animate-spin' : ''} />
                            {isChecking ? '探活中' : '探活'}
                          </button>
                          <button
                            onClick={() => void handleDelete(account.id)}
                            className="p-1 rounded-lg text-slate-400 hover:bg-rose-50 hover:text-rose-600 transition-colors"
                            title="删除该账号记录"
                            aria-label="删除该账号记录"
                          >
                            <Trash2 size={13} />
                          </button>
                        </div>
                      </td>
                    </tr>
                  )
                })
              ) : (
                <tr>
                  <td colSpan={7} className="py-20 text-center text-slate-400 font-bold">
                    {loading ? (
                      <div className="flex flex-col items-center gap-3">
                        <Loader2 className="animate-spin text-indigo-500" size={24} />
                        <span className="text-[10px] font-black text-indigo-600 uppercase tracking-widest animate-pulse">正在穿透数据库，加载账号资产中...</span>
                      </div>
                    ) : (
                      '无匹配的账号资产记录'
                    )}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>

        <div className="flex h-8 items-center justify-between border-t border-slate-200 bg-slate-50 px-4 text-[9px] font-bold tracking-widest text-slate-700 shrink-0">
          <div>总计资产：{total} 个账户</div>
          <div className="flex items-center gap-4">
            <span className="animate-pulse text-indigo-500/50">当前页展示：{accounts.length} 条</span>
            <span>页码：{page}/{totalPages}</span>
          </div>
        </div>
      </div>

      {/* 分页控制台 */}
      <div className="flex items-center justify-end gap-3 shrink-0 pt-2 border-t border-slate-100">
        <div className="mr-auto flex items-center gap-2">
          <span className="text-[10px] font-bold text-slate-500">每页条数</span>
          <select
            value={pageSize}
            onChange={(e) => {
              setPageSize(Number(e.target.value))
              setPage(1)
            }}
            className="phantom-select phantom-btn--sm py-0 h-8 shadow-sm"
            aria-label="选择每页显示条数"
          >
            {[10, 20, 50, 100].map((size) => (
              <option key={size} value={size}>{size} 条 / 页</option>
            ))}
          </select>
        </div>
        <button
          type="button"
          onClick={() => setPage((current) => Math.max(1, current - 1))}
          disabled={page <= 1 || loading}
          className="phantom-btn phantom-btn--sm phantom-btn--secondary shadow-sm"
        >
          <ChevronLeft size={14} />
          上一页
        </button>
        <button
          type="button"
          onClick={() => setPage((current) => Math.min(totalPages, current + 1))}
          disabled={page >= totalPages || loading}
          className="phantom-btn phantom-btn--sm phantom-btn--secondary shadow-sm"
        >
          下一页
          <ChevronRight size={14} />
        </button>
      </div>

      {/* 已生成账号的 JSON 高级折叠抽屉与三件套快捷复制超级 Modal 弹窗 */}
      {selectedAccount && (
        <AccountDetailModal
          account={selectedAccount}
          oauthFolded={oauthFolded}
          setOauthFolded={setOauthFolded}
          onClose={() => setSelectedAccount(null)}
          copyToClipboard={copyToClipboard}
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

      {promptConfig && (
        <PromptModal
          isOpen={true}
          title={promptConfig.title}
          message={promptConfig.message}
          placeholder={promptConfig.placeholder}
          defaultValue={promptConfig.defaultValue}
          onConfirm={promptConfig.onConfirm}
          onCancel={() => setPromptConfig(null)}
        />
      )}
    </div>
  )
}

// ==========================================
interface AccountDetailModalProps {
  account: GeneratedAccountRecord
  oauthFolded: boolean
  setOauthFolded: (folded: boolean) => void
  onClose: () => void
  copyToClipboard: (text: string) => void
}

function AccountDetailModal({
  account,
  oauthFolded,
  setOauthFolded,
  onClose,
  copyToClipboard,
}: AccountDetailModalProps) {
  const credentials = useMemo(() => {
    try {
      if (account.oauth_credentials_json) {
        return typeof account.oauth_credentials_json === 'string'
          ? (JSON.parse(account.oauth_credentials_json) as Record<string, unknown>)
          : (account.oauth_credentials_json as unknown as Record<string, unknown>)
      }
      return {}
    } catch {
      return {}
    }
  }, [account.oauth_credentials_json])

  const sessionKey = account.session_token || (credentials.session_key as string) || ''
  const accessToken = account.access_token || (credentials.access_token as string) || ''
  const apiSecret = account.password || ''

  // 拼接 Session 完整串
  const formattedSession = `session_key=${sessionKey}; access_token=${accessToken}`

  return createPortal(
    <div
      className="fixed inset-0 z-[10000] flex items-center justify-center bg-slate-900/60 backdrop-blur-md animate-in fade-in duration-200"
      onClick={onClose}
    >
      <div
        className="w-[680px] max-w-[95vw] bg-white rounded-3xl border border-slate-200 shadow-2xl flex flex-col max-h-[85vh] overflow-hidden relative animate-in fade-in zoom-in-95 duration-200"
        onClick={(e: React.MouseEvent) => e.stopPropagation()}
      >
        {/* 顶部标题区 */}
        <div className="p-5 border-b border-slate-100 bg-gradient-to-br from-indigo-50/50 via-purple-50/20 to-blue-50/30 shrink-0 relative overflow-hidden">
          <div className="absolute inset-0 bg-white/20 backdrop-blur-[1px]" />
          <div className="relative z-10 flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="w-9 h-9 rounded-xl bg-indigo-50 text-indigo-600 flex items-center justify-center shadow-inner border border-indigo-150/40">
                <Key size={16} />
              </div>
              <div>
                <h3 className="text-sm font-black text-slate-800 leading-none mb-1">
                  凭证与提取参数详情 (CREDENTIALS)
                </h3>
                <span className="font-mono text-[9px] text-slate-450 leading-none uppercase tracking-widest">
                  Secure Credentials Vault
                </span>
              </div>
            </div>

            <button
              onClick={onClose}
              className="h-8 w-8 flex items-center justify-center rounded-xl bg-white text-slate-400 hover:text-slate-800 transition-colors shadow-sm border border-slate-150/50 hover:bg-slate-50"
              title="关闭凭证库"
              aria-label="关闭凭证库"
            >
              <X size={14} />
            </button>
          </div>
        </div>

        {/* 滚动内容区域 */}
        <div className="flex-grow overflow-y-auto p-6 custom-scrollbar bg-slate-50/40 space-y-5">
          {/* 一键复制三件套聚合面板 */}
          <div className="rounded-2xl border border-indigo-100 bg-white p-5 shadow-sm space-y-4">
            <div className="flex items-center gap-2 border-b border-slate-100 pb-2 mb-1">
              <ShieldCheck className="text-emerald-500" size={14} />
              <span className="text-[10px] font-black uppercase text-indigo-600 tracking-wider">
                聚合提取三件套 (QUICK COPY PANEL)
              </span>
            </div>

            <div className="space-y-3">
              {/* API 密匙 / Session_key */}
              <div className="group relative rounded-xl border border-slate-150 bg-slate-50/30 p-3 hover:bg-slate-50 transition-colors">
                <div className="text-[8px] font-bold text-slate-400 uppercase mb-1 flex items-center justify-between">
                  <span>Session Key (API 登录密钥)</span>
                  <button
                    onClick={() => copyToClipboard(sessionKey)}
                    disabled={!sessionKey}
                    className="text-indigo-600 hover:text-indigo-800 flex items-center gap-1 font-bold h-4"
                    title="复制 API 登录密钥"
                  >
                    <Copy size={11} /> 复制
                  </button>
                </div>
                <div className="font-mono text-[10px] text-slate-850 break-all pr-8 select-all font-bold">
                  {sessionKey || '--- 未生成 ---'}
                </div>
              </div>

              {/* JWT 权限 Token */}
              <div className="group relative rounded-xl border border-slate-150 bg-slate-50/30 p-3 hover:bg-slate-50 transition-colors">
                <div className="text-[8px] font-bold text-slate-400 uppercase mb-1 flex items-center justify-between">
                  <span>Access Token (JWT 访问签名)</span>
                  <button
                    onClick={() => copyToClipboard(accessToken)}
                    disabled={!accessToken}
                    className="text-indigo-600 hover:text-indigo-800 flex items-center gap-1 font-bold h-4"
                    title="复制 JWT 访问签名"
                  >
                    <Copy size={11} /> 复制
                  </button>
                </div>
                <div className="font-mono text-[10px] text-slate-500 break-all pr-8 select-all truncate">
                  {accessToken || '--- 未生成 ---'}
                </div>
              </div>

              {/* Session 完整装配串 */}
              <div className="group relative rounded-xl border border-indigo-150 bg-indigo-50/10 p-3 hover:bg-indigo-50/30 transition-colors">
                <div className="text-[8px] font-black text-indigo-500 uppercase mb-1 flex items-center justify-between">
                  <span>Session 拼装串 (直接装配至浏览器登录 COOKIE)</span>
                  <button
                    onClick={() => copyToClipboard(formattedSession)}
                    disabled={!sessionKey && !accessToken}
                    className="text-indigo-600 hover:text-indigo-800 flex items-center gap-1 font-black h-4"
                    title="复制拼装完整 Session 串"
                  >
                    <Copy size={11} /> 快捷复制完整 Session
                  </button>
                </div>
                <div className="font-mono text-[10px] text-indigo-900 break-all pr-8 select-all truncate font-bold">
                  {sessionKey || accessToken ? formattedSession : '--- 未生成 ---'}
                </div>
              </div>
            </div>
          </div>

          {/* 属性网格 */}
          <div className="grid grid-cols-2 gap-3">
            <FieldCard label="数据库记录 ID (DB_INDEX)" value={account.id} />
            <FieldCard label="中枢分组池标签 (POOL_TAG)" value={account.pool_tag || 'default'} />
            <FieldCard label="邮箱账号 (EMAIL)" value={account.address} />
            <FieldCard label="代理服务节点 (PROXY_NODE)" value={account.proxy_url || '无使用代理'} />
            <FieldCard label="关联并发工作流 (RUN_ID)" value={account.run_id} />
            <FieldCard label="安全登录密匙 (AUTH_SECRET)" value={apiSecret || '---'} />
          </div>

          {/* 底层 OAuth 与 凭证完整 JSON 折叠抽屉 */}
          <div className="rounded-2xl border border-slate-200 bg-white overflow-hidden shadow-sm transition-all hover:border-slate-350">
            <button
              type="button"
              onClick={() => setOauthFolded(!oauthFolded)}
              className="w-full flex items-center justify-between px-5 py-3.5 bg-slate-50 border-b border-slate-100 hover:bg-slate-100 transition-colors"
            >
              <div className="flex items-center gap-2">
                <Database size={13} className="text-slate-500" />
                <span className="text-[10px] font-black text-slate-700 tracking-wider uppercase">
                  底层凭证完整 JSON (CREDENTIALS DUMP)
                </span>
              </div>
              <ChevronDown
                size={14}
                className={`text-slate-400 transition-transform duration-300 ${oauthFolded ? '' : 'rotate-180'}`}
              />
            </button>

            {!oauthFolded && (
              <div className="p-5 bg-slate-900 relative">
                <button
                  type="button"
                  onClick={() => copyToClipboard(JSON.stringify(credentials, null, 2))}
                  className="absolute top-4 right-4 text-slate-400 hover:text-white transition-colors p-1"
                  title="复制 JSON DUMP"
                >
                  <Copy size={13} />
                </button>
                <pre className="text-[9px] font-mono leading-relaxed text-slate-200 select-all overflow-x-auto max-h-[220px] custom-scrollbar">
                  {JSON.stringify(credentials, null, 2)}
                </pre>
              </div>
            )}
          </div>
        </div>

        {/* 凭证库页脚 */}
        <div className="p-4 border-t border-slate-100 bg-slate-50 flex items-center justify-between text-[8px] font-mono text-slate-400 shrink-0">
          <span>SAFE_CREDS_HASH: {account.id.slice(0, 16).toUpperCase()}</span>
          <span className="flex items-center gap-1 text-indigo-500 font-bold uppercase tracking-widest">
            <Lock size={10} /> phantom secure vault
          </span>
        </div>
      </div>
    </div>,
    document.body,
  )
}
