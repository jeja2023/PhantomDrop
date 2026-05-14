/**
 * 对代理 URL 进行脱敏处理，隐藏用户名和密码
 */
export const maskProxyUrl = (url: string | null | undefined): string => {
  if (!url) return ''
  try {
    // 处理包含协议头的完整 URL
    if (url.includes('@')) {
      return url.replace(/([^:/]+:\/\/)([^:/]+):([^@/]+)@/, '$1***:***@')
    }
    // 处理不带协议头的 user:pass@host:port
    if (url.includes(':') && url.includes('@')) {
       return url.replace(/^([^:/]+):([^@/]+)@/, '***:***@')
    }
    return url
  } catch {
    return url
  }
}

/**
 * 脱敏消息中的敏感信息（如代理 URL）
 */
export const redactMessage = (msg: string | null | undefined): string => {
  if (!msg) return ''
  // 匹配常见的代理 URL 格式并脱敏
  return msg.replace(/([a-zA-Z0-9]+:\/\/)([^:/]+):([^@/]+)@([a-zA-Z0-9.-]+)/g, '$1***:***@$4')
}
