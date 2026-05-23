import { createContext, useCallback, useContext, useMemo, useRef, useState, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { AnimatePresence, motion } from 'framer-motion'
import { AlertCircle, CheckCircle2, Info, XCircle } from 'lucide-react'

type ToastTone = 'success' | 'info' | 'warn' | 'error'

type ToastInput = {
  title: string
  desc?: string
  tone?: ToastTone
  durationMs?: number
}

type ToastState = Required<Pick<ToastInput, 'title' | 'tone'>> & {
  desc?: string
}

type ToastContextValue = {
  showToast: (toast: ToastInput | string) => void
}

const ToastContext = createContext<ToastContextValue | null>(null)

const toneMeta: Record<ToastTone, { border: string; shadow: string; icon: ReactNode }> = {
  success: {
    border: 'border-emerald-100',
    shadow: 'shadow-emerald-500/10',
    icon: <CheckCircle2 className="text-emerald-500" size={20} />,
  },
  info: {
    border: 'border-blue-100',
    shadow: 'shadow-blue-500/10',
    icon: <Info className="text-blue-500" size={20} />,
  },
  warn: {
    border: 'border-amber-100',
    shadow: 'shadow-amber-500/10',
    icon: <AlertCircle className="text-amber-500" size={20} />,
  },
  error: {
    border: 'border-rose-100',
    shadow: 'shadow-rose-500/10',
    icon: <XCircle className="text-rose-500" size={20} />,
  },
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toast, setToast] = useState<ToastState | null>(null)
  const timerRef = useRef<number | null>(null)

  const showToast = useCallback((input: ToastInput | string) => {
    const nextToast = typeof input === 'string' ? { title: input } : input
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current)
    }

    setToast({
      title: nextToast.title,
      desc: nextToast.desc,
      tone: nextToast.tone ?? 'success',
    })

    timerRef.current = window.setTimeout(() => {
      setToast(null)
      timerRef.current = null
    }, nextToast.durationMs ?? 2500)
  }, [])

  const value = useMemo(() => ({ showToast }), [showToast])
  const meta = toast ? toneMeta[toast.tone] : null

  return (
    <ToastContext.Provider value={value}>
      {children}
      {createPortal(
        <AnimatePresence>
          {toast && meta ? (
            <motion.div
              initial={{ opacity: 0, y: -24, scale: 0.98 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: -18, scale: 0.98, transition: { duration: 0.16 } }}
              className="fixed right-6 top-20 z-[10050] max-w-[calc(100vw-3rem)]"
            >
              <div className={`flex items-center gap-3 rounded-2xl border ${meta.border} bg-white px-5 py-3 shadow-2xl ${meta.shadow}`}>
                {meta.icon}
                <div className="flex min-w-0 flex-col">
                  <span className="truncate text-sm font-bold text-slate-800">{toast.title}</span>
                  {toast.desc ? <span className="truncate text-[10px] font-mono text-slate-500">{toast.desc}</span> : null}
                </div>
              </div>
            </motion.div>
          ) : null}
        </AnimatePresence>,
        document.body,
      )}
    </ToastContext.Provider>
  )
}

// eslint-disable-next-line react-refresh/only-export-components
export function useToast() {
  const context = useContext(ToastContext)
  if (!context) {
    throw new Error('useToast must be used inside ToastProvider')
  }
  return context.showToast
}
