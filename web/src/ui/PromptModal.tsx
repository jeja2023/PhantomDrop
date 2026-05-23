import { useState, useEffect } from 'react'
import { Edit3, X } from 'lucide-react'
import { motion, AnimatePresence } from 'framer-motion'
import { createPortal } from 'react-dom'

interface PromptModalProps {
  isOpen: boolean
  title: string
  message: string
  placeholder?: string
  defaultValue?: string
  confirmText?: string
  cancelText?: string
  onConfirm: (value: string) => void
  onCancel: () => void
}

export default function PromptModal({
  isOpen,
  title,
  message,
  placeholder = '请输入内容...',
  defaultValue = '',
  confirmText = '确定',
  cancelText = '取消',
  onConfirm,
  onCancel,
}: PromptModalProps) {
  const [inputValue, setInputValue] = useState(defaultValue)

  // 当 defaultValue 改变或者弹窗打开时更新输入框内容
  useEffect(() => {
    if (isOpen) {
      setInputValue(defaultValue)
    }
  }, [isOpen, defaultValue])

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onConfirm(inputValue)
  }

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

            {/* 提示图标与标题 */}
            <div className="flex items-center gap-3">
              <div className="w-9 h-9 rounded-xl flex items-center justify-center shrink-0 bg-purple-50 text-purple-600 border border-purple-100">
                <Edit3 size={18} />
              </div>
              <div>
                <h4 className="text-xs font-black text-slate-800 leading-none mb-1">{title}</h4>
                <span className="font-mono text-[8px] text-slate-400 tracking-widest leading-none uppercase select-none">INPUT PROMPT</span>
              </div>
            </div>

            {/* 输入框与表单 */}
            <form onSubmit={handleSubmit} className="flex flex-col gap-3">
              <p className="text-[10px] font-bold text-slate-500 leading-relaxed font-sans pr-2">
                {message}
              </p>

              <input
                type="text"
                autoFocus
                value={inputValue}
                onChange={(e) => setInputValue(e.target.value)}
                placeholder={placeholder}
                className="w-full bg-slate-50 border border-slate-200 rounded-xl px-4 py-2.5 text-xs font-bold outline-none focus:bg-white focus:border-purple-500 focus:ring-4 focus:ring-purple-100 transition-all duration-300 shadow-inner focus:shadow-[0_0_12px_rgba(168,85,247,0.25)]"
              />

              {/* 底部按钮栏 */}
              <div className="flex items-center justify-end gap-2 border-t border-slate-100 pt-4 mt-2">
                <button
                  type="button"
                  onClick={onCancel}
                  className="phantom-btn phantom-btn--secondary phantom-btn--sm h-8 min-h-8 px-4 rounded-xl text-[10px]"
                >
                  {cancelText}
                </button>
                <button
                  type="submit"
                  className="phantom-btn phantom-btn--sm h-8 min-h-8 px-5 rounded-xl shadow-md bg-gradient-to-r from-purple-500 to-indigo-600 hover:from-purple-600 hover:to-indigo-700 text-white shadow-purple-500/10 flex items-center justify-center gap-1.5 text-[10px] focus:ring-purple-200"
                >
                  {confirmText}
                </button>
              </div>
            </form>
          </motion.div>
        </div>
      )}
    </AnimatePresence>,
    document.body
  )
}
