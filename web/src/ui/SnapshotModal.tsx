import { X, Download, Fullscreen, Image as ImageIcon } from 'lucide-react'
import { motion, AnimatePresence } from 'framer-motion'
import { createPortal } from 'react-dom'

interface SnapshotModalProps {
  url: string | null
  onClose: () => void
}

/**
 * 快照预览模态框组件
 * 用于在不离开页面的情况下查看浏览器驱动抓拍的调试图片
 */
export default function SnapshotModal({ url, onClose }: SnapshotModalProps) {
  if (!url) return null

  return createPortal(
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        className="fixed inset-0 z-[10000] flex items-center justify-center bg-slate-950/90 p-4 backdrop-blur-sm"
        onClick={onClose}
      >
        <motion.div
          initial={{ scale: 0.9, opacity: 0, y: 20 }}
          animate={{ scale: 1, opacity: 1, y: 0 }}
          exit={{ scale: 0.9, opacity: 0, y: 20 }}
          className="relative max-w-5xl w-full bg-slate-900 rounded-3xl overflow-hidden shadow-2xl border border-slate-800"
          onClick={(e) => e.stopPropagation()}
        >
          {/* Header */}
          <div className="px-6 py-4 border-b border-slate-800 flex items-center justify-between bg-slate-900/50 backdrop-blur-md">
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-xl bg-blue-500/10 flex items-center justify-center text-blue-400">
                <ImageIcon size={18} />
              </div>
              <div>
                <h3 className="text-sm font-bold text-white">快照预览 (Snapshot)</h3>
                <p className="text-[10px] text-slate-500 font-mono uppercase tracking-widest">Debug capture from browser driver</p>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <button
                onClick={() => window.open(url, '_blank')}
                className="p-2 rounded-xl hover:bg-slate-800 text-slate-400 hover:text-white transition-all"
                title="原图打开"
              >
                <Fullscreen size={18} />
              </button>
              <a
                href={url}
                download
                className="p-2 rounded-xl hover:bg-slate-800 text-slate-400 hover:text-white transition-all"
                title="下载快照"
              >
                <Download size={18} />
              </a>
              <div className="w-px h-4 bg-slate-800 mx-1" />
              <button
                onClick={onClose}
                className="p-2 rounded-xl hover:bg-rose-500/10 text-slate-400 hover:text-rose-400 transition-all"
              >
                <X size={20} />
              </button>
            </div>
          </div>

          {/* Image Area */}
          <div className="p-4 bg-slate-950 flex items-center justify-center min-h-[300px] max-h-[70vh] overflow-auto scrollbar-thin">
            <img
              src={url}
              alt="Snapshot"
              className="max-w-full h-auto rounded-lg shadow-lg border border-slate-800 select-none animate-in fade-in zoom-in-95 duration-500"
            />
          </div>

          {/* Footer */}
          <div className="px-6 py-3 bg-slate-900 border-t border-slate-800 flex items-center justify-between">
            <span className="text-[10px] text-slate-500 font-mono tracking-tighter">PHANTOM_DEBUGGER::SNAPSHOT_VIEWER_PRO</span>
            <button
              onClick={onClose}
              className="px-4 py-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 text-white text-[11px] font-bold transition-all shadow-sm"
            >
              关闭预览
            </button>
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>,
    document.body,
  )
}
