/**
 * 对代理 URL 进行脱敏处理，隐藏用户名和密码
 */
export const maskProxyUrl = (url: string | null | undefined): string => {
  if (!url) return ''
  return '[代理地址已隐藏]'
}

/**
 * 脱敏消息中的敏感信息（如代理 URL）
 */
export const redactMessage = (msg: string | null | undefined): string => {
  if (!msg) return ''
  return msg
    .replace(/\b(?:\d{1,3}\.){3}\d{1,3}\b/g, '[IP已隐藏]')
    .replace(/IP\s*[:：]\s*[^|，,]+/gi, 'IP: [已隐藏]')
    .replace(/(归属地|所在地|country|city)\s*[:：]\s*[^|，,]+/gi, '$1: [已隐藏]')
    .replace(/(组织|运营商|org|asn)\s*[:：]\s*[^|，,]+/gi, '$1: [已隐藏]')
    .replace(/\b(?:https?|socks4a?|socks5h?):\/\/[^\s，,|]+/gi, (value) => (
      value.includes('@') || value.toLowerCase().includes('proxy') ? '[代理地址已隐藏]' : value
    ))
}
