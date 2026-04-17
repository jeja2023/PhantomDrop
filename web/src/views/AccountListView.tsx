import { useState, useEffect, type FC } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { 
  Search, 
  Download, 
  RefreshCw, 
  Copy, 
  ExternalLink, 
  ShieldCheck, 
  Database,
  Calendar,
  Lock,
  CloudUpload,
  ChevronLeft,
  ChevronRight,
  CheckCircle2,
  AlertCircle,
  Trash2,
  Trash,
  Key,
  X
} from 'lucide-react';
import { fetchJson, deleteJson, postJson } from '../lib/api';
import type { GeneratedAccountRecord, DashboardStats } from '../types';

interface AccountPageResponse {
  items: GeneratedAccountRecord[];
  limit: number;
  offset: number;
  total: number;
}

const AccountListView: FC = () => {
  const [accounts, setAccounts] = useState<GeneratedAccountRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState('');
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(10);
  const [total, setTotal] = useState(0);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [checkingIds, setCheckingIds] = useState<string[]>([]);
  const [selectedAccount, setSelectedAccount] = useState<GeneratedAccountRecord | null>(null);
  const [stats, setStats] = useState<DashboardStats | null>(null);
  const [showToast, setShowToast] = useState(false);
  const [toastMsg, setToastMsg] = useState('');

  const loadAccounts = async () => {
    setLoading(true);
    try {
      const offset = (page - 1) * pageSize;
      const queryParam = search ? `&q=${encodeURIComponent(search)}` : '';
      const data = await fetchJson<AccountPageResponse>(`/api/accounts?limit=${pageSize}&offset=${offset}${queryParam}`);
      setAccounts(data.items);
      setTotal(data.total);
      
      // 同时加载统计信息
      const statsData = await fetchJson<any>('/api/stats');
      setStats(statsData);
    } catch (error) {
      console.error('Failed to load accounts/stats:', error);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadAccounts();
  }, [page, search]);

  const handleExport = () => {
    window.open('/api/workflow-runs/all/accounts/export', '_blank');
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
    setToastMsg(text.length > 24 ? '数据已复制到剪贴板' : `已复制: ${text}`);
    setShowToast(true);
    setTimeout(() => setShowToast(false), 2000);
    const event = new CustomEvent('phantom-log', { detail: { msg: `用户复制了数据: ${text.slice(0, 20)}...`, level: 'info' } });
    window.dispatchEvent(event);
  };

  const handleDelete = async (id: string) => {
    if (!window.confirm('确定要永久删除这条账号记录吗？')) return;
    try {
      await deleteJson(`/api/accounts/${id}`);
      setAccounts(prev => prev.filter(acc => acc.id !== id));
      setSelectedIds(prev => prev.filter(item => item !== id));
      setTotal(prev => prev - 1);
      
      const event = new CustomEvent('phantom-log', { detail: { msg: '账号记录已删除', level: 'success' } });
      window.dispatchEvent(event);
    } catch (error) {
      console.error('Failed to delete account:', error);
      const event = new CustomEvent('phantom-log', { detail: { msg: '删除失败', level: 'error' } });
      window.dispatchEvent(event);
    }
  };

  const handleBatchDelete = async () => {
    if (selectedIds.length === 0) return;
    if (!window.confirm(`确定要永久删除选中的 ${selectedIds.length} 条账号记录吗？`)) return;
    
    try {
      await fetchJson('/api/accounts/batch', {
        method: 'DELETE',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify({ ids: selectedIds })
      });
      
      setAccounts(prev => prev.filter(acc => !selectedIds.includes(acc.id)));
      setTotal(prev => prev - selectedIds.length);
      setSelectedIds([]);
      
      const event = new CustomEvent('phantom-log', { detail: { msg: '批量删除完成', level: 'success' } });
      window.dispatchEvent(event);
    } catch (error) {
      console.error('Failed to batch delete accounts:', error);
      const event = new CustomEvent('phantom-log', { detail: { msg: '批量删除失败', level: 'error' } });
      window.dispatchEvent(event);
    }
  };

  const toggleSelect = (id: string) => {
    setSelectedIds(prev => prev.includes(id) ? prev.filter(item => item !== id) : [...prev, id]);
  };

  const toggleSelectAll = () => {
    const allOnPageSelected = accounts.length > 0 && accounts.every(acc => selectedIds.includes(acc.id));
    if (allOnPageSelected) {
      setSelectedIds([]);
    } else {
      const pageIds = accounts.map(acc => acc.id);
      setSelectedIds(prev => Array.from(new Set([...prev, ...pageIds])));
    }
  };

  const handleSelectAllAcrossPages = async () => {
    setLoading(true);
    try {
      const queryParam = search ? `?q=${encodeURIComponent(search)}` : '';
      const res = await fetchJson<{ status: string, ids: string[] }>(`/api/accounts/ids${queryParam}`);
      if (res.status === 'success') {
        setSelectedIds(res.ids);
        const event = new CustomEvent('phantom-log', { 
            detail: { msg: `已选中全部 ${res.ids.length} 个账号`, level: 'info' } 
        });
        window.dispatchEvent(event);
      }
    } catch (error) {
      console.error('Failed to select all IDs:', error);
    } finally {
      setLoading(false);
    }
  };

  const handleCheckStatus = async (id: string) => {
    if (checkingIds.includes(id)) return;
    
    setCheckingIds(prev => [...prev, id]);
    try {
      const res = await postJson<{ status: string, account_status: string }, any>(`/api/accounts/${id}/check-status`, {});
      
      if (res.status === 'success') {
          // 更新本地列表中的状态
          setAccounts(prev => prev.map(acc => 
            acc.id === id ? { ...acc, status: res.account_status } : acc
          ));
          
          const event = new CustomEvent('phantom-log', { 
            detail: { msg: `状态检查完成: ${res.account_status}`, level: 'success' } 
          });
          window.dispatchEvent(event);
      }
    } catch (error: any) {
      console.error('Failed to check status:', error);
      const event = new CustomEvent('phantom-log', { 
        detail: { msg: `检查失败: ${error.message || '网络错误'}`, level: 'error' } 
      });
      window.dispatchEvent(event);
    } finally {
      setCheckingIds(prev => prev.filter(item => item !== id));
    }
  };

  const handleBatchCheckStatus = async () => {
    if (selectedIds.length === 0) return;
    
    setLoading(true);
    try {
      const res = await postJson<{ status: string, results: { id: string, status: string }[] }, any>('/api/accounts/batch/check-status', { ids: selectedIds });
      
      if (res.status === 'success') {
          // 更新本地列表中的状态
          setAccounts(prev => prev.map(acc => {
              const result = res.results.find(r => r.id === acc.id);
              return result ? { ...acc, status: result.status } : acc;
          }));
          
          const event = new CustomEvent('phantom-log', { 
            detail: { msg: `批量检查完成，已更新 ${res.results.length} 条数据`, level: 'success' } 
          });
          window.dispatchEvent(event);
      }
    } catch (error: any) {
      console.error('Failed to batch check status:', error);
      const event = new CustomEvent('phantom-log', { 
        detail: { msg: `批量检查失败: ${error.message || '网络错误'}`, level: 'error' } 
      });
      window.dispatchEvent(event);
    } finally {
      setLoading(false);
    }
  };

  const handleBatchUploadCpa = async () => {
    if (selectedIds.length === 0) return;
    
    setLoading(true);
    try {
      const res = await postJson<{ status: string, message: string }, any>('/api/accounts/batch/upload-cpa', { ids: selectedIds });
      
      if (res.status === 'success') {
          const event = new CustomEvent('phantom-log', { 
            detail: { msg: `CPA 分发完成: ${res.message}`, level: 'success' } 
          });
          window.dispatchEvent(event);
      }
    } catch (error: any) {
      console.error('Failed to upload to CPA:', error);
      const event = new CustomEvent('phantom-log', { 
        detail: { msg: `CPA 分发失败: ${error.message || '由于设置未配置或网络错误'}`, level: 'error' } 
      });
      window.dispatchEvent(event);
    } finally {
      setLoading(false);
    }
  };

  const handleCleanupFailures = async () => {
    if (!window.confirm('确定要清理所有注册失败（状态非 Registered/Success）的记录吗？')) return;
    try {
      const res = await postJson<{ status: string, deleted: number }, any>('/api/accounts/cleanup-failures', {});
      const count = res.deleted || 0;
      
      const event = new CustomEvent('phantom-log', { 
        detail: { msg: `清理完成，共移除 ${count} 条失败记录`, level: 'success' } 
      });
      window.dispatchEvent(event);
      loadAccounts();
    } catch (error) {
      console.error('Failed to cleanup failures:', error);
      const event = new CustomEvent('phantom-log', { 
        detail: { msg: '清理操作失败', level: 'error' } 
      });
      window.dispatchEvent(event);
    }
  };

  const filteredAccounts = accounts;

  return (
    <div className="h-full flex flex-col space-y-6 animate-in fade-in slide-in-from-bottom-4 duration-500">
      <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-4">
        <div />

        <div className="flex items-center gap-3">
          <button 
            onClick={() => loadAccounts()}
            className="phantom-btn phantom-btn--secondary phantom-btn--sm"
            disabled={loading}
          >
            <RefreshCw size={12} className={loading ? 'animate-spin' : ''} />
            刷新
          </button>
          <button 
            onClick={handleCleanupFailures}
            className="phantom-btn phantom-btn--secondary phantom-btn--sm hover:text-rose-600"
            title="清理所有注册失败的账号"
          >
            <Trash size={12} />
            清理失败项
          </button>
          {selectedIds.length > 0 && (
            <button 
              onClick={handleBatchCheckStatus}
              className="phantom-btn phantom-btn--secondary phantom-btn--sm hover:text-indigo-600"
              disabled={loading}
              title="批量检测选中账号的存活状态"
            >
              <ShieldCheck size={12} className={loading ? 'animate-spin' : ''} />
              批量检测存活 ({selectedIds.length})
            </button>
          )}
          {selectedIds.length > 0 && (
            <button 
              onClick={handleBatchUploadCpa}
              className="phantom-btn phantom-btn--secondary phantom-btn--sm hover:text-emerald-600"
              disabled={loading}
              title="一键同步至 CPA/分发平台"
            >
              <CloudUpload size={12} className={loading ? 'animate-pulse' : ''} />
              同步至 CPA ({selectedIds.length})
            </button>
          )}
          {selectedIds.length > 0 && (
            <button 
              onClick={handleBatchDelete}
              className="phantom-btn phantom-btn--danger phantom-btn--sm"
              disabled={loading}
            >
              <Trash2 size={12} />
              批量删除 ({selectedIds.length})
            </button>
          )}
          <button 
            onClick={handleExport}
            className="phantom-btn phantom-btn--primary phantom-btn--sm"
          >
            <Download size={12} />
            导出 (.CSV)
          </button>
        </div>
      </div>

      <div className="flex flex-col space-y-4 flex-grow min-h-0 overflow-hidden">
        {/* 顶部过滤与状态区 */}
        <div className="grid grid-cols-1 lg:grid-cols-4 gap-4">
          {/* 搜索过滤 */}
          <div className="lg:col-span-2 glass-panel rounded-2xl py-2.5 px-4 border border-slate-200 shadow-sm flex items-center gap-4">
            <div className="flex items-center gap-2 text-indigo-600 shrink-0">
              <Search size={14} />
              <h3 className="text-xs font-black uppercase tracking-wider">过滤筛选</h3>
            </div>
            <div className="relative flex-grow group">
              <input
                type="text"
                placeholder="搜索地址 / 状态 / RunID..."
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="w-full bg-slate-50 border border-slate-200 rounded-xl px-4 py-2 text-sm outline-none focus:border-indigo-500 focus:bg-white transition-all pl-10 h-9"
              />
              <Search className="absolute left-3.5 top-1/2 -translate-y-1/2 text-slate-400 group-focus-within:text-indigo-500 transition-colors" size={14} />
            </div>
          </div>

          {/* 存储状态 */}
          <div className="lg:col-span-2 glass-panel rounded-2xl py-2.5 px-4 border border-slate-200 shadow-sm flex items-center justify-around gap-4 bg-indigo-50/10">
            <div className="flex items-center gap-3">
               <div className="w-7 h-7 rounded-lg bg-emerald-100 flex items-center justify-center text-emerald-600">
                  <Database size={14} />
               </div>
               <StatItem label="库存总量" value={`${total} 条`} sub="ACCOUNTS" />
            </div>
            <div className="w-px h-8 bg-slate-200 mx-2 hidden sm:block"></div>
            <div className="flex items-center gap-3">
               <div className="w-7 h-7 rounded-lg bg-indigo-100 flex items-center justify-center text-indigo-600">
                  <RefreshCw size={14} />
               </div>
               <StatItem label="主库负载" value="活跃 (Active)" sub="SYMBOLS" />
            </div>
            <div className="w-px h-8 bg-slate-200 mx-2 hidden sm:block"></div>
            <div className="flex items-center gap-3">
               <div className="w-7 h-7 rounded-lg bg-amber-100 flex items-center justify-center text-amber-600">
                  <ShieldCheck size={14} />
               </div>
               <StatItem label="今日新增" value={`+${stats?.today_accounts_24h || 0}`} sub="NEW" />
            </div>
          </div>
        </div>

        {/* 核心数据表格 */}
        <div className="flex flex-col min-h-0 bg-white rounded-3xl border border-slate-200 shadow-sm overflow-hidden flex-grow">
          <div className="flex-grow overflow-auto scrollbar-thin">
            <table className="w-full text-left border-collapse">
              <thead className="sticky top-0 z-10 bg-slate-50/90 backdrop-blur-md">
                <tr>
                  <th className="border border-slate-200 px-4 py-2 w-[40px]">
                    <input 
                      type="checkbox" 
                      className="rounded border-slate-300 text-indigo-600 focus:ring-indigo-500"
                      checked={accounts.length > 0 && accounts.every(acc => selectedIds.includes(acc.id))}
                      onChange={toggleSelectAll}
                    />
                  </th>
                  <th className="border border-slate-200 px-4 py-2 text-[10px] font-black text-slate-500 uppercase tracking-widest leading-none w-[25%]">账号详情</th>
                  <th className="border border-slate-200 px-4 py-2 text-[10px] font-black text-slate-500 uppercase tracking-widest leading-none w-[20%]">注册密令</th>
                  <th className="border border-slate-200 px-4 py-2 text-[10px] font-black text-slate-500 uppercase tracking-widest leading-none w-[21%]">同步时间</th>
                  <th className="border border-slate-200 px-4 py-2 text-[10px] font-black text-slate-500 uppercase tracking-widest leading-none w-[10%]">账号类型</th>
                  <th className="border border-slate-200 px-4 py-2 text-[10px] font-black text-slate-500 uppercase tracking-widest leading-none w-[14%]">当前状态</th>
                  <th className="border border-slate-200 px-4 py-2 text-[10px] font-black text-slate-500 uppercase tracking-widest leading-none text-right w-[10%]">操作</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100">
                {selectedIds.length > 0 && selectedIds.length === accounts.length && total > accounts.length && (
                  <tr className="bg-indigo-50/50 border-b border-indigo-100">
                    <td colSpan={7} className="px-6 py-2 text-center text-xs font-medium text-indigo-700">
                       已选择本页所有条目。
                       <button 
                        onClick={handleSelectAllAcrossPages}
                        className="ml-2 font-black underline hover:text-indigo-900 transition-colors"
                       >
                         选择系统内全部 {total} 个匹配账号
                       </button>
                    </td>
                  </tr>
                )}
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
                    <tr key={account.id} className={`hover:bg-slate-50/50 transition-colors ${selectedIds.includes(account.id) ? 'bg-indigo-50/30' : ''}`}>
                      <td className="border border-slate-100 px-4 py-1.5">
                        <input 
                          type="checkbox" 
                          className="rounded border-slate-300 text-indigo-600 focus:ring-indigo-500"
                          checked={selectedIds.includes(account.id)}
                          onChange={() => toggleSelect(account.id)}
                        />
                      </td>
                      <td className="border border-slate-100 px-4 py-1.5">
                        <div className="flex items-center justify-between gap-2">
                          <div className="flex flex-col min-w-0">
                            <div className="flex items-center gap-2">
                               <span className="text-sm font-black text-slate-800 tracking-tight truncate">{account.address}</span>
                               {(account.access_token || account.session_token) && (
                                 <span title="已捕获 Token">
                                   <Key size={12} className="text-amber-500" />
                                 </span>
                               )}
                            </div>
                            <span className="text-[10px] font-mono text-slate-400 leading-none">ID: {account.id.slice(0, 8)}...</span>
                          </div>
                          <button 
                            onClick={(e) => { e.stopPropagation(); copyToClipboard(account.address); }}
                            className="p-1 rounded-md text-slate-400 hover:text-indigo-600 hover:bg-indigo-50 transition-all shrink-0"
                            title="复制账号地址"
                          >
                            <Copy size={12} />
                          </button>
                        </div>
                      </td>
                      <td className="border border-slate-100 px-4 py-1.5">
                        <div className="flex items-center justify-between gap-2">
                          <div className="flex items-center gap-2 min-w-0">
                            <div className="w-5 h-5 rounded bg-slate-100 flex items-center justify-center text-slate-400 shrink-0">
                              <Lock size={10} />
                            </div>
                            <code className="text-[11px] font-mono text-slate-600 bg-slate-50 px-1.5 py-0.5 rounded truncate">
                              {account.password}
                            </code>
                          </div>
                          <button 
                            onClick={(e) => { e.stopPropagation(); copyToClipboard(account.password); }}
                            className="p-1 rounded-md text-slate-400 hover:text-indigo-600 hover:bg-indigo-50 transition-all shrink-0"
                            title="复制注册密码"
                          >
                            <Copy size={12} />
                          </button>
                        </div>
                      </td>
                      <td className="border border-slate-100 px-4 py-1.5">
                        <div className="flex items-center gap-2 text-slate-800">
                          <Calendar size={12} className="text-slate-600" />
                          <span className="text-[11px] font-mono leading-none font-bold">
                            {new Date(account.created_at * 1000).toLocaleString('zh-CN', { 
                                year: 'numeric',
                                month: '2-digit', 
                                day: '2-digit',
                                hour: '2-digit',
                                minute: '2-digit',
                                second: '2-digit'
                             })}
                          </span>
                        </div>
                      </td>
                      <td className="border border-slate-100 px-4 py-1.5">
                        <span className="px-1.5 py-0.5 rounded bg-slate-100 text-slate-600 text-[9px] font-black uppercase tracking-widest border border-slate-200">
                           {account.account_type || 'FREE'}
                        </span>
                      </td>
                      <td className="border border-slate-100 px-4 py-1.5">
                        <div className="flex items-center">
                           <StatusBadge status={account.status} />
                        </div>
                      </td>
                      <td className="border border-slate-100 px-4 py-1.5 text-right">
                        <div className="flex items-center justify-end gap-1.5">
                           <button 
                            onClick={(e) => { e.stopPropagation(); handleCheckStatus(account.id); }}
                            disabled={checkingIds.includes(account.id)}
                            className={`p-1 rounded-md text-slate-400 hover:text-indigo-600 hover:bg-indigo-50 transition-all ${checkingIds.includes(account.id) ? 'animate-pulse' : ''}`}
                            title="检查账户活跃状态"
                           >
                              <ShieldCheck size={14} className={checkingIds.includes(account.id) ? 'animate-spin' : ''} />
                           </button>
                           <button 
                             onClick={(e) => { e.stopPropagation(); setSelectedAccount(account); }}
                             className="p-1 rounded-md text-slate-400 hover:text-indigo-600 hover:bg-indigo-50 transition-all"
                             title="查看账号详情/Token"
                           >
                             <ExternalLink size={14} />
                           </button>
                           <button 
                            onClick={(e) => { e.stopPropagation(); handleDelete(account.id); }}
                            className="p-1 rounded-md text-slate-400 hover:text-rose-600 hover:bg-rose-50 transition-all"
                           >
                             <Trash2 size={14} />
                           </button>
                        </div>
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>

          <div className="px-6 py-2.5 bg-slate-50/50 border-t border-slate-200 flex items-center justify-between">
            <div className="flex items-center gap-6">
              <div className="text-[10px] font-mono text-slate-400">
                第 <span className="text-slate-700 font-bold">{page}</span> 页 / 共 {Math.ceil(total / pageSize) || 1} 页 
                <span className="mx-2">|</span>
                总记录: {total}
              </div>
              
              <div className="flex items-center gap-2">
                <span className="text-[10px] text-slate-400 font-bold uppercase tracking-tight">每页显示:</span>
                <select 
                  value={pageSize}
                  onChange={(e) => {
                    setPageSize(Number(e.target.value));
                    setPage(1);
                  }}
                  className="bg-white border border-slate-200 rounded-md px-2 py-0.5 text-[10px] font-bold text-slate-600 outline-none focus:border-indigo-500 transition-all cursor-pointer"
                >
                  {[10, 20, 50, 100].map(size => (
                    <option key={size} value={size}>{size} 条</option>
                  ))}
                </select>
              </div>
            </div>

            <div className="flex items-center gap-1.5">
              <button 
                onClick={() => setPage(p => Math.max(1, p - 1))}
                disabled={page <= 1}
                className="p-1 rounded-md border border-slate-200 bg-white text-slate-500 hover:bg-slate-50 disabled:opacity-40 disabled:cursor-not-allowed transition-all"
              >
                <ChevronLeft size={16} />
              </button>
              <span className="text-[10px] font-black text-slate-400 px-2">{page}</span>
              <button 
                onClick={() => setPage(p => p + 1)}
                disabled={page >= Math.ceil(total / pageSize)}
                className="p-1 rounded-md border border-slate-200 bg-white text-slate-500 hover:bg-slate-50 disabled:opacity-40 disabled:cursor-not-allowed transition-all"
              >
                <ChevronRight size={16} />
              </button>
            </div>
      </div>
    </div>
  </div>
      {selectedAccount && (
        <div className="fixed inset-0 z-[1000] flex items-center justify-center bg-slate-900/60 p-4 backdrop-blur-md">
            <div className="bg-white rounded-3xl w-full max-w-2xl max-h-[90vh] flex flex-col shadow-2xl animate-in zoom-in-95 duration-300 overflow-hidden">
                <div className="px-6 py-4 border-b border-slate-100 flex items-center justify-between shrink-0">
                    <div className="flex items-center gap-3">
                        <div className="w-10 h-10 rounded-xl bg-indigo-50 text-indigo-600 flex items-center justify-center">
                            <Key size={20} />
                        </div>
                        <div>
                            <h3 className="text-lg font-black text-slate-900">账号资产密令</h3>
                            <p className="text-[10px] font-mono text-slate-400 uppercase tracking-widest">{selectedAccount.address}</p>
                        </div>
                    </div>
                    <button onClick={() => setSelectedAccount(null)} className="p-2 rounded-xl hover:bg-slate-100 text-slate-400 hover:text-slate-900 transition-all">
                        <X size={20} />
                    </button>
                </div>

                <div className="p-6 overflow-y-auto space-y-6 scrollbar-thin">
                    <SecretField label="Access Token (API)" value={selectedAccount.access_token} onCopy={copyToClipboard} />
                    <SecretField label="Session Token (Web)" value={selectedAccount.session_token} onCopy={copyToClipboard} />
                    <SecretField label="Refresh Token" value={selectedAccount.refresh_token} onCopy={copyToClipboard} />
                    
                    <div className="grid grid-cols-2 gap-4">
                        <div className="p-4 rounded-2xl bg-slate-50 border border-slate-100">
                            <p className="text-[10px] font-black text-slate-400 uppercase tracking-widest mb-1">Device ID</p>
                            <code className="text-[11px] font-mono text-slate-700 break-all">{selectedAccount.device_id}</code>
                        </div>
                        <div className="p-4 rounded-2xl bg-slate-50 border border-slate-100">
                            <p className="text-[10px] font-black text-slate-400 uppercase tracking-widest mb-1">Workspace ID</p>
                            <code className="text-[11px] font-mono text-slate-700 break-all">{selectedAccount.workspace_id || 'N/A'}</code>
                        </div>
                    </div>
                </div>

                <div className="px-6 py-4 bg-slate-50 border-t border-slate-100 flex justify-end shrink-0">
                    <button onClick={() => setSelectedAccount(null)} className="phantom-btn phantom-btn--primary">
                        确认退出预览
                    </button>
                </div>
            </div>
        </div>
      )}
      {/* 复制提醒 Toast */}
      <AnimatePresence>
        {showToast && (
          <motion.div 
            initial={{ opacity: 0, y: -20, x: '-50%', scale: 0.9 }}
            animate={{ opacity: 1, y: '-50%', x: '-50%', scale: 1 }}
            exit={{ opacity: 0, y: -10, x: '-50%', scale: 0.9, transition: { duration: 0.15 } }}
            style={{ left: '50%', top: '50%' }}
            className="fixed z-[1001] px-5 py-2.5 bg-slate-900/95 text-white rounded-2xl shadow-2xl flex items-center gap-3 border border-slate-700/50 backdrop-blur-md"
          >
            <div className="w-5 h-5 rounded-full bg-emerald-500 flex items-center justify-center shrink-0 shadow-[0_0_10px_rgba(16,185,129,0.3)]">
              <CheckCircle2 size={12} className="text-white" />
            </div>
            <span className="text-xs font-black tracking-tight whitespace-nowrap">{toastMsg}</span>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};

function SecretField({ label, value, onCopy }: { label: string; value: string | null | undefined; onCopy: (v: string) => void }) {
  return (
    <div className="space-y-2 group">
      <div className="flex items-center justify-between">
        <span className="text-[10px] font-black text-slate-500 uppercase tracking-widest">{label}</span>
        {value && (
          <button onClick={() => onCopy(value)} className="text-[10px] font-bold text-indigo-600 hover:text-indigo-700 transition-colors opacity-0 group-hover:opacity-100">
            点击复制全部
          </button>
        )}
      </div>
      <div className="relative">
        <textarea
          readOnly
          value={value || '未捕获该类型的 Token 产物'}
          className={`w-full min-h-[80px] rounded-2xl bg-slate-50 border border-slate-200 p-4 text-[11px] font-mono outline-none resize-none transition-all focus:border-indigo-500 focus:bg-white ${!value ? 'text-slate-400 italic' : 'text-slate-700'}`}
        />
        {value && (
          <div className="absolute top-3 right-3">
            <div className="w-8 h-8 rounded-lg bg-indigo-50 text-indigo-600 flex items-center justify-center opacity-40 group-hover:opacity-100 transition-opacity">
              <Lock size={14} />
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function StatItem({ label, value, sub }: { label: string; value: string; sub: string }) {
  return (
    <div className="flex flex-col group">
      <p className="text-[10px] font-mono text-slate-400 uppercase tracking-tighter group-hover:text-indigo-600 transition-colors leading-none mb-1">{label}</p>
      <div className="flex items-baseline gap-1.5">
        <span className="text-sm font-black text-slate-800 leading-none">{value}</span>
        <span className="text-[9px] font-black text-slate-300 uppercase italic tracking-widest">{sub}</span>
      </div>
    </div>
  );
}

function StatusBadge({ status }: { status: string }) {
  const s = status.toLowerCase();
  const isSuccess = s.includes('registered') || s === 'success' || s.includes('active') || s === 'uploaded';
  const isError = s.includes('banned') || s.includes('expired') || s.includes('invalid');
  const isNone = s.includes('no token');
  const isUploaded = s === 'uploaded';

  return (
    <span 
      title={`原始状态: ${status}`}
      className={`flex items-center gap-1.5 px-3 py-1 rounded-full text-[10px] font-black uppercase tracking-tighter border shadow-sm transition-all hover:scale-105 active:scale-95 cursor-default ${
      isUploaded
        ? 'bg-violet-50 text-violet-600 border-violet-100'
        : isSuccess
          ? 'bg-emerald-50 text-emerald-600 border-emerald-100'
          : isError
            ? 'bg-rose-50 text-rose-600 border-rose-100'
            : isNone
              ? 'bg-slate-100 text-slate-500 border-slate-200'
              : 'bg-amber-50 text-amber-600 border-amber-100'
    }`}>
      {isSuccess ? <CheckCircle2 size={10} /> : <AlertCircle size={10} />}
      {isUploaded ? '已同步 (CPA)' : status}
    </span>
  );
}

export default AccountListView;
