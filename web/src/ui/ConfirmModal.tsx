import { AlertTriangle, X } from 'lucide-react'
import { motion, AnimatePresence } from 'framer-motion'
import { createPortal } from 'react-dom'

interface ConfirmModalProps {
  isOpen: boolean
  title: string
  message: string
  confirmText?: string
  cancelText?: string
  tone?: 'danger' | 'info' | 'warn'
  onConfirm: () => void
  onCancel: () => void
}

export default function ConfirmModal({
  isOpen,
  title,
  message,
  confirmText = '确定',
  cancelText = '取消',
  tone = 'info',
  onConfirm,
  onCancel,
}: ConfirmModalProps) {
  // 根据不同的提示类型设置不同的图标与按钮配色
  const theme = {
    danger: {
      iconBg: 'bg-rose-50 text-rose-600 border border-rose-100',
      btnClass: 'bg-gradient-to-r from-rose-500 to-red-600 hover:from-rose-600 hover:to-red-700 text-white shadow-rose-500/10 focus:ring-rose-200',
    },
    warn: {
      iconBg: 'bg-amber-50 text-amber-600 border border-amber-100',
      btnClass: 'bg-gradient-to-r from-amber-500 to-orange-600 hover:from-amber-600 hover:to-orange-700 text-white shadow-amber-500/10 focus:ring-amber-200',
    },
    info: {
      iconBg: 'bg-blue-50 text-blue-600 border border-blue-100',
      btnClass: 'bg-gradient-to-r from-blue-500 to-indigo-600 hover:from-blue-600 hover:to-indigo-700 text-white shadow-blue-500/10 focus:ring-blue-200',
    },
  }[tone]

  return createPortal(
    <AnimatePresence>
      {isOpen && (
        <div className="fixed inset-0 z-[10000] flex items-center justify-center p-4">
          {/* 背景模糊遮罩层 */}
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={onCancel}
            className="absolute inset-0 bg-slate-900/40 backdrop-blur-sm"
          />

          {/* 模态框主体卡片 */}
          <motion.div
            initial={{ opacity: 0, scale: 0.95, y: 16 }}
            animate={{ opacity: 1, scale: 1, y: 0, transition: { type: 'spring', duration: 0.38, bounce: 0.15 } }}
            exit={{ opacity: 0, scale: 0.96, y: 8, transition: { duration: 0.16 } }}
            className="relative w-full max-w-sm rounded-3xl bg-white p-6 shadow-2xl border border-slate-100 flex flex-col gap-4 z-10"
          >
            {/* 右上角关闭按钮 */}
            <button
              onClick={onCancel}
              className="absolute right-4 top-4 p-1.5 rounded-xl text-slate-400 hover:bg-slate-50 hover:text-slate-700 transition-colors"
            >
              <X size={14} />
            </button>

            {/* 警告图标与标题 */}
            <div className="flex items-center gap-3">
              <div className={`w-9 h-9 rounded-xl flex items-center justify-center shrink-0 ${theme.iconBg}`}>
                <AlertTriangle size={18} />
              </div>
              <div>
                <h4 className="text-xs font-black text-slate-800 leading-none mb-1">{title}</h4>
                <span className="font-mono text-[8px] text-slate-400 tracking-widest leading-none uppercase select-none">SYSTEM CONFIRMATION</span>
              </div>
            </div>

            {/* 确认正文描述 */}
            <p className="text-[10px] font-bold text-slate-500 leading-relaxed font-sans pr-2">
              {message}
            </p>

            {/* 底部按钮栏 */}
            <div className="flex items-center justify-end gap-2 border-t border-slate-100 pt-4 mt-2">
              <button
                onClick={onCancel}
                className="phantom-btn phantom-btn--secondary phantom-btn--sm h-8 min-h-8 px-4 rounded-xl text-[10px]"
              >
                {cancelText}
              </button>
              <button
                onClick={onConfirm}
                className={`phantom-btn phantom-btn--sm h-8 min-h-8 px-5 rounded-xl shadow-md flex items-center justify-center gap-1.5 text-[10px] ${theme.btnClass}`}
              >
                {confirmText}
              </button>
            </div>
          </motion.div>
        </div>
      )}
    </AnimatePresence>,
    document.body
  )
}
