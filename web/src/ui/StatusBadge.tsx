import { AlertCircle, CheckCircle2, Clock, Loader2, UploadCloud } from 'lucide-react'
import type { WorkflowRunRecord, WorkflowStepRecord } from '../types'

export function AccountStatusBadge({ status }: { status: string }) {
  const normalized = status.toLowerCase()
  const isUploaded = normalized === 'uploaded'
  const isSuccess =
    normalized.includes('registered') || normalized === 'success' || normalized.includes('active') || isUploaded
  const isError = normalized.includes('banned') || normalized.includes('expired') || normalized.includes('invalid') || normalized.includes('failed')
  const isNone = normalized.includes('no token')
  const isCooling = normalized.includes('cooling') || normalized.includes('cooldown') || normalized.includes('冷却')

  const tone = isUploaded
    ? 'bg-violet-50 text-violet-600 border-violet-100'
    : isCooling
      ? 'bg-indigo-50 text-indigo-600 border-indigo-100 animate-pulse'
      : isSuccess
        ? 'bg-emerald-50 text-emerald-600 border-emerald-100'
        : isError
          ? 'bg-rose-50 text-rose-600 border-rose-100'
          : isNone
            ? 'bg-slate-100 text-slate-500 border-slate-200'
            : 'bg-amber-50 text-amber-600 border-amber-100'

  return (
    <span
      title={`账号状态: ${status}`}
      className={`flex cursor-default items-center gap-1.5 rounded-full border px-3 py-1 text-[10px] font-black uppercase tracking-tighter shadow-sm transition-all hover:scale-105 active:scale-95 ${tone}`}
    >
      {isUploaded ? <UploadCloud size={10} /> : isSuccess ? <CheckCircle2 size={10} /> : <AlertCircle size={10} />}
      {isUploaded ? '已上传 (CPA)' : status}
    </span>
  )
}

export function RunStatusBadge({ status }: { status: WorkflowRunRecord['status'] }) {
  return (
    <span className={`rounded-full px-3 py-1 text-[10px] font-black tracking-widest ${runStatusTone(status)}`}>
      {translateRunStatus(status)}
    </span>
  )
}

export function StepStatusBadge({ level }: { level: WorkflowStepRecord['level'] }) {
  const icon = level === 'running' ? <Loader2 size={10} className="animate-spin" /> : level === 'success' ? <CheckCircle2 size={10} /> : level === 'info' ? <Clock size={10} /> : <AlertCircle size={10} />

  return (
    <span className={`inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-[10px] font-black uppercase tracking-wider ${stepStatusTone(level)}`}>
      {icon}
      {translateStepLevel(level)}
    </span>
  )
}

function translateRunStatus(status: WorkflowRunRecord['status']) {
  const map: Record<WorkflowRunRecord['status'], string> = {
    running: '运行中',
    success: '成功',
    warn: '警告',
    error: '错误',
    cancelled: '已取消',
  }
  return map[status] ?? status
}

function translateStepLevel(level: WorkflowStepRecord['level']) {
  const map: Record<WorkflowStepRecord['level'], string> = {
    running: '运行中',
    success: '成功',
    warn: '警告',
    info: '信息',
    error: '错误',
    cancelled: '已取消',
  }
  return map[level] ?? level
}

function runStatusTone(status: WorkflowRunRecord['status']) {
  const map: Record<WorkflowRunRecord['status'], string> = {
    running: 'bg-blue-50 text-blue-600',
    success: 'bg-emerald-50 text-emerald-600',
    warn: 'bg-amber-50 text-amber-600',
    error: 'bg-rose-50 text-rose-600',
    cancelled: 'bg-slate-100 text-slate-500',
  }
  return map[status] ?? 'bg-slate-100 text-slate-500'
}

function stepStatusTone(level: WorkflowStepRecord['level']) {
  const map: Record<WorkflowStepRecord['level'], string> = {
    running: 'bg-blue-50 text-blue-600',
    success: 'bg-emerald-50 text-emerald-600',
    warn: 'bg-amber-50 text-amber-600',
    info: 'bg-slate-100 text-slate-500',
    error: 'bg-rose-50 text-rose-600',
    cancelled: 'bg-slate-100 text-slate-500',
  }
  return map[level] ?? 'bg-slate-100 text-slate-500'
}
