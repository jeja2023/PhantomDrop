import React, { useState, useEffect } from 'react';
import { 
  Users, 
  Search, 
  Download, 
  RefreshCw, 
  Copy, 
  ExternalLink, 
  ShieldCheck, 
  Database,
  Calendar,
  Lock,
  ChevronLeft,
  ChevronRight,
  MoreVertical,
  CheckCircle2,
  AlertCircle
} from 'lucide-react';
import { fetchJson } from '../lib/api';
import type { GeneratedAccountRecord } from '../types';

interface AccountPageResponse {
  items: GeneratedAccountRecord[];
  limit: number;
  offset: number;
}

const AccountListView: React.FC = () => {
  const [accounts, setAccounts] = useState<GeneratedAccountRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState('');
  const [page, setPage] = useState(1);
  const [pageSize] = useState(15);
  const [total, setTotal] = useState(0);

  const loadAccounts = async () => {
    setLoading(true);
    try {
      const offset = (page - 1) * pageSize;
      // 注意：后端目前返回的是 {items, limit, offset}，并没有返回总数，
      // 我们先假设列表能拉取到，或者通过 limit 控制
      const data = await fetchJson<AccountPageResponse>(`/api/accounts?limit=${pageSize}&offset=${offset}`);
      setAccounts(data.items);
      // 由于后端没返回总数，我们简单处理：如果有数据且达到 pageSize，则假设有下一页
      if (data.items.length === pageSize) {
          setTotal(page * pageSize + 1);
      } else {
          setTotal((page - 1) * pageSize + data.items.length);
      }
    } catch (error) {
      console.error('Failed to load accounts:', error);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadAccounts();
  }, [page]);

  const handleExport = () => {
    window.open('/api/workflow-runs/all/accounts/export', '_blank');
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
    // 可选：触发一个全局 Toast
    const event = new CustomEvent('phantom-log', { detail: { msg: '已复制到剪贴板', level: 'success' } });
    window.dispatchEvent(event);
  };

  const filteredAccounts = accounts.filter(acc => 
    acc.address.toLowerCase().includes(search.toLowerCase()) ||
    acc.status.toLowerCase().includes(search.toLowerCase()) ||
    acc.run_id.toLowerCase().includes(search.toLowerCase())
  );

  return (
    <div className="h-full flex flex-col space-y-6 animate-in fade-in slide-in-from-bottom-4 duration-500">
      <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-4">
        <div className="flex items-center gap-4">
          <div className="w-12 h-12 rounded-2xl bg-indigo-600 flex items-center justify-center text-white shadow-lg shadow-indigo-200">
            <Users size={24} />
          </div>
          <div>
            <h1 className="text-2xl font-black text-slate-900 tracking-tight">账号产物中心</h1>
            <div className="flex items-center gap-2 mt-1">
              <span className="text-[10px] font-mono text-slate-500 uppercase tracking-widest">Account Management & Repository</span>
              <span className="w-1 h-1 rounded-full bg-slate-300"></span>
              <span className="text-[10px] font-mono text-indigo-600 font-bold uppercase tracking-widest">Global Vault</span>
            </div>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <button 
            onClick={() => loadAccounts()}
            className="phantom-btn phantom-btn--secondary"
            disabled={loading}
          >
            <RefreshCw size={14} className={loading ? 'animate-spin' : ''} />
            刷新
          </button>
          <button 
            onClick={handleExport}
            className="phantom-btn phantom-btn--primary"
          >
            <Download size={14} />
            批量导出 (.CSV)
          </button>
        </div>
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-4 gap-6 min-h-0 flex-grow overflow-hidden">
        <div className="xl:col-span-1 space-y-6">
          <div className="glass-panel rounded-3xl p-6 border border-slate-200 shadow-sm space-y-4">
            <div className="flex items-center gap-2 text-indigo-600 mb-2">
              <Search size={16} />
              <h3 className="text-sm font-black uppercase tracking-wider">过滤筛选</h3>
            </div>
            <div className="relative group">
              <input
                type="text"
                placeholder="搜索地址 / 状态 / RunID..."
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="w-full bg-slate-50 border border-slate-200 rounded-2xl px-4 py-3 text-sm outline-none focus:border-indigo-500 focus:bg-white transition-all shadow-inner pl-10"
              />
              <Search className="absolute left-3.5 top-1/2 -translate-y-1/2 text-slate-400 group-focus-within:text-indigo-500 transition-colors" size={16} />
            </div>
            
            <div className="pt-2">
                <div className="p-4 rounded-2xl bg-slate-50 border border-dashed border-slate-300">
                   <p className="text-[11px] text-slate-500 leading-relaxed text-center">
                     系统会自动汇总所有 OpenAI 工作流生成的产物。包含成功的注册记录、Token 信息以及环境指纹数据。
                   </p>
                </div>
            </div>
          </div>

          <div className="glass-panel rounded-3xl p-6 border border-slate-200 shadow-sm">
            <h3 className="text-sm font-black text-slate-900 mb-4 flex items-center gap-2">
              <ShieldCheck size={16} className="text-emerald-500" />
              存储状态
            </h3>
            <div className="space-y-4">
                <StatItem label="主数据库" value="活跃 (Active)" sub="SQLite Standard" />
                <StatItem label="今日新增" value="+24" sub="Accounts" />
                <StatItem label="同步状态" value="100%" sub="Local Sync" />
            </div>
          </div>
        </div>

        <div className="xl:col-span-3 flex flex-col min-h-0 bg-white rounded-3xl border border-slate-200 shadow-sm overflow-hidden">
          <div className="flex-grow overflow-auto scrollbar-thin">
            <table className="w-full text-left border-collapse">
              <thead className="sticky top-0 z-10 bg-slate-50/90 backdrop-blur-md border-b border-slate-200">
                <tr>
                  <th className="px-6 py-4 text-[10px] font-black text-slate-500 uppercase tracking-widest leading-none">账号详情</th>
                  <th className="px-6 py-4 text-[10px] font-black text-slate-500 uppercase tracking-widest leading-none">注册密令</th>
                  <th className="px-6 py-4 text-[10px] font-black text-slate-500 uppercase tracking-widest leading-none">同步时间</th>
                  <th className="px-6 py-4 text-[10px] font-black text-slate-500 uppercase tracking-widest leading-none">账号类型</th>
                  <th className="px-6 py-4 text-[10px] font-black text-slate-500 uppercase tracking-widest leading-none">当前状态</th>
                  <th className="px-6 py-4 text-[10px] font-black text-slate-500 uppercase tracking-widest leading-none text-right">操作</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100">
                {loading && accounts.length === 0 ? (
                  <tr>
                    <td colSpan={6} className="py-20 text-center">
                      <div className="flex flex-col items-center gap-3">
                         <RefreshCw className="w-8 h-8 text-indigo-400 animate-spin" />
                         <span className="text-xs font-mono text-slate-400 animate-pulse">正在从数据湖摄取资产记录...</span>
                      </div>
                    </td>
                  </tr>
                ) : filteredAccounts.length === 0 ? (
                  <tr>
                    <td colSpan={6} className="py-20 text-center">
                      <div className="flex flex-col items-center gap-3">
                         <Database className="w-12 h-12 text-slate-200" />
                         <span className="text-xs font-bold text-slate-400">未找到符合条件的账号记录</span>
                      </div>
                    </td>
                  </tr>
                ) : (
                  filteredAccounts.map((account) => (
                    <tr key={account.id} className="group hover:bg-slate-50/50 transition-colors">
                      <td className="px-6 py-5">
                        <div className="flex flex-col">
                          <span className="text-sm font-black text-slate-800 tracking-tight">{account.address}</span>
                          <span className="text-[10px] font-mono text-slate-400 mt-1">ID: {account.id.slice(0, 13)}...</span>
                        </div>
                      </td>
                      <td className="px-6 py-5">
                        <div className="flex items-center gap-2 group/pwd cursor-pointer" onClick={() => copyToClipboard(account.password)}>
                          <div className="w-6 h-6 rounded-lg bg-slate-100 flex items-center justify-center text-slate-400 group-hover/pwd:bg-indigo-100 group-hover/pwd:text-indigo-600 transition-colors">
                            <Lock size={12} />
                          </div>
                          <code className="text-[11px] font-mono text-slate-500 bg-slate-50 px-2 py-0.5 rounded group-hover/pwd:bg-indigo-50 group-hover/pwd:text-indigo-700 transition-colors">
                            {account.password}
                          </code>
                          <Copy size={12} className="text-slate-300 opacity-0 group-hover/pwd:opacity-100 transition-all ml-1" />
                        </div>
                      </td>
                      <td className="px-6 py-5">
                        <div className="flex items-center gap-2 text-slate-500">
                          <Calendar size={12} className="text-slate-400" />
                          <span className="text-[11px] font-mono leading-none">
                            {new Date(account.created_at * 1000).toLocaleString('zh-CN', { 
                                month: '2-digit', 
                                day: '2-digit',
                                hour: '2-digit',
                                minute: '2-digit'
                             })}
                          </span>
                        </div>
                      </td>
                      <td className="px-6 py-5">
                        <span className="px-2 py-1 rounded bg-slate-100 text-slate-600 text-[10px] font-black uppercase tracking-widest border border-slate-200">
                           {account.account_type || 'FREE'}
                        </span>
                      </td>
                      <td className="px-6 py-5">
                        <div className="flex items-center">
                           <StatusBadge status={account.status} />
                        </div>
                      </td>
                      <td className="px-6 py-5 text-right">
                        <div className="flex items-center justify-end gap-2">
                           <button className="p-2 rounded-xl text-slate-400 hover:text-indigo-600 hover:bg-indigo-50 transition-all opacity-0 group-hover:opacity-100">
                             <ExternalLink size={16} />
                           </button>
                           <button className="p-2 rounded-xl text-slate-400 hover:text-slate-600 hover:bg-slate-100 transition-all">
                             <MoreVertical size={16} />
                           </button>
                        </div>
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>

          <div className="px-6 py-4 bg-slate-50 border-t border-slate-200 flex items-center justify-between">
            <div className="text-[10px] font-mono text-slate-500">
              第 {page} 页 / 共 {Math.ceil(total / pageSize)} 页
            </div>
            <div className="flex items-center gap-2">
              <button 
                onClick={() => setPage(p => Math.max(1, p - 1))}
                disabled={page <= 1}
                className="p-1.5 rounded-lg border border-slate-200 bg-white text-slate-600 hover:bg-slate-50 disabled:opacity-40 transition-all"
              >
                <ChevronLeft size={16} />
              </button>
              <button 
                onClick={() => setPage(p => p + 1)}
                disabled={accounts.length < pageSize}
                className="p-1.5 rounded-lg border border-slate-200 bg-white text-slate-600 hover:bg-slate-50 disabled:opacity-40 transition-all"
              >
                <ChevronRight size={16} />
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

const StatItem = ({ label, value, sub }: { label: string; value: string; sub: string }) => (
    <div className="flex items-center justify-between group">
        <div>
            <p className="text-[10px] font-mono text-slate-400 uppercase tracking-tighter group-hover:text-indigo-600 transition-colors">{label}</p>
            <p className="text-xs font-bold text-slate-700">{value}</p>
        </div>
        <span className="text-[9px] font-black text-slate-300 uppercase italic tracking-widest">{sub}</span>
    </div>
);

const StatusBadge = ({ status }: { status: string }) => {
    const isSuccess = status.toLowerCase().includes('registered') || status === 'success';
    return (
        <span className={`flex items-center gap-1.5 px-3 py-1 rounded-full text-[10px] font-black uppercase tracking-tighter border ${
            isSuccess 
            ? 'bg-emerald-50 text-emerald-600 border-emerald-100' 
            : 'bg-amber-50 text-amber-600 border-amber-100'
        }`}>
            {isSuccess ? <CheckCircle2 size={10} /> : <AlertCircle size={10} />}
            {status}
        </span>
    );
};

export default AccountListView;
