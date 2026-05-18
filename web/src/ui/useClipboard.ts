import { useCallback } from 'react'
import { useToast } from './Toast'

export function useClipboard() {
  const showToast = useToast()

  return useCallback(
    async (
      value: string | null | undefined,
      options: { title?: string; desc?: string; silent?: boolean } = {},
    ) => {
      if (!value) return false

      try {
        await navigator.clipboard.writeText(value)
        if (!options.silent) {
          showToast({
            title: options.title ?? '复制成功',
            desc: options.desc ?? (value.length > 24 ? '数据已复制到剪贴板' : `已复制: ${value}`),
            tone: 'success',
          })
        }
        return true
      } catch {
        if (!options.silent) {
          showToast({
            title: '复制失败',
            desc: '浏览器拒绝了剪贴板写入',
            tone: 'error',
          })
        }
        return false
      }
    },
    [showToast],
  )
}
