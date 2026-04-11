import { useEffect, useRef, useState } from 'react'
import { ExternalLink, ShieldCheck, Zap, Power, Server, Copy, CheckCircle2 } from 'lucide-react'
import { motion, AnimatePresence } from 'framer-motion'
import { fetchJson, postJson } from '../lib/api'
import PageHeader from '../ui/PageHeader'
import type { TunnelStatus } from '../types'

const defaultStatus: TunnelStatus = {
  active: false,
  url: null,
  port: 4000,
  subdomain: '',
  provider: 'manual',
}

export default function TunnelView() {
  const [status, setStatus] = useState<TunnelStatus>(defaultStatus)
  const [loading, setLoading] = useState(false)
  const [copied, setCopied] = useState(false)
  const [port, setPort] = useState(4000)
  const [subdomain, setSubdomain] = useState('')
  const [publicUrl, setPublicUrl] = useState('')
  const draftDirtyRef = useRef(false)

  const syncDraftFromStatus = (data: TunnelStatus) => {
    setPort(data.port || 4000)
    setSubdomain(data.subdomain || '')
    setPublicUrl(data.url || '')
    draftDirtyRef.current = false
  }

  const fetchStatus = async () => {
    const data = await fetchJson<TunnelStatus>('/api/tunnel/status')
    setStatus(data)

    if (data.active || !draftDirtyRef.current) {
      syncDraftFromStatus(data)
    }
  }

  useEffect(() => {
    const fetchStatus = async () => {
      const data = await fetchJson<TunnelStatus>('/api/tunnel/status')
      setStatus(data)

      if (data.active || !draftDirtyRef.current) {
        syncDraftFromStatus(data)
      }
    }

    void fetchStatus()
    const interval = setInterval(() => {
      void fetchStatus()
    }, 3000)
    return () => clearInterval(interval)
  }, [])

  const handlePortChange = (value: string) => {
    const nextPort = Number(value)
    setPort(Number.isFinite(nextPort) ? nextPort : 0)
    draftDirtyRef.current = true
  }

  const handleSubdomainChange = (value: string) => {
    setSubdomain(value)
    draftDirtyRef.current = true
  }

  const handlePublicUrlChange = (value: string) => {
    setPublicUrl(value)
    draftDirtyRef.current = true
  }

  const handleToggle = async () => {
    setLoading(true)
    try {
      if (status.active) {
        await postJson<{ status: string }, Record<string, never>>('/api/tunnel/stop', {})
      } else {
        await postJson<{ status: string; url: string }, { port: number; subdomain?: string; public_url: string }>(
          '/api/tunnel/start',
          {
            port,
            subdomain: subdomain.trim() || undefined,
            public_url: publicUrl.trim(),
          },
        )
      }

      draftDirtyRef.current = false
      await fetchStatus()
    } finally {
      setLoading(false)
    }
  }

  const copyToClipboard = () => {
    if (!status.url) {
      return
    }

    navigator.clipboard.writeText(status.url)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <div className="page-shell page-shell--full space-y-2 animate-in fade-in duration-700 overflow-hidden">
      <PageHeader
        title=""
        kicker=""
        description=""
        status={
          <div
            className={`rounded-full border px-3 py-1.5 text-[10px] font-black tracking-widest ${
              status.active
                ? 'border-emerald-100 bg-emerald-50 text-emerald-700'
                : 'border-slate-200 bg-slate-50 text-slate-600'
            }`}
          >
            {status.active ? '运行中' : '离线'}
          </div>
        }
      />

      <div className="grid grid-cols-1 gap-2 md:grid-cols-3 shrink-0">
        <div
          className={`p-3 rounded-2xl border transition-all duration-500 ${
            status.active
              ? 'bg-emerald-50/50 border-emerald-100 shadow-lg shadow-emerald-500/10'
              : 'bg-slate-50 border-slate-200'
          }`}
        >
          <div className="flex justify-between items-start mb-2">
            <div className={`p-2 rounded-lg ${status.active ? 'bg-emerald-500 text-white' : 'bg-slate-200 text-slate-400'}`}>
              <Power size={16} />
            </div>
            <div
              className={`text-[9px] font-bold px-2 py-0.5 rounded-full tracking-tighter ${
                status.active ? 'bg-emerald-500 text-white' : 'bg-slate-300 text-slate-600'
              }`}
            >
              {status.active ? '已激活' : '已断开'}
            </div>
          </div>
          <div className="text-[26px] font-black text-slate-900 font-mono mb-1 leading-none">
            {status.active ? 'ACTIVE' : 'IDLE'}
          </div>
          <div className="text-[9px] text-slate-500 font-bold tracking-widest font-mono">地址登记状态</div>
        </div>

        <div className="p-3 rounded-2xl bg-white border border-slate-200 shadow-sm">
          <div className="flex justify-between items-start mb-2">
            <div className="p-2 rounded-lg bg-blue-600 text-white shadow-lg shadow-blue-500/20">
              <Server size={16} />
            </div>
          </div>
          <div className="text-[26px] font-black text-slate-900 font-mono mb-1 leading-none">{status.port}</div>
          <div className="text-[9px] text-slate-500 font-bold tracking-widest font-mono">本地映射端口</div>
        </div>

        <div className="p-3 rounded-2xl bg-white border border-slate-200 shadow-sm flex flex-col justify-center">
          <div className="text-[9px] text-slate-500 font-bold tracking-widest font-mono mb-1.5">安全防护</div>
          <div className="flex items-center gap-2">
            <ShieldCheck size={15} className="text-blue-600 shrink-0" />
            <span className="text-[12px] font-bold text-slate-800">由外部隧道负责公网接入</span>
          </div>
          <div className="mt-1.5 text-[9px] text-slate-400 leading-tight">
            中枢仅保存最终公网地址，不直接管理第三方隧道进程。
          </div>
        </div>
      </div>

      <section className="page-panel overflow-visible divide-y divide-slate-100 shadow-sm shrink-0">
        <div className="p-4">
          <h3 className="text-[15px] font-bold text-slate-900 mb-2.5 flex items-center gap-2">
            <Zap size={16} className="text-blue-600" />
            快速部署配置
          </h3>

          <div className="grid grid-cols-1 gap-3 mb-4 md:grid-cols-[140px_minmax(0,1.4fr)_minmax(0,1fr)]">
            <div className="space-y-1.5">
              <label className="text-[10px] font-bold text-slate-400 tracking-widest ml-1">本地端口</label>
              <input
                type="number"
                value={port}
                onChange={(e) => handlePortChange(e.target.value)}
                disabled={status.active}
                placeholder="例如: 4000"
                className="phantom-input w-full"
              />
            </div>

            <div className="space-y-1.5">
              <label className="text-[10px] font-bold text-slate-400 tracking-widest ml-1">公网地址</label>
              <input
                type="url"
                value={publicUrl}
                onChange={(e) => handlePublicUrlChange(e.target.value)}
                disabled={status.active}
                placeholder="例如: https://phantom.example.com"
                className="phantom-input w-full"
              />
            </div>

            <div className="space-y-1.5">
              <label className="text-[10px] font-bold text-slate-400 tracking-widest ml-1">备注二级域名（可选）</label>
              <input
                type="text"
                value={subdomain}
                onChange={(e) => handleSubdomainChange(e.target.value)}
                disabled={status.active}
                placeholder="例如: phantom-hub-01"
                className="phantom-input w-full"
              />
            </div>
          </div>

          <button
            onClick={handleToggle}
            disabled={loading}
            className={`phantom-btn w-full ${status.active ? 'phantom-btn--danger' : 'phantom-btn--primary'}`}
          >
            {loading ? (
              <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin"></div>
            ) : status.active ? (
              <>
                <Power size={15} />
                <span>清空公网地址登记</span>
              </>
            ) : (
              <>
                <Zap size={15} />
                <span>保存公网地址登记</span>
              </>
            )}
          </button>
        </div>

        <AnimatePresence>
          {status.active && status.url && (
            <motion.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: 'auto', opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              className="bg-slate-50/50 p-4 overflow-hidden"
            >
              <div className="bg-white border border-dashed border-blue-300 rounded-2xl p-4 relative overflow-hidden">
                <div className="flex flex-col md:flex-row items-center justify-between gap-3">
                  <div className="flex-grow">
                    <span className="text-[9px] font-bold text-blue-500 tracking-widest block mb-2 font-mono">
                      公网访问地址
                    </span>
                    <div className="text-[15px] font-black text-slate-900 font-mono break-all leading-snug">{status.url}</div>
                  </div>
                  <div className="flex items-center gap-3 shrink-0">
                    <button onClick={copyToClipboard} className="phantom-btn phantom-btn--sm phantom-btn--secondary">
                      {copied ? <CheckCircle2 size={13} className="text-emerald-500" /> : <Copy size={13} />}
                      {copied ? '已复制' : '复制地址'}
                    </button>
                    <a
                      href={status.url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="phantom-btn phantom-btn--sm phantom-btn--primary"
                    >
                      <ExternalLink size={13} />
                      访问链接
                    </a>
                  </div>
                </div>

                <div className="mt-2.5 pt-2.5 border-t border-slate-100 flex items-start gap-2.5">
                  <div className="mt-0.5 text-blue-500">
                    <Zap size={13} />
                  </div>
                  <p className="text-[9px] text-slate-500 leading-relaxed font-medium">
                    请将上方公网地址填入邮件转发工作节点的{' '}
                    <code className="bg-slate-100 px-1 rounded text-rose-500 font-mono">PHANTOM_HUB_URL</code>.
                    {' '}环境变量中，以确保数据链路畅通。
                  </p>
                </div>
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </section>
    </div>
  )
}
