import React from 'react';
import { Wallet, Info, Activity } from 'lucide-react';

const AccountSummary = ({ state, isConnected }) => {
    if (!state) return (
        <div className="bg-slate-900/50 backdrop-blur-xl rounded-2xl border border-slate-800 p-8 flex flex-col items-center justify-center min-h-[300px] text-slate-500">
            <Activity className="w-12 h-12 mb-4 animate-pulse" />
            <p className="font-medium animate-pulse">Waiting for data stream...</p>
        </div>
    );

    return (
        <div className="grid gap-6">
            {/* Global Summary */}
            <div className="bg-gradient-to-br from-indigo-600 to-violet-700 rounded-2xl p-6 text-white shadow-xl relative overflow-hidden group">
                <div className="absolute -right-8 -bottom-8 w-48 h-48 bg-white/10 rounded-full blur-3xl group-hover:bg-white/20 transition-colors" />
                <div className="relative z-10">
                    <div className="flex justify-between items-start mb-4">
                        <div className="p-3 bg-white/20 rounded-xl backdrop-blur-md">
                            <Wallet className="w-6 h-6" />
                        </div>
                        <div className={`px-3 py-1 rounded-full text-[10px] font-bold tracking-widest uppercase border ${isConnected ? 'bg-emerald-500/20 text-emerald-300 border-emerald-500/30' : 'bg-rose-500/20 text-rose-300 border-rose-500/30'
                            }`}>
                            {isConnected ? '• Real-time' : '• Offline'}
                        </div>
                    </div>
                    <div className="text-indigo-100 text-sm font-semibold uppercase tracking-wider mb-1">Total Equity (USDT)</div>
                    <div className="text-4xl font-extrabold tracking-tight mb-4 tabular-nums">
                        ${parseFloat(state.total_equity_usdt).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                    </div>
                    <div className="flex gap-4 items-center">
                        <div className="bg-white/10 rounded-lg px-4 py-2 backdrop-blur-md">
                            <div className="text-[10px] text-indigo-200 uppercase font-bold tracking-tighter">Unrealized PnL</div>
                            <div className={`font-mono font-bold ${parseFloat(state.total_unrealized_pnl) >= 0 ? 'text-emerald-300' : 'text-rose-300'}`}>
                                {parseFloat(state.total_unrealized_pnl) >= 0 ? '+' : ''}${parseFloat(state.total_unrealized_pnl).toFixed(2)}
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            {/* Exchange Breakdown */}
            <div className="bg-slate-900/50 backdrop-blur-xl rounded-2xl border border-slate-800 p-6">
                <div className="flex items-center gap-2 mb-6 text-slate-400">
                    <span className="p-1 px-2 rounded-md bg-slate-800 text-[10px] font-bold">INFO</span>
                    <h3 className="text-sm font-bold uppercase tracking-widest">Exchanges</h3>
                </div>
                <div className="grid grid-cols-2 gap-4">
                    {Object.entries(state.exchange_states).map(([ex, s]) => (
                        <div key={ex} className="p-4 rounded-xl bg-slate-800/30 border border-slate-800/50 hover:border-slate-700 transition-colors group">
                            <div className="text-[10px] text-slate-500 uppercase font-bold tracking-tighter mb-1 group-hover:text-indigo-400 transition-colors">{ex}</div>
                            <div className="text-lg font-bold text-white tabular-nums">${parseFloat(s.total_equity).toFixed(0)}</div>
                            <div className="w-full bg-slate-800 h-1.5 rounded-full mt-3 overflow-hidden">
                                <div
                                    className={`h-full transition-all duration-1000 ${parseFloat(s.margin_ratio) > 0.7 ? 'bg-rose-500' : parseFloat(s.margin_ratio) > 0.4 ? 'bg-amber-500' : 'bg-emerald-500'
                                        }`}
                                    style={{ width: `${(parseFloat(s.margin_ratio) * 100).toFixed(0)}%` }}
                                />
                            </div>
                            <div className="flex justify-between mt-1">
                                <span className="text-[9px] text-slate-600 font-bold uppercase">Margin</span>
                                <span className="text-[9px] text-slate-400 font-mono font-bold">{(parseFloat(s.margin_ratio) * 100).toFixed(1)}%</span>
                            </div>
                        </div>
                    ))}
                </div>
            </div>
        </div>
    );
};

export default AccountSummary;
