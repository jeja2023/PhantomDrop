import { Lock } from 'lucide-react'

export function FieldCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-2xl border border-slate-100 bg-slate-50 p-4">
      <p className="mb-1 text-[10px] font-black uppercase tracking-widest text-slate-400">{label}</p>
      <code className="break-all text-[11px] font-mono text-slate-700">{value}</code>
    </div>
  )
}

export function SecretField({
  label,
  value,
  emptyLabel = '暂无 Token 数据',
  onCopy,
}: {
  label: string
  value: string | null | undefined
  emptyLabel?: string
  onCopy?: (value: string) => void
}) {
  return (
    <div className="group space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-[10px] font-black uppercase tracking-widest text-slate-500">{label}</span>
        {value && onCopy ? (
          <button
            type="button"
            onClick={() => onCopy(value)}
            className="text-[10px] font-bold text-indigo-600 opacity-0 transition-colors hover:text-indigo-700 group-hover:opacity-100"
          >
            复制
          </button>
        ) : null}
      </div>
      <div className="relative">
        <textarea
          readOnly
          value={value || emptyLabel}
          className={`w-full min-h-[80px] resize-none rounded-2xl border border-slate-200 bg-slate-50 p-4 text-[11px] font-mono outline-none transition-all focus:border-indigo-500 focus:bg-white ${
            !value ? 'text-slate-400 italic' : 'text-slate-700'
          }`}
        />
        {value ? (
          <div className="absolute right-3 top-3">
            <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-indigo-50 text-indigo-600 opacity-40 transition-opacity group-hover:opacity-100">
              <Lock size={14} />
            </div>
          </div>
        ) : null}
      </div>
    </div>
  )
}
