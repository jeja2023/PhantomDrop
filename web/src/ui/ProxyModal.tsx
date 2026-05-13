import { useState, useEffect } from 'react'
import { X, Globe, Lock, User, Check, Eye, EyeOff, Loader2, Wifi, AlertCircle } from 'lucide-react'
import { motion, AnimatePresence } from 'framer-motion'
import { createPortal } from 'react-dom'
import { postJson } from '../lib/api'

interface ProxyModalProps {
  isOpen: boolean
  onClose: () => void
  value: string
  onChange: (value: string) => void
}

/**
 * 解析代理服务器 URL 字符串
 */
function parseProxyUrl(urlStr: string) {
  let protocol = 'http'
  let username = ''
  let password = ''
  let host = ''
  let port = ''

  const working = urlStr.trim()
  if (!working) {
    return { protocol, username, password, host, port }
  }

  try {
    let parseStr = working
    if (!working.includes('://')) {
      parseStr = 'http://' + working
    }
    const url = new URL(parseStr)
    protocol = url.protocol.replace(':', '') || 'http'
    username = decodeURIComponent(url.username) || ''
    password = decodeURIComponent(url.password) || ''
    host = url.hostname || ''
    port = url.port || ''
  } catch {
    // 降级正则解析
    let rest = working
    const protoMatch = rest.match(/^([^:]+):\/\//)
    if (protoMatch) {
      protocol = protoMatch[1]
      rest = rest.substring(protoMatch[0].length)
    }
    const authMatch = rest.match(/^([^:]+):([^@]+)@/)
    if (authMatch) {
      username = authMatch[1]
      password = authMatch[2]
      rest = rest.substring(authMatch[0].length)
    } else {
      const singleAuthMatch = rest.match(/^([^@]+)@/)
      if (singleAuthMatch) {
        username = singleAuthMatch[1]
        rest = rest.substring(singleAuthMatch[0].length)
      }
    }
    const portMatch = rest.match(/:(\d+)$/)
    if (portMatch) {
      port = portMatch[1]
      host = rest.substring(0, rest.length - portMatch[0].length)
    } else {
      host = rest
    }
  }

  return { protocol, username, password, host, port }
}

/**
 * 格式化代理服务器 URL 字符串
 */
function formatProxyUrl(
  protocol: string,
  host: string,
  port: string,
  username?: string,
  password?: string
) {
  if (!host) return ''
  let url = ''
  if (protocol) {
    url += `${protocol}://`
  } else {
    url += 'http://'
  }
  if (username) {
    url += username
    if (password) {
      url += `:${password}`
    }
    url += '@'
  }
  url += host
  if (port) {
    url += `:${port}`
  }
  return url
}

/**
 * 代理服务器配置弹窗组件
 * 支持拆分配置主机、端口、用户名、密码
 */
export default function ProxyModal({ isOpen, onClose, value, onChange }: ProxyModalProps) {
  const [protocol, setProtocol] = useState('http')
  const [host, setHost] = useState('')
  const [port, setPort] = useState('')
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [showPassword, setShowPassword] = useState(false)

  const [isTesting, setIsTesting] = useState(false)
  const [testResult, setTestResult] = useState<{ status: 'success' | 'error'; message: string; latency_ms?: number } | null>(null)

  // 当弹窗打开时，解析当前传入的值并填充表单
  useEffect(() => {
    if (isOpen) {
      const parsed = parseProxyUrl(value)
      setProtocol(parsed.protocol)
      setHost(parsed.host)
      setPort(parsed.port)
      setUsername(parsed.username)
      setPassword(parsed.password)
      setShowPassword(false)
    }
  }, [isOpen, value])

  const previewUrl = formatProxyUrl(protocol, host, port, username, password)

  // 当配置参数改变时，清空上一次的测试结果
  useEffect(() => {
    setTestResult(null)
  }, [previewUrl])

  if (!isOpen) return null

  const handleTest = async () => {
    setIsTesting(true)
    setTestResult(null)
    try {
      const res = await postJson<{
        status: 'success' | 'error'
        message: string
        latency_ms?: number
      }, { proxy_url: string }>('/api/proxy/test', { proxy_url: previewUrl })
      setTestResult(res)
    } catch (e) {
      setTestResult({
        status: 'error',
        message: e instanceof Error ? e.message : '连接超时或未知网络错误'
      })
    } finally {
      setIsTesting(false)
    }
  }

  const handleSave = () => {
    onChange(previewUrl)
    onClose()
  }

  return createPortal(
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        className="fixed inset-0 z-[10000] flex items-center justify-center bg-slate-950/60 p-4 backdrop-blur-sm"
        onClick={onClose}
      >
        <motion.div
          initial={{ scale: 0.95, opacity: 0, y: 15 }}
          animate={{ scale: 1, opacity: 1, y: 0 }}
          exit={{ scale: 0.95, opacity: 0, y: 15 }}
          className="relative max-w-md w-full bg-white rounded-3xl overflow-hidden shadow-2xl border border-slate-200"
          onClick={(e) => e.stopPropagation()}
        >
          {/* 头部 (Header) */}
          <div className="px-6 py-5 border-b border-slate-100 flex items-center justify-between bg-slate-50/50 backdrop-blur-md">
            <div className="flex items-center gap-3">
              <div className="w-9 h-9 rounded-xl bg-blue-600/10 flex items-center justify-center text-blue-600">
                <Globe size={18} />
              </div>
              <div>
                <h3 className="text-sm font-bold text-slate-950">代理服务器鉴权配置</h3>
                <p className="text-[10px] text-slate-500 font-mono uppercase tracking-wider">Proxy Authentication Config</p>
              </div>
            </div>
            <button
              onClick={onClose}
              className="p-1.5 rounded-lg hover:bg-slate-100 text-slate-400 hover:text-slate-700 transition-all"
            >
              <X size={18} />
            </button>
          </div>

          {/* 表单区域 (Form) */}
          <div className="p-6 space-y-4">
            {/* 协议与主机 */}
            <div className="grid grid-cols-3 gap-3">
              <div className="space-y-1.5 col-span-1">
                <label className="text-[11px] font-bold text-slate-700">代理协议</label>
                <select
                  value={protocol}
                  onChange={(e) => setProtocol(e.target.value)}
                  className="w-full bg-slate-50 border border-slate-200 rounded-xl px-3 py-2 text-sm outline-none focus:border-blue-500 transition-colors"
                >
                  <option value="http">HTTP</option>
                  <option value="https">HTTPS</option>
                  <option value="socks5">SOCKS5</option>
                  <option value="socks5h">SOCKS5H</option>
                </select>
              </div>
              <div className="space-y-1.5 col-span-2">
                <label className="text-[11px] font-bold text-slate-700">主机名 / IP</label>
                <input
                  type="text"
                  placeholder="如 127.0.0.1"
                  value={host}
                  onChange={(e) => setHost(e.target.value)}
                  className="w-full bg-slate-50 border border-slate-200 rounded-xl px-3 py-2 text-sm outline-none focus:border-blue-500 transition-colors"
                />
              </div>
            </div>

            {/* 端口号 */}
            <div className="space-y-1.5">
              <label className="text-[11px] font-bold text-slate-700">端口号</label>
              <input
                type="text"
                placeholder="如 10809"
                value={port}
                onChange={(e) => setPort(e.target.value.replace(/\D/g, ''))}
                className="w-full bg-slate-50 border border-slate-200 rounded-xl px-3 py-2 text-sm outline-none focus:border-blue-500 transition-colors"
              />
            </div>

            {/* 分割线 */}
            <div className="relative flex py-1 items-center">
              <div className="flex-grow border-t border-slate-100"></div>
              <span className="flex-shrink mx-4 text-[10px] font-bold text-slate-400 tracking-wider">鉴权凭据 (可选)</span>
              <div className="flex-grow border-t border-slate-100"></div>
            </div>

            {/* 用户名 */}
            <div className="space-y-1.5">
              <label className="text-[11px] font-bold text-slate-700">代理用户名</label>
              <div className="relative">
                <div className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400">
                  <User size={14} />
                </div>
                <input
                  type="text"
                  placeholder="选填"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  className="w-full bg-slate-50 border border-slate-200 rounded-xl pl-9 pr-3 py-2 text-sm outline-none focus:border-blue-500 transition-colors"
                />
              </div>
            </div>

            {/* 密码 */}
            <div className="space-y-1.5">
              <label className="text-[11px] font-bold text-slate-700">代理密码</label>
              <div className="relative">
                <div className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400">
                  <Lock size={14} />
                </div>
                <input
                  type={showPassword ? 'text' : 'password'}
                  placeholder="选填"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  className="w-full bg-slate-50 border border-slate-200 rounded-xl pl-9 pr-10 py-2 text-sm outline-none focus:border-blue-500 transition-colors"
                />
                <button
                  type="button"
                  onClick={() => setShowPassword(!showPassword)}
                  className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-600"
                >
                  {showPassword ? <EyeOff size={14} /> : <Eye size={14} />}
                </button>
              </div>
            </div>

            {/* 预览生成的 Proxy URL */}
            <div className="mt-4 p-3.5 bg-slate-50 rounded-2xl border border-slate-100 space-y-2">
              <div className="flex items-center justify-between">
                <span className="text-[10px] font-bold text-slate-400 uppercase tracking-wider block">生成的完整代理 URL</span>
                <button
                  type="button"
                  onClick={handleTest}
                  disabled={isTesting || !host}
                  className={`text-[10px] font-bold flex items-center gap-1.5 transition-all px-2.5 py-1 rounded-lg ${
                    host
                      ? 'text-blue-600 hover:bg-blue-50 cursor-pointer'
                      : 'text-slate-300 cursor-not-allowed'
                  }`}
                >
                  {isTesting ? (
                    <Loader2 size={10} className="animate-spin" />
                  ) : (
                    <Wifi size={10} />
                  )}
                  {isTesting ? '正在测试...' : '测试联通性'}
                </button>
              </div>
              <code className="text-xs font-mono text-slate-600 break-all select-all block">
                {previewUrl || '等待配置...'}
              </code>

              {/* 测试结果反馈 */}
              {testResult && (
                <div
                  className={`flex items-start gap-2 p-2.5 rounded-xl border text-[11px] animate-in fade-in slide-in-from-top-1 duration-200 ${
                    testResult.status === 'success'
                      ? 'bg-emerald-50 border-emerald-100 text-emerald-700'
                      : 'bg-rose-50 border-rose-100 text-rose-700'
                  }`}
                >
                  {testResult.status === 'success' ? (
                    <Check size={14} className="shrink-0 mt-0.5" />
                  ) : (
                    <AlertCircle size={14} className="shrink-0 mt-0.5" />
                  )}
                  <div className="flex-grow leading-normal">
                    <span className="font-bold">
                      {testResult.status === 'success' ? '测试联通成功' : '测试联通失败'}
                    </span>
                    {testResult.latency_ms !== undefined && (
                      <span className="font-mono bg-emerald-500/15 px-1 py-0.5 rounded ml-1.5 text-[9px] font-bold">
                        {testResult.latency_ms}ms
                      </span>
                    )}
                    <p className="mt-0.5 text-[10px] font-mono break-all opacity-85">
                      {testResult.message}
                    </p>
                  </div>
                </div>
              )}
            </div>
          </div>

          {/* 底部按钮 (Footer) */}
          <div className="px-6 py-4 bg-slate-50 border-t border-slate-100 flex items-center justify-end gap-3">
            <button
              onClick={onClose}
              className="px-4 py-2 rounded-xl text-xs font-bold bg-white border border-slate-200 text-slate-600 hover:bg-slate-50 hover:text-slate-900 transition-all shadow-sm"
            >
              取消
            </button>
            <button
              onClick={handleSave}
              disabled={!host}
              className={`px-4 py-2 rounded-xl text-xs font-bold flex items-center gap-1.5 text-white transition-all shadow-md ${
                host
                  ? 'bg-blue-600 hover:bg-blue-700 shadow-blue-600/10'
                  : 'bg-slate-300 cursor-not-allowed shadow-none'
              }`}
            >
              <Check size={14} />
              应用配置
            </button>
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>,
    document.body
  )
}
