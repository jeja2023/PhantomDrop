import { useEffect, useMemo, useState, type ReactNode } from 'react'
import {
  Mail,
  Search,
  Download,
  CheckCircle2,
  Loader2,
  ExternalLink,
  X,
  ChevronLeft,
  ChevronRight,
  ArchiveRestore,
  Archive,
  Trash2,
  Copy,
} from 'lucide-react'
import { deleteJson, fetchJson, postJson } from '../lib/api'
import PageHeader from '../ui/PageHeader'
import type {
  EmailDetailApi,
  EmailItem,
  EmailPageResponse,
  EmailRecordApi,
  PhantomEmailDeletedDetail,
  PhantomEmailUpdatedDetail,
} from '../types'

function formatEmail(record: EmailRecordApi): EmailItem {
  return {
    id: record.id,
    from: record.from_addr,
    subject: record.subject || '无主题',
    time: new Date(record.created_at * 1000).toLocaleString(),
    code: record.extracted_code || '',
    link: record.extracted_link || undefined,
    isArchived: record.is_archived,
  }
}

export default function EmailListView({ emails, externalQuery = '' }: { emails: EmailItem[]; externalQuery?: string }) {
  const [isExporting, setIsExporting] = useState(false)
  const [showToast, setShowToast] = useState(false)
  const [query, setQuery] = useState('')
  const [searching, setSearching] = useState(false)
  const [searchResults, setSearchResults] = useState<EmailItem[]>([])
  const [archivedFilter, setArchivedFilter] = useState<'all' | 'active' | 'archived'>('all')
  const [page, setPage] = useState(1)
  const [pageSize] = useState(20)
  const [total, setTotal] = useState(emails.length)
  const [hasLoadedPage, setHasLoadedPage] = useState(false)
  const [selectedIds, setSelectedIds] = useState<string[]>([])
  const [selectedEmail, setSelectedEmail] = useState<EmailDetailApi | null>(null)
  const [loadingDetail, setLoadingDetail] = useState(false)
  const [copiedField, setCopiedField] = useState<string | null>(null)

  useEffect(() => {
    setQuery(externalQuery)
    setPage(1)
  }, [externalQuery])

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
        setHasLoadedPage(true)
      } finally {
        setSearching(false)
      }
    }, 220)

    return () => window.clearTimeout(timeoutId)
  }, [archivedFilter, page, pageSize, query])

  const visibleEmails = useMemo(() => (hasLoadedPage ? searchResults : emails), [emails, hasLoadedPage, searchResults])
  const totalPages = Math.max(1, Math.ceil(total / pageSize))
  const allVisibleSelected = visibleEmails.length > 0 && visibleEmails.every((email) => selectedIds.includes(email.id))
  const archivedVisibleCount = visibleEmails.filter((email) => email.isArchived).length

  const refreshQueryUrl = `/api/emails/query?q=${encodeURIComponent(query.trim())}&page=${page}&page_size=${pageSize}${
    archivedFilter === 'all' ? '' : `&archived=${archivedFilter === 'archived'}`
  }`

  const openEmailDetail = async (emailId: string) => {
    setLoadingDetail(true)
    try {
      const detail = await fetchJson<EmailDetailApi>(`/api/emails/${emailId}`)
      setSelectedEmail(detail)
    } catch (error) {
      const message = error instanceof Error ? error.message : '读取邮件详情失败'
      window.dispatchEvent(
        new CustomEvent('phantom-log', {
          detail: { msg: `读取邮件详情失败：${message}`, level: 'warn' },
        }),
      )
    } finally {
      setLoadingDetail(false)
    }
  }

  const toggleArchive = async (emailId: string, archived: boolean) => {
    await postJson<{ status: string }, { archived: boolean }>(`/api/emails/${emailId}/archive`, { archived })
    window.dispatchEvent(
      new CustomEvent<PhantomEmailUpdatedDetail>('phantom-email-updated', {
        detail: { id: emailId, archived },
      }),
    )
    setSelectedEmail((current) => (current && current.id === emailId ? { ...current, is_archived: archived } : current))
    const refresh = await fetchJson<EmailPageResponse>(refreshQueryUrl)
    setSearchResults(refresh.items.map(formatEmail))
    setTotal(refresh.total)
  }

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
      new CustomEvent<PhantomEmailDeletedDetail>('phantom-email-deleted', {
        detail: { id: emailId },
      }),
    )
    window.dispatchEvent(
      new CustomEvent('phantom-log', {
        detail: { msg: `邮件已删除：${emailId}`, level: 'success' },
      }),
    )
    setSelectedIds((current) => current.filter((id) => id !== emailId))
    setSelectedEmail((current) => (current && current.id === emailId ? null : current))
    await refreshCurrentPage()
  }

  const archiveSelected = async (archived: boolean) => {
    if (selectedIds.length === 0) return

    await postJson<{ status: string }, { ids: string[]; archived: boolean }>('/api/emails/batch/archive', {
      ids: selectedIds,
      archived,
    })
    for (const id of selectedIds) {
      window.dispatchEvent(
        new CustomEvent<PhantomEmailUpdatedDetail>('phantom-email-updated', {
          detail: { id, archived },
        }),
      )
    }
    window.dispatchEvent(
      new CustomEvent('phantom-log', {
        detail: { msg: `批量归档完成，共处理 ${selectedIds.length} 封邮件`, level: 'success' },
      }),
    )
    setSelectedIds([])
    await refreshCurrentPage()
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
        new CustomEvent<PhantomEmailDeletedDetail>('phantom-email-deleted', {
          detail: { id },
        }),
      )
    }
    window.dispatchEvent(
      new CustomEvent('phantom-log', {
        detail: { msg: `批量删除完成，共处理 ${selectedIds.length} 封邮件`, level: 'success' },
      }),
    )
    setSelectedIds([])
    setSelectedEmail(null)
    await refreshCurrentPage()
  }

  const handleExport = () => {
    if (visibleEmails.length === 0) {
      window.dispatchEvent(
        new CustomEvent('phantom-log', {
          detail: { msg: '没有可导出的数据', level: 'info' },
        }),
      )
      return
    }

    setIsExporting(true)

    try {
      const headers = ['编号', '状态', '发件人', '摘要', '捕获时间', '验证码']
      const csvRows = [headers.join(',')]

      for (const email of visibleEmails) {
        const escapeCsv = (value: string) => `"${String(value).replace(/"/g, '""')}"`
        csvRows.push(
          [
            escapeCsv(email.id),
            escapeCsv(email.isArchived ? '已归档' : '已解析'),
            escapeCsv(email.from),
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

      setShowToast(true)
      setTimeout(() => setShowToast(false), 3000)
    } finally {
      setIsExporting(false)
    }
  }

  const copyField = async (label: string, value: string | null | undefined) => {
    if (!value) return
    await navigator.clipboard.writeText(value)
    setCopiedField(label)
    setTimeout(() => setCopiedField((current) => (current === label ? null : current)), 1200)
  }

  return (
    <div className="page-shell page-shell--full relative animate-in fade-in duration-700">
      <div className={`fixed right-10 top-20 z-[100] transform transition-all duration-500 ${showToast ? 'translate-y-0 opacity-100' : '-translate-y-12 pointer-events-none opacity-0'}`}>
        <div className="flex items-center gap-3 rounded-2xl border border-blue-100 bg-white px-6 py-3 shadow-2xl shadow-blue-500/10">
          <CheckCircle2 className="text-emerald-500" size={20} />
          <div className="flex flex-col">
            <span className="text-sm font-bold text-slate-800">导出成功</span>
            <span className="text-[10px] font-mono text-slate-500">邮件解析结果已导出为 CSV 文件。</span>
          </div>
        </div>
      </div>

      <PageHeader
        title="邮件解析列表"
        kicker="邮件解析数据库"
        description="按后端真实分页接口检索邮件、查看详情、归档、删除，并导出当前筛选结果。"
        status={
          <div className="flex items-center gap-2 rounded-full border border-emerald-100 bg-emerald-50 px-3 py-1.5">
            <div className="h-1.5 w-1.5 rounded-full bg-emerald-500"></div>
            <span className="text-[10px] font-black tracking-widest text-emerald-700">{query.trim() ? '远程检索中' : '实时同步中'}</span>
          </div>
        }
        actions={
          <>
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
              <option value="all">全部</option>
              <option value="active">未归档</option>
              <option value="archived">已归档</option>
            </select>
            <div className="group flex items-center gap-2 rounded-lg border border-slate-200 bg-slate-100 px-3 py-1.5 transition-all focus-within:border-blue-400 focus-within:bg-white">
              <Search size={14} className="text-slate-600 group-focus-within:text-blue-500" />
              <input
                placeholder="搜索发件人、主题、验证码..."
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                className="w-48 bg-transparent border-none text-[10px] font-bold text-slate-900 placeholder:text-slate-600 outline-none"
              />
              {searching ? <Loader2 size={14} className="animate-spin text-slate-400" /> : null}
            </div>
            <button
              onClick={handleExport}
              disabled={isExporting}
              className={`phantom-btn phantom-btn--sm ${isExporting ? 'phantom-btn--muted' : 'phantom-btn--primary'}`}
            >
              <span className="flex items-center gap-2">
                {isExporting ? <Loader2 size={14} className="animate-spin" /> : <Download size={14} />}
                {isExporting ? '生成中...' : '导出'}
              </span>
            </button>
          </>
        }
      />

      <div className="flex items-center justify-between rounded-2xl border border-slate-200 bg-white px-4 py-3 text-xs">
        <div className="font-bold text-slate-600">已选择 {selectedIds.length} 项</div>
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

      <div className="page-panel flex min-h-0 flex-grow flex-col overflow-hidden rounded-3xl border border-slate-200 bg-white">
        <div className="min-h-0 flex-grow overflow-auto custom-scrollbar">
          <table className="phantom-table">
            <thead className="sticky top-0 z-20">
              <tr>
                <th className="w-[56px] text-center text-[10px] font-bold">
                  <input type="checkbox" aria-label="全选当前页邮件" title="全选当前页邮件" checked={allVisibleSelected} onChange={toggleSelectAll} />
                </th>
                <th className="w-[120px] text-center text-[10px] font-bold">状态</th>
                <th className="w-[240px] text-left text-[10px] font-bold">发件人</th>
                <th className="text-left text-[10px] font-bold">摘要</th>
                <th className="w-[180px] text-left text-[10px] font-bold">捕获时间</th>
                <th className="w-[140px] text-right text-[10px] font-bold">验证码</th>
              </tr>
            </thead>
            <tbody>
              {visibleEmails.length > 0 ? (
                visibleEmails.map((email) => (
                  <tr key={email.id} className="cursor-pointer hover:bg-slate-50/80" onClick={() => void openEmailDetail(email.id)}>
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
                      <span className={`whitespace-nowrap rounded-full px-2 py-0.5 text-[9px] font-black border ${
                        email.isArchived ? 'border-slate-200 bg-slate-100 text-slate-500' : 'border-emerald-100 bg-emerald-50 text-emerald-600'
                      }`}>
                        {email.isArchived ? '已归档' : '已解析'}
                      </span>
                    </td>
                    <td className="font-mono text-xs font-bold text-slate-800">{email.from}</td>
                    <td className="text-xs font-medium text-slate-600">{email.subject}</td>
                    <td className="text-[10px] font-mono text-slate-500">{email.time}</td>
                    <td className="text-right">
                      <div className="flex items-center justify-end gap-2">
                        <div className="font-mono text-base font-black tracking-widest text-blue-600">{email.code || '---'}</div>
                        <button
                          type="button"
                          aria-label={email.isArchived ? '取消归档邮件' : '归档邮件'}
                          title={email.isArchived ? '取消归档邮件' : '归档邮件'}
                          onClick={(event) => {
                            event.stopPropagation()
                            void toggleArchive(email.id, !email.isArchived)
                          }}
                          className="rounded-lg p-1.5 text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-700"
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
                          className="rounded-lg p-1.5 text-slate-400 transition-colors hover:bg-rose-50 hover:text-rose-600"
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    </td>
                  </tr>
                ))
              ) : (
                <tr>
                  <td colSpan={6} className="py-20 text-center text-slate-700">
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

        <div className="flex h-8 items-center justify-between border-t border-slate-200 bg-slate-50 px-4 text-[9px] font-bold tracking-widest text-slate-700">
          <div>总条数：{total}</div>
          <div className="flex items-center gap-4">
            <span>已归档：{archivedVisibleCount}</span>
            <span className="animate-pulse text-blue-500/40">当前可见：{visibleEmails.length}</span>
            <span>页码：{page}/{totalPages}</span>
          </div>
        </div>
      </div>

      <div className="flex items-center justify-end gap-3">
        <button type="button" onClick={() => setPage((current) => Math.max(1, current - 1))} disabled={page <= 1 || searching} className="phantom-btn phantom-btn--sm phantom-btn--secondary">
          <ChevronLeft size={14} />
          上一页
        </button>
        <button
          type="button"
          onClick={() => setPage((current) => Math.min(totalPages, current + 1))}
          disabled={page >= totalPages || searching}
          className="phantom-btn phantom-btn--sm phantom-btn--secondary"
        >
          下一页
          <ChevronRight size={14} />
        </button>
      </div>

      {selectedEmail || loadingDetail ? (
        <div className="fixed inset-0 z-[110] flex items-center justify-center bg-slate-900/40 p-4 backdrop-blur-sm sm:p-6">
          <div className="flex w-full max-w-4xl max-h-full flex-col overflow-hidden rounded-3xl border border-slate-200 bg-white shadow-2xl">
            <div className="flex shrink-0 items-center justify-between border-b border-slate-100 px-6 py-4">
              <div className="flex flex-col">
                <h3 className="text-lg font-black leading-tight text-slate-900">邮件详情</h3>
                <p className="text-[10px] font-mono tracking-widest text-slate-500 uppercase mt-0.5">详情视图</p>
              </div>
              <button
                type="button"
                aria-label="关闭邮件详情"
                title="关闭邮件详情"
                onClick={() => setSelectedEmail(null)}
                className="rounded-xl p-2 text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-700"
              >
                <X size={18} />
              </button>
            </div>

            {loadingDetail ? (
              <div className="flex flex-1 items-center justify-center text-slate-500">
                <Loader2 size={18} className="mr-3 animate-spin" />
                正在读取邮件详情...
              </div>
            ) : selectedEmail ? (
              <div className="grid flex-1 gap-4 overflow-y-auto p-5 lg:grid-cols-[1.2fr_1fr]">
                <div className="space-y-3">
                  <InfoCard label="发件人" value={selectedEmail.from_addr} />
                  <InfoCard label="收件人" value={selectedEmail.to_addr} />
                  <InfoCard label="主题" value={selectedEmail.subject || '无主题'} />
                  <div className="rounded-2xl border border-slate-200 bg-slate-50 p-3.5">
                    <div className="text-[10px] font-mono text-slate-500">纯文本正文</div>
                    <pre className="mt-2 whitespace-pre-wrap break-words text-xs leading-relaxed text-slate-700">{selectedEmail.body_text || '无纯文本内容'}</pre>
                  </div>
                </div>

                <div className="space-y-3">
                  <InfoCard label="捕获时间" value={new Date(selectedEmail.created_at * 1000).toLocaleString()} />
                  <InfoCard label="归档状态" value={selectedEmail.is_archived ? '已归档' : '活跃'} />
                  <InfoCard
                    label="提取验证码"
                    value={selectedEmail.extracted_code || '----'}
                    emphasize
                    action={
                      selectedEmail.extracted_code ? (
                        <button
                          type="button"
                          title="复制验证码"
                          onClick={() => void copyField('code', selectedEmail.extracted_code)}
                          className="phantom-btn phantom-btn--sm phantom-btn--secondary phantom-btn--icon"
                        >
                          <Copy size={14} />
                        </button>
                      ) : undefined
                    }
                    actionLabel={copiedField === 'code' ? '已复制' : undefined}
                  />
                  <div className="flex gap-3">
                    <button type="button" onClick={() => void toggleArchive(selectedEmail.id, !selectedEmail.is_archived)} className="phantom-btn phantom-btn--sm phantom-btn--secondary flex-1">
                      {selectedEmail.is_archived ? <ArchiveRestore size={14} /> : <Archive size={14} />}
                      {selectedEmail.is_archived ? '取消归档' : '归档邮件'}
                    </button>
                    <button type="button" onClick={() => void deleteEmail(selectedEmail.id)} className="phantom-btn phantom-btn--sm phantom-btn--danger flex-1">
                      <Trash2 size={14} />
                      删除邮件
                    </button>
                  </div>
                  <div className="rounded-2xl border border-slate-200 bg-slate-50 p-3.5">
                    <div className="flex items-center justify-between gap-2 text-xs font-mono text-slate-500">
                      <span>提取链接</span>
                      {selectedEmail.extracted_link ? (
                        <button
                          type="button"
                          title="复制链接"
                          onClick={() => void copyField('link', selectedEmail.extracted_link)}
                          className="phantom-btn phantom-btn--sm phantom-btn--secondary phantom-btn--icon"
                        >
                          <Copy size={14} />
                        </button>
                      ) : null}
                    </div>
                    {selectedEmail.extracted_link ? (
                      <a href={selectedEmail.extracted_link} target="_blank" rel="noreferrer" className="mt-2 inline-flex items-center gap-2 break-all text-sm font-bold text-blue-600 hover:text-blue-700">
                        <ExternalLink size={14} />
                        {selectedEmail.extracted_link}
                      </a>
                    ) : (
                      <div className="mt-1 text-sm text-slate-500">未提取到链接</div>
                    )}
                  </div>
                  <div className="rounded-2xl border border-slate-200 bg-slate-50 p-3.5">
                    <div className="flex items-center justify-between gap-2 text-[10px] font-mono text-slate-500">
                      <span>HTML 片段</span>
                      {selectedEmail.body_html ? (
                        <button
                          type="button"
                          title="复制 HTML 片段"
                          onClick={() => void copyField('html', selectedEmail.body_html)}
                          className="phantom-btn phantom-btn--sm phantom-btn--secondary phantom-btn--icon"
                        >
                          <Copy size={14} />
                        </button>
                      ) : null}
                    </div>
                    <pre className="mt-2 max-h-[200px] overflow-auto whitespace-pre-wrap break-words text-[10px] leading-relaxed text-slate-500">{selectedEmail.body_html || '无 HTML 内容'}</pre>
                  </div>
                </div>
              </div>
            ) : null}
          </div>
        </div>
      ) : null}
    </div>
  )
}

function InfoCard({
  label,
  value,
  emphasize = false,
  action,
  actionLabel,
}: {
  label: string
  value: string
  emphasize?: boolean
  action?: ReactNode
  actionLabel?: string
}) {
  return (
    <div className="rounded-2xl border border-slate-200 bg-slate-50 p-3.5">
      <div className="flex items-center justify-between gap-2 text-[11px] font-mono text-slate-500">
        <span>{label}</span>
        <div className="flex items-center gap-2">
          {actionLabel ? <span className="text-[10px] text-emerald-600">{actionLabel}</span> : null}
          {action}
        </div>
      </div>
      <div className={`mt-0.5 break-all ${emphasize ? 'font-mono text-base font-black tracking-widest text-blue-600' : 'text-sm font-bold text-slate-900'}`}>{value}</div>
    </div>
  )
}
