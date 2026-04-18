import { useState, useEffect, type ReactNode } from 'react'
import {
  Save,
  Globe,
  Loader2,
  Shield,
  Activity,
  CheckCircle2,
  Copy,
  ExternalLink,
  Radar,
  Lock,
} from 'lucide-react'
import { fetchJson, postJson } from '../lib/api'
import type { CloudflareAutomationStatus, PhantomSettingsUpdatedDetail, SettingsPayload } from '../types'

type CloudflareMode = 'local_trycloudflare' | 'public_ip' | 'public_domain'

export default function SettingsView() {
  const [isSaving, setIsSaving] = useState(false)
  const [isLoading, setIsLoading] = useState(true)
  const [showToast, setShowToast] = useState(false)
  const [webhookUrl, setWebhookUrl] = useState('')
  const [accountDomain, setAccountDomain] = useState('phantom.local')
  const [updateRate, setUpdateRate] = useState(1000)
  const [authSecret, setAuthSecret] = useState('')
  const [decodeDepth, setDecodeDepth] = useState('深度扫描')
  const [showSecret, setShowSecret] = useState(false)
  const [cloudflareDefaultMode, setCloudflareDefaultMode] = useState<CloudflareMode>('public_domain')
  const [cloudflarePublicUrl, setCloudflarePublicUrl] = useState('')
  const [cloudflareRouteLocalPart, setCloudflareRouteLocalPart] = useState('inbox')
  const [cloudflareZoneDomain, setCloudflareZoneDomain] = useState('')
  const [cloudflareApiToken, setCloudflareApiToken] = useState('')
  const [cloudflareZoneId, setCloudflareZoneId] = useState('')
  const [cloudflareAccountId, setCloudflareAccountId] = useState('')
  const [cpaUrl, setCpaUrl] = useState('')
  const [cpaKey, setCpaKey] = useState('')
  const [sub2apiUrl, setSub2apiUrl] = useState('')
  const [sub2apiKey, setSub2apiKey] = useState('')
  const [showCloudflareToken, setShowCloudflareToken] = useState(false)
  const [showCpaKey, setShowCpaKey] = useState(false)
  const [automationStatus, setAutomationStatus] = useState<CloudflareAutomationStatus | null>(null)
  const [cpaAuthStatus, setCpaAuthStatus] = useState<'authenticated' | 'unauthenticated' | 'invalid'>('unauthenticated')
  const [cpaAuthEmail, setCpaAuthEmail] = useState('')
  const [cpaCodeVerifier, setCpaCodeVerifier] = useState('')
  const [cpaCallbackUrl, setCpaCallbackUrl] = useState('')
  const [isExchanging, setIsExchanging] = useState(false)

  useEffect(() => {
    const loadSettings = async () => {
      try {
        const settings = await fetchJson<SettingsPayload>('/api/settings')
        setWebhookUrl(settings.webhook_url || '')
        setAccountDomain(settings.account_domain || 'phantom.local')
        setUpdateRate(Math.max(1000, settings.update_rate || 1000))
        setAuthSecret(settings.auth_secret || '')
        setDecodeDepth(settings.decode_depth || '深度扫描')
        setCloudflareDefaultMode(settings.cloudflare_default_mode || 'public_domain')
        setCloudflarePublicUrl(settings.cloudflare_public_url || '')
        setCloudflareRouteLocalPart(settings.cloudflare_route_local_part || 'inbox')
        setCloudflareZoneDomain(settings.cloudflare_zone_domain || '')
        setCloudflareApiToken(settings.cloudflare_api_token || '')
        setCloudflareZoneId(settings.cloudflare_zone_id || '')
        setCloudflareAccountId(settings.cloudflare_account_id || '')
        setCpaUrl(settings.cpa_url || '')
        setCpaKey(settings.cpa_key || '')
        setSub2apiUrl(settings.sub2api_url || '')
        setSub2apiKey(settings.sub2api_key || '')
        const status = await fetchJson<CloudflareAutomationStatus>('/api/cloudflare/automation/status')
        setAutomationStatus(status)
        const cpaStatus = await fetchJson<{ status: any; email?: string }>('/api/cpa/auth-status')
        setCpaAuthStatus(cpaStatus.status)
        if (cpaStatus.email) setCpaAuthEmail(cpaStatus.email)
      } finally {
        setIsLoading(false)
      }
    }

    void loadSettings()

    const interval = setInterval(() => {
      void fetchJson<CloudflareAutomationStatus>('/api/cloudflare/automation/status').then(setAutomationStatus).catch(() => undefined)
    }, 2500)

    return () => clearInterval(interval)
  }, [])

  const persistSettings = async () => {
    await postJson<{ status: string }, SettingsPayload>('/api/settings/save', {
      webhook_url: webhookUrl || null,
      account_domain: accountDomain || null,
      update_rate: updateRate,
      auth_secret: authSecret || null,
      decode_depth: decodeDepth,
      cloudflare_default_mode: cloudflareDefaultMode,
      cloudflare_public_url: cloudflarePublicUrl || null,
      cloudflare_route_local_part: cloudflareRouteLocalPart || null,
      cloudflare_zone_domain: cloudflareZoneDomain || null,
      cloudflare_api_token: cloudflareApiToken || null,
      cloudflare_zone_id: cloudflareZoneId || null,
      cloudflare_account_id: cloudflareAccountId || null,
      cpa_url: cpaUrl || null,
      cpa_key: cpaKey || null,
      sub2api_url: sub2apiUrl || null,
      sub2api_key: sub2apiKey || null,
    })

    window.dispatchEvent(
      new CustomEvent<PhantomSettingsUpdatedDetail>('phantom-settings-updated', {
        detail: {
          update_rate: updateRate,
          decode_depth: decodeDepth,
          account_domain: accountDomain,
        },
      }),
    )
  }

  const handleSave = async () => {
    setIsSaving(true)
    try {
      await persistSettings()
      setShowToast(true)
      window.setTimeout(() => setShowToast(false), 3000)
    } finally {
      setIsSaving(false)
    }
  }

  const handleSaveAndInitialize = async () => {
    setIsSaving(true)
    try {
      await persistSettings()
      await postJson<{ status: string }, { mode?: string; public_url?: string }>('/api/cloudflare/automation/run', {
        mode: cloudflareDefaultMode,
        public_url: cloudflarePublicUrl || undefined,
      })
      const status = await fetchJson<CloudflareAutomationStatus>('/api/cloudflare/automation/status')
      setAutomationStatus(status)
      setShowToast(true)
      window.setTimeout(() => setShowToast(false), 3000)
    } finally {
      setIsSaving(false)
    }
  }

  const handleRetestChain = async () => {
    setIsSaving(true)
    try {
      await postJson<{ status: string }, { mode?: string; public_url?: string }>('/api/cloudflare/automation/run', {
        mode: cloudflareDefaultMode,
        public_url: cloudflarePublicUrl || undefined,
      })
      const status = await fetchJson<CloudflareAutomationStatus>('/api/cloudflare/automation/status')
      setAutomationStatus(status)
    } finally {
      setIsSaving(false)
    }
  }

  const handleCopy = async (value: string) => {
    await navigator.clipboard.writeText(value)
  }

  const actionBusy = isSaving || isLoading || automationStatus?.running

  const handleCodexLogin = async () => {
    try {
      const res = await fetchJson<{ url: string; code_verifier: string }>('/api/cpa/oauth-url')
      setCpaCodeVerifier(res.code_verifier)
      window.open(res.url, '_blank')
    } catch (e) {
      alert('获取 OAuth 链接失败')
    }
  }

  const handleExchangeCode = async () => {
    if (!cpaCallbackUrl || !cpaCodeVerifier) return
    setIsExchanging(true)
    try {
      const res = await postJson<{ status: string; data?: { id_token?: string } }, { callback_url: string; code_verifier: string }>('/api/cpa/exchange', {
        callback_url: cpaCallbackUrl,
        code_verifier: cpaCodeVerifier,
      })
      if (res.status === 'success') {
        setCpaAuthStatus('authenticated')
        if (res.data?.id_token) {
          // 简单演示：这里可以增加前端解析 JWT 展示 Email，但我们主要靠后端 status 回传
          const cpaStatus = await fetchJson<{ status: any; email?: string }>('/api/cpa/auth-status')
          if (cpaStatus.email) setCpaAuthEmail(cpaStatus.email)
        }
        setCpaCallbackUrl('')
        setCpaCodeVerifier('')
      } else {
        alert('授权失败，请检查 URL 是否正确')
      }
    } catch (e) {
      alert('令牌交换请求失败')
    } finally {
      setIsExchanging(false)
    }
  }

  return (
    <div className="page-shell relative min-w-0 space-y-2.5 animate-in fade-in slide-in-from-top-4 duration-500 pb-4 overflow-y-auto">
      <div className={`fixed top-20 right-10 z-[100] transition-all duration-500 transform ${showToast ? 'translate-y-0 opacity-100' : '-translate-y-12 opacity-0 pointer-events-none'}`}>
        <div className="rounded-2xl border border-emerald-100 bg-white px-6 py-3 shadow-2xl shadow-emerald-500/10">
          <div className="flex items-center gap-3">
            <CheckCircle2 className="text-emerald-500" size={20} />
            <div className="flex flex-col">
              <span className="text-sm font-bold text-slate-800">配置同步成功</span>
              <span className="text-[10px] text-slate-500 font-mono">设置已写入后端。</span>
            </div>
          </div>
        </div>
      </div>

      <div className="flex items-center justify-between gap-4 border-b border-slate-100/60 pb-2">
        <div className="flex items-baseline gap-3">
          <h1 className="text-xl font-black tracking-tight text-slate-900 group-hover:text-blue-600 transition-colors"></h1>
          <span className="text-[9px] font-black tracking-widest text-slate-400 font-mono opacity-60"></span>
        </div>
        <div className="flex items-center gap-2">
          {automationStatus?.running ? <div className="h-1.5 w-1.5 rounded-full bg-blue-500 animate-pulse mr-1"></div> : null}
          <div className="flex items-center gap-1.5 border-r border-slate-200 pr-3 mr-1">
            <button
              onClick={handleSaveAndInitialize}
              disabled={actionBusy}
              className={`phantom-btn h-8 px-3 transition-all duration-300 ${
                actionBusy ? 'phantom-btn--muted' : 'phantom-btn--secondary border-blue-100 hover:bg-blue-50/50'
              }`}
            >
              {actionBusy ? <Loader2 size={12} className="animate-spin text-blue-500" /> : <Globe size={12} className="text-blue-500" />}
              <span className="text-[10px] font-bold">部署</span>
            </button>
            <button
              onClick={handleSave}
              disabled={isSaving || isLoading}
              className={`phantom-btn h-8 px-4 shadow-lg transition-all duration-300 shadow-blue-500/10 ${
                isSaving || isLoading ? 'phantom-btn--muted' : 'phantom-btn--primary active:scale-95'
              }`}
            >
              {isSaving || isLoading ? <Loader2 size={12} className="animate-spin" /> : <Save size={12} />}
              <span className="text-[10px] font-bold">保存</span>
            </button>
          </div>
          <div
            className={`rounded-md border px-2 py-0.5 text-[8px] font-black tracking-widest ${
              automationStatus?.running
                ? 'border-blue-100 bg-blue-50 text-blue-600 animate-pulse'
                : automationStatus?.last_success
                  ? 'border-emerald-100 bg-emerald-50 text-emerald-600'
                  : 'border-slate-200 bg-slate-50 text-slate-400'
            }`}
          >
            {automationStatus?.running ? '运行中' : automationStatus?.last_success ? '已就绪' : '离线'}
          </div>
        </div>
      </div>

      <div className="grid min-h-0 gap-4 xl:grid-cols-[minmax(0,0.92fr)_minmax(0,1.08fr)]">
        <div className="min-w-0 space-y-4">
          <SettingsSectionCard icon={<Globe size={14} />} title="网络连接">
            <SettingsRow
              title="推送地址"
              hint="边缘节点回传入口"
              control={
                <input
                  aria-label="边缘节点推送地址"
                  title="边缘节点推送地址"
                  value={webhookUrl}
                  onChange={(e) => setWebhookUrl(e.target.value)}
                  placeholder="http://127.0.0.1:5000/webhook"
                  disabled={isLoading}
                  className="phantom-input w-full"
                />
              }
            />
            <SettingsRow
              title="默认账户域名"
              hint="账户产物输出域名"
              control={
                <input
                  aria-label="默认账户域名"
                  title="默认账户域名"
                  value={accountDomain}
                  onChange={(e) => setAccountDomain(e.target.value)}
                  placeholder="phantom.local"
                  disabled={isLoading}
                  className="phantom-input w-full"
                />
              }
            />
            <SettingsRow
              title="实时轮询频率"
              hint="前端状态刷新间隔"
              control={
                <div className="flex items-center gap-4 rounded-xl border border-slate-100 bg-slate-50 px-3 py-1.5 focus-within:border-blue-200 transition-all">
                  <input
                    aria-label="实时轮询频率"
                    title="实时轮询频率"
                    type="range"
                    min="1000"
                    max="5000"
                    value={updateRate}
                    onChange={(e) => setUpdateRate(Math.max(1000, parseInt(e.target.value, 10) || 1000))}
                    disabled={isLoading}
                    className="flex-grow appearance-none h-1 bg-slate-200 rounded-full cursor-pointer accent-blue-600"
                  />
                  <span className="text-[10px] font-black font-mono text-blue-600 w-12 text-right">{updateRate}ms</span>
                </div>
              }
            />
          </SettingsSectionCard>

          <SettingsSectionCard icon={<Shield size={14} />} title="安全协议">
            <SettingsRow
              title="节点认证密钥"
              hint="边缘节点请求鉴权"
              control={
                <div className="relative">
                  <input
                    aria-label="节点认证密钥"
                    title="节点认证密钥"
                    type={showSecret ? 'text' : 'password'}
                    value={authSecret}
                    onChange={(e) => setAuthSecret(e.target.value)}
                    disabled={isLoading}
                    placeholder="请输入认证密钥"
                    className="phantom-input w-full pr-10"
                  />
                  <button
                    type="button"
                    onClick={() => setShowSecret(!showSecret)}
                    className="absolute right-0 top-0 bottom-0 px-3 flex items-center justify-center text-slate-400 hover:text-blue-500 transition-colors"
                  >
                    <Lock size={14} className={showSecret ? 'text-blue-500' : ''} />
                  </button>
                </div>
              }
            />
            <SettingsRow
              title="自动解析等级"
              hint="邮件正文解析深度"
              control={
                <div className="relative">
                  <select
                    aria-label="自动解析等级"
                    title="自动解析等级"
                    value={decodeDepth}
                    onChange={(e) => setDecodeDepth(e.target.value)}
                    disabled={isLoading}
                    className="phantom-select w-full pr-10"
                  >
                    <option>深度扫描</option>
                    <option>仅解析头部</option>
                    <option>仅解析纯文本</option>
                  </select>
                </div>
              }
            />
          </SettingsSectionCard>


          <SettingsSectionCard icon={<Activity size={14} />} title="链路结果">
            <div className="space-y-2">
              <div className="grid gap-1.5 sm:grid-cols-2">
                <ResultCard title="当前步骤" value={automationStatus?.current_step} emptyLabel="等待中" />
                <ResultCard title="最后一次成功" value={automationStatus?.last_success ? '已就绪' : null} emptyLabel="暂无成功记录" />
              </div>

              <div className="grid gap-1.5 sm:grid-cols-2">
                <ResultCard
                  title="工作节点地址"
                  value={automationStatus?.worker_url}
                  emptyLabel="尚未生成"
                  actions={
                    automationStatus?.worker_url ? (
                      <div className="flex gap-1.5">
                        <button onClick={() => void handleCopy(automationStatus.worker_url!)} className="text-slate-400 hover:text-blue-500">
                          <Copy size={12} />
                        </button>
                        <a href={automationStatus.worker_url} target="_blank" rel="noopener noreferrer" className="text-slate-400 hover:text-blue-500">
                          <ExternalLink size={12} />
                        </a>
                      </div>
                    ) : null
                  }
                />
                <ResultCard
                  title="最终收件地址"
                  value={automationStatus?.email_address}
                  emptyLabel="尚未生成"
                  actions={
                    automationStatus?.email_address ? (
                      <button onClick={() => void handleCopy(automationStatus.email_address!)} className="text-slate-400 hover:text-blue-500">
                        <Copy size={12} />
                      </button>
                    ) : null
                  }
                />
              </div>

              <div className="rounded-lg border border-slate-100 bg-white p-2 text-[10px]">
                <div className="mb-1.5 flex items-center justify-between px-1">
                  <span className="text-[8px] font-black tracking-widest text-slate-400">链路日志</span>
                  <button onClick={handleRetestChain} disabled={actionBusy} className="text-[9px] font-black text-blue-500 flex items-center gap-1 hover:text-blue-600">
                    {automationStatus?.running ? <Loader2 size={10} className="animate-spin" /> : <Radar size={10} />}
                    重测
                  </button>
                </div>
                <div className="space-y-0.5 max-h-32 overflow-y-auto pr-1 scrollbar-thin">
                  {(automationStatus?.logs && automationStatus.logs.length > 0 ? automationStatus.logs : [{ level: 'info', message: '等待系统指令...' }]).map((entry, index) => (
                    <div
                      key={index}
                      className={`rounded px-1.5 py-0.5 flex gap-1.5 items-start ${
                        entry.level === 'success'
                          ? 'bg-emerald-50 text-emerald-700'
                          : entry.level === 'error'
                            ? 'bg-rose-50 text-rose-700'
                            : entry.level === 'warn'
                              ? 'bg-amber-50 text-amber-700'
                              : entry.level === 'step'
                                ? 'bg-blue-50 text-blue-700'
                                : 'text-slate-500'
                      }`}
                    >
                      <span className="opacity-30 font-mono shrink-0">[{index + 1}]</span>
                      <span className="truncate">{entry.message}</span>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </SettingsSectionCard>
        </div>

        <div className="min-w-0 space-y-4">
          <SettingsSectionCard icon={<Radar size={14} />} title="邮件转发自动化">
            <div className="grid gap-2 md:grid-cols-2">
              <SettingsTile
                title="接入模式"
                hint="默认公网接入策略"
                control={
                  <select
                    aria-label="默认接入模式"
                    title="默认接入模式"
                    value={cloudflareDefaultMode}
                    onChange={(e) => setCloudflareDefaultMode(e.target.value as CloudflareMode)}
                    disabled={isLoading}
                    className="phantom-select w-full"
                  >
                    <option value="public_domain">公网域名</option>
                    <option value="public_ip">公网 IP</option>
                    <option value="local_trycloudflare">本地临时隧道</option>
                  </select>
                }
              />
              <SettingsTile
                title="公网地址"
                hint="默认公网入口地址"
                control={
                  <input
                    aria-label="默认公网地址"
                    title="默认公网地址"
                    value={cloudflarePublicUrl}
                    onChange={(e) => setCloudflarePublicUrl(e.target.value)}
                    placeholder="https://hub.example.com"
                    disabled={isLoading}
                    className="phantom-input w-full"
                  />
                }
              />
              <SettingsTile
                title="收件地址前缀"
                hint="本地邮箱名前缀"
                control={
                  <input
                    aria-label="收件地址前缀"
                    title="收件地址前缀"
                    value={cloudflareRouteLocalPart}
                    onChange={(e) => setCloudflareRouteLocalPart(e.target.value)}
                    placeholder="inbox"
                    disabled={isLoading}
                    className="phantom-input w-full"
                  />
                }
              />
              <SettingsTile
                title="主域名"
                hint="托管区域域名"
                control={
                  <input
                    aria-label="主域名"
                    title="主域名"
                    value={cloudflareZoneDomain}
                    onChange={(e) => setCloudflareZoneDomain(e.target.value)}
                    placeholder="example.com"
                    disabled={isLoading}
                    className="phantom-input w-full"
                  />
                }
              />
              <div className="md:col-span-2">
                <SettingsTile
                  title="接口令牌"
                  hint="Cloudflare 接口令牌"
                  control={
                    <div className="relative">
                      <input
                        aria-label="接口令牌"
                        title="接口令牌"
                        type={showCloudflareToken ? 'text' : 'password'}
                        value={cloudflareApiToken}
                        onChange={(e) => setCloudflareApiToken(e.target.value)}
                        disabled={isLoading}
                        placeholder="请输入接口令牌"
                        className="phantom-input w-full pr-10"
                      />
                      <button
                        type="button"
                        onClick={() => setShowCloudflareToken(!showCloudflareToken)}
                        className="absolute right-0 top-0 bottom-0 px-3 flex items-center justify-center text-slate-400 hover:text-blue-500 transition-colors"
                      >
                        <Lock size={14} className={showCloudflareToken ? 'text-blue-500' : ''} />
                      </button>
                    </div>
                  }
                />
              </div>
              <SettingsTile
                title="区域编号"
                hint="Zone 标识"
                control={<input value={cloudflareZoneId} onChange={(e) => setCloudflareZoneId(e.target.value)} disabled={isLoading} className="phantom-input w-full" placeholder="请输入区域编号" />}
              />
              <SettingsTile
                title="账户编号"
                hint="Account 标识"
                control={<input value={cloudflareAccountId} onChange={(e) => setCloudflareAccountId(e.target.value)} disabled={isLoading} className="phantom-input w-full" placeholder="请输入账户编号" />}
              />
            </div>
          </SettingsSectionCard>
          
          <SettingsSectionCard icon={<ExternalLink size={14} />} title="账号分发 (CPA)">
            <SettingsRow
              title="CPA 接口地址"
              hint="推送产物的 API 端点"
              control={
                <input
                  aria-label="CPA 接口地址"
                  title="CPA 接口地址"
                  value={cpaUrl}
                  onChange={(e) => setCpaUrl(e.target.value)}
                  placeholder="https://cpa.chat/api/openai/import"
                  disabled={isLoading}
                  className="phantom-input w-full"
                />
              }
            />
            <SettingsRow
              title="CPA 管理密码"
              hint="平台管理密钥 (MANAGEMENT_PASSWORD)"
              control={
                <div className="relative">
                  <input
                    aria-label="CPA 管理密码"
                    title="CPA 管理密码"
                    type={showCpaKey ? 'text' : 'password'}
                    value={cpaKey}
                    onChange={(e) => setCpaKey(e.target.value)}
                    disabled={isLoading}
                    placeholder="请输入管理密码或密钥"
                    className="phantom-input w-full pr-10"
                  />
                  <button
                    type="button"
                    onClick={() => setShowCpaKey(!showCpaKey)}
                    className="absolute right-0 top-0 bottom-0 px-3 flex items-center justify-center text-slate-400 hover:text-blue-500 transition-colors"
                  >
                    <Lock size={14} className={showCpaKey ? 'text-blue-500' : ''} />
                  </button>
                </div>
              }
            />
            <SettingsRow
              title="CPA 认证状态"
              hint="Codex 服务登录凭据"
              control={
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-2">
                    <div className={`h-2 w-2 rounded-full ${cpaAuthStatus === 'authenticated' ? 'bg-emerald-500 animate-pulse' : 'bg-slate-300'}`}></div>
                    <span className="text-[10px] font-bold text-slate-600">
                      {cpaAuthStatus === 'authenticated' ? `已授权 (${cpaAuthEmail || 'Codex Service'})` : '未授权'}
                    </span>
                  </div>
                  <div className="flex gap-1.5">
                    {cpaAuthStatus === 'authenticated' && (
                      <button
                        onClick={async () => {
                          if (confirm('确定要清除 CPA 认证状态吗？')) {
                            await postJson('/api/settings/save', { cpa_auth_json: '' })
                            setCpaAuthStatus('unauthenticated')
                            setCpaAuthEmail('')
                          }
                        }}
                        className="phantom-btn h-7 px-2.5 phantom-btn--secondary border-red-50 hover:bg-red-50 text-red-400 text-[9px] font-black"
                        title="清除授权"
                      >
                        <Lock size={10} />
                      </button>
                    )}
                    <button
                      onClick={handleCodexLogin}
                      disabled={isExchanging}
                      className="phantom-btn h-7 px-3 phantom-btn--secondary border-blue-100 hover:bg-blue-50 text-[9px] font-black"
                    >
                      <ExternalLink size={10} className="mr-1" />
                      {cpaAuthStatus === 'authenticated' ? '重连授权' : '去登录授权'}
                    </button>
                  </div>
                </div>
              }
            />
            {cpaCodeVerifier && (
              <div className="mt-2 space-y-2 rounded-xl border border-blue-50 bg-blue-50/30 p-2.5 animate-in slide-in-from-top-2">
                <div className="flex items-center justify-between px-1">
                  <span className="text-[9px] font-black text-blue-600">回调 URL 提交</span>
                  <span className="text-[8px] text-blue-400 font-mono">等待授权回调...</span>
                </div>
                <div className="flex gap-2">
                  <input
                    aria-label="回调 URL"
                    title="回调 URL"
                    value={cpaCallbackUrl}
                    onChange={(e) => setCpaCallbackUrl(e.target.value)}
                    placeholder="在此粘贴包含 code=... 的完整回调 URL"
                    className="phantom-input flex-grow text-[10px] h-8"
                  />
                  <button
                    onClick={handleExchangeCode}
                    disabled={isExchanging || !cpaCallbackUrl}
                    className="phantom-btn h-8 px-4 phantom-btn--primary shadow-md shadow-blue-500/10 active:scale-95 transition-all"
                  >
                    {isExchanging ? <Loader2 size={12} className="animate-spin" /> : <Save size={12} />}
                    <span className="text-[10px] font-bold ml-1">确认提交</span>
                  </button>
                </div>
                <p className="px-1 text-[8px] text-slate-400">
                  授权完成后，浏览器会跳转至 localhost:1455，请复制完整的跳转后 URL 地址提交。
                </p>
              </div>
            )}
          </SettingsSectionCard>

        </div>
      </div>
    </div>
  )
}

function SettingsSectionCard({ icon, title, children }: { icon: ReactNode; title: string; children: ReactNode }) {
  return (
    <section className="animate-in fade-in slide-in-from-bottom-2 duration-500">
      <div className="page-panel group/card overflow-hidden rounded-[12px] border border-slate-200/60 bg-white transition-all duration-300 hover:shadow-md">
        <div className="border-b border-slate-100 bg-slate-50/40 px-3 py-1.5">
          <div className="flex items-center gap-2">
            <div className="text-slate-400">{icon}</div>
            <div className="text-[11px] font-black tracking-tight text-slate-800">{title}</div>
          </div>
        </div>
        <div className="px-3 py-1.5">{children}</div>
      </div>
    </section>
  )
}

function SettingsRow({ title, hint, control }: { title: string; hint: string; control: ReactNode }) {
  return (
    <div className="grid items-center gap-3 border-b border-slate-100/60 py-2 last:border-b-0 md:grid-cols-[1fr_260px]">
      <div className="space-y-0.5 group/row">
        <div className="flex items-center gap-1.5">
          <div className="h-1 w-1 rounded-full bg-slate-200 group-hover/row:bg-blue-400"></div>
          <div className="text-[11px] font-bold text-slate-600 transition-colors group-hover/row:text-slate-900">{title}</div>
        </div>
        <div className="pl-3 text-[8px] font-black tracking-widest text-slate-300 font-mono italic opacity-60">{hint}</div>
      </div>
      <div className="relative">{control}</div>
    </div>
  )
}

function SettingsTile({ title, hint, control }: { title: string; hint: string; control: ReactNode }) {
  return (
    <div className="group/tile rounded-[12px] border border-slate-200/60 bg-slate-50/50 p-2.5 transition-all duration-300 hover:bg-white hover:border-blue-100">
      <div className="mb-1.5 flex flex-col">
        <div className="text-[11px] font-black tracking-tight text-slate-800">{title}</div>
        <div className="text-[8px] font-black tracking-widest text-slate-400 font-mono opacity-40">{hint}</div>
      </div>
      <div className="relative">{control}</div>
    </div>
  )
}

function ResultCard({ title, value, emptyLabel, actions }: { title: string; value?: string | null; emptyLabel: string; actions?: ReactNode }) {
  return (
    <div className="group/result rounded-lg border border-slate-100 bg-slate-50/30 p-2 transition-all">
      <div className="flex items-center justify-between mb-0.5">
        <div className="text-[8px] font-black tracking-tight text-slate-400">{title}</div>
        <div className={`h-1 w-1 rounded-full ${value ? 'bg-emerald-400' : 'bg-slate-200'}`}></div>
      </div>
      <div className="flex min-h-[16px] items-center justify-between gap-2 text-[10px]">
        {value ? <div className="truncate font-mono font-bold text-slate-700">{value}</div> : <div className="text-slate-300 italic text-[9px]">{emptyLabel}</div>}
        {actions ? <div className="shrink-0">{actions}</div> : null}
      </div>
    </div>
  )
}
