import { motion, AnimatePresence } from 'framer-motion'
import { Mail, ArrowRight, ShieldCheck, Cpu } from 'lucide-react'
import type { EmailItem } from '../types'

interface GridProps {
  data: EmailItem[]
}

export default function Grid({ data }: GridProps) {
  return (
    <div className="w-full h-full relative overflow-hidden bg-slate-50/50 flex flex-col">
      <div className="absolute inset-0 dot-matrix opacity-20 pointer-events-none"></div>

      <div className="h-10 flex items-center justify-between px-6 border-b border-slate-200 bg-slate-50 backdrop-blur z-20 sticky top-0 font-mono">
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2 text-slate-600 text-[10px] tracking-widest">
            <Cpu size={12} className="animate-spin-slow" />
            流缓冲区：<span className="text-blue-500">{data.length} 个节点</span>
          </div>
          <div className="w-px h-3 bg-white/10"></div>
          <div className="text-[10px] text-slate-700 font-black italic">解析引擎已就绪</div>
        </div>
        <div className="flex items-center gap-2">
          <span className="w-1.5 h-1.5 rounded-full bg-blue-500 animate-pulse shadow-[0_0_8px_#3b82f6]"></span>
          <span className="text-blue-400 text-[10px] font-black tracking-tighter">60.0 帧</span>
        </div>
      </div>

      <div className="flex-grow overflow-y-auto custom-scrollbar p-3">
        <div className="grid grid-cols-1 xl:grid-cols-2 gap-2">
          <AnimatePresence mode="popLayout">
            {data.length > 0 ? (
              data.map((item, index) => (
                <motion.div
                  key={item.id}
                  initial={{ opacity: 0, x: -10, scale: 0.98 }}
                  animate={{ opacity: 1, x: 0, scale: 1 }}
                  exit={{ opacity: 0, scale: 0.95, y: 10 }}
                  transition={{ duration: 0.3, delay: index * 0.03, ease: 'circOut' }}
                  className="group relative flex items-center gap-3 py-1.5 px-3 rounded-lg border border-slate-200 bg-slate-50/50 hover:bg-white hover:shadow-md hover:shadow-slate-200/40 transition-all duration-300 backdrop-blur-sm"
                >
                  <div className="absolute left-0 top-0 h-full w-0.5 bg-blue-500/0 group-hover:bg-blue-500 transition-all"></div>

                  <div className="flex items-center gap-2.5 min-w-[150px]">
                    <div className="w-7 h-7 rounded-lg bg-blue-500/10 flex items-center justify-center border border-blue-500/20 group-hover:scale-105 transition-transform shrink-0">
                      <Mail size={13} className="text-blue-500" />
                    </div>
                    <div className="flex flex-col min-w-0">
                      <span className="text-[8px] text-slate-400 font-black tracking-widest leading-none mb-0.5 uppercase">发送源 / FROM</span>
                      <span className="text-[11px] font-mono text-slate-900 opacity-90 truncate leading-none">{item.from}</span>
                    </div>
                  </div>

                   <div className="flex-grow flex flex-col items-center justify-center min-w-[100px] px-2 opacity-60 group-hover:opacity-100 transition-opacity">
                    <span className="text-[8px] text-blue-400 font-black tracking-widest leading-none mb-1 uppercase">接收目标 / TO</span>
                    <span className="text-[10px] font-mono text-blue-600/70 truncate w-full text-center leading-none tracking-tighter">{item.to}</span>
                    <div className="mt-1.5 flex items-center w-full gap-2 opacity-30">
                      <div className="h-[1px] flex-grow bg-gradient-to-r from-transparent to-blue-400"></div>
                      <ArrowRight size={10} className="text-blue-500" />
                      <div className="h-[1px] flex-grow bg-gradient-to-r from-blue-400 to-transparent"></div>
                    </div>
                  </div>

                  <div className="flex flex-col items-end min-w-[90px]">
                    <div className="flex items-center gap-1.5 mb-0">
                      <ShieldCheck size={9} className="text-emerald-500" />
                      <span className="text-[9px] text-slate-500 font-black tracking-widest leading-none uppercase">验证码</span>
                    </div>
                    <div className="px-2 py-0 bg-blue-500/10 border border-blue-500/20 rounded text-blue-600 font-black text-sm font-mono tracking-widest shadow-sm">
                      {item.code || '----'}
                    </div>
                  </div>
                </motion.div>
              ))
            ) : (
              <div className="h-full col-span-full flex flex-col items-center justify-center py-20 opacity-20">
                <Cpu size={48} className="mb-4 animate-pulse" />
                <p className="text-xs font-mono tracking-[0.3em]">等待流注入...</p>
              </div>
            )}
          </AnimatePresence>
        </div>
      </div>

      <div className="h-8 border-t border-slate-200 bg-slate-100 flex items-center px-6 justify-between text-[9px] text-slate-700 font-mono tracking-widest shrink-0">
        <span>缓存同步：{data.length} 个节点</span>
        <span className="text-blue-500/40 animate-pulse">神经解析已就绪</span>
      </div>
    </div>
  )
}
