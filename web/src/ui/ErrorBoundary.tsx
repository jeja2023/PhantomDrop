import { Component, type ErrorInfo, type ReactNode } from 'react'
import { AlertTriangle, RotateCcw, TerminalSquare } from 'lucide-react'

interface ErrorBoundaryProps {
  children: ReactNode
}

interface ErrorBoundaryState {
  hasError: boolean
  message: string
}

export default class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = {
    hasError: false,
    message: '',
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return {
      hasError: true,
      message: error.message || 'UNKNOWN_ERROR',
    }
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('PhantomDrop UI crashed', error, errorInfo)
  }

  private handleReload = () => {
    window.location.reload()
  }

  private handleOpenConsole = () => {
    window.open('http://127.0.0.1:4000/', '_blank', 'noopener,noreferrer')
  }

  render() {
    if (!this.state.hasError) {
      return this.props.children
    }

    return (
      <div className="min-h-screen bg-slate-50 px-6 py-10 text-slate-900">
        <div className="mx-auto flex min-h-[80vh] max-w-3xl items-center justify-center">
          <div className="w-full rounded-3xl border border-rose-200 bg-white p-10 shadow-xl shadow-rose-100/60">
            <div className="flex items-start gap-4">
              <div className="rounded-2xl bg-rose-50 p-3 text-rose-600">
                <AlertTriangle size={24} />
              </div>
              <div className="space-y-3">
                <div>
                  <h1 className="text-2xl font-black tracking-tight">前端界面发生运行时错误</h1>
                  <p className="mt-1 text-sm text-slate-600">Web 主界面已中断渲染。你可以直接刷新页面，或者暂时切到 Rust 内建控制台继续工作。</p>
                </div>
                <div className="rounded-2xl border border-slate-200 bg-slate-50 p-4">
                  <div className="text-[10px] font-black uppercase tracking-[0.2em] text-slate-500">ERROR_MESSAGE</div>
                  <div className="mt-2 break-all font-mono text-sm text-rose-700">{this.state.message}</div>
                </div>
                <div className="flex flex-wrap gap-3">
                  <button
                    type="button"
                    onClick={this.handleReload}
                    className="inline-flex items-center gap-2 rounded-2xl bg-blue-600 px-5 py-3 text-sm font-bold text-white transition-colors hover:bg-blue-500"
                  >
                    <RotateCcw size={16} />
                    刷新页面
                  </button>
                  <button
                    type="button"
                    onClick={this.handleOpenConsole}
                    className="inline-flex items-center gap-2 rounded-2xl border border-slate-200 bg-white px-5 py-3 text-sm font-bold text-slate-700 transition-colors hover:bg-slate-50"
                  >
                    <TerminalSquare size={16} />
                    打开内建控制台
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    )
  }
}
