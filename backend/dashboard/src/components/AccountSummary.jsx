import React from 'react';
import { Wallet, Info, Activity } from 'lucide-react';

const n = (v) => Number(v) || 0;

const AccountSummary = ({ state, isConnected }) => {
    if (!state) return (
        <div className="bg-white/5 rounded-2xl border border-white/10 p-8 flex flex-col items-center justify-center min-h-[300px] text-zinc-600">
            <Activity className="w-10 h-10 mb-4 animate-pulse" />
            <p className="font-medium text-sm animate-pulse">Waiting for data stream...</p>
        </div>
    );

    const totalEquity = n(state.total_equity_usdt);
    const totalPnl = n(state.total_unrealized_pnl);

    return (
        <div className="grid gap-4">
            {/* Global Summary */}
            <div className="bg-white/5 border border-white/10 rounded-2xl p-6 text-white relative overflow-hidden group hover:border-white/20 transition-colors">
                <div className="flex justify-between items-start mb-6">
                    <div className="p-2.5 bg-white/10 rounded-xl">
                        <Wallet className="w-5 h-5" />
                    </div>
                    <div className={`px-2.5 py-1 rounded-full text-[10px] font-bold tracking-widest uppercase border ${
                        isConnected
                            ? 'bg-white/10 text-white border-white/20'
                            : 'bg-white/5 text-zinc-600 border-white/10'
                    }`}>
                        {isConnected ? '• Live' : '• Offline'}
                    </div>
                </div>
                <div className="text-zinc-500 text-[10px] font-bold uppercase tracking-widest mb-1">Total Equity (USDT)</div>
                <div className="text-4xl font-extrabold tracking-tight mb-4 tabular-nums font-mono">
                    ${totalEquity.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                </div>
                <div className="bg-white/5 border border-white/10 rounded-lg px-4 py-2 inline-block">
                    <div className="text-[10px] text-zinc-500 uppercase font-bold tracking-tighter">Unrealized PnL</div>
                    <div className={`font-mono font-bold text-sm ${totalPnl >= 0 ? 'text-emerald-400' : 'text-rose-400'}`}>
                        {totalPnl >= 0 ? '+' : ''}${totalPnl.toFixed(2)}
                    </div>
                </div>
            </div>

            {/* Exchange Breakdown */}
            <div className="bg-white/5 border border-white/10 rounded-2xl p-5">
                <div className="flex items-center gap-2 mb-4 text-zinc-500">
                    <Info className="w-3.5 h-3.5" />
                    <h3 className="text-[10px] font-bold uppercase tracking-widest">Exchanges</h3>
                </div>
                <div className="grid grid-cols-2 gap-3">
                    {Object.entries(state.exchange_states).map(([ex, s]) => {
                        const equity = n(s.total_equity);
                        const marginRatio = n(s.margin_ratio);
                        return (
                            <div key={ex} className="p-3 rounded-xl bg-white/5 border border-white/10 hover:border-white/20 transition-colors">
                                <div className="text-[9px] text-zinc-600 uppercase font-bold tracking-tighter mb-1">{ex}</div>
                                <div className="text-base font-bold text-white tabular-nums font-mono">${equity.toFixed(0)}</div>
                                <div className="w-full bg-white/10 h-1 rounded-full mt-2.5 overflow-hidden">
                                    <div
                                        className={`h-full transition-all duration-1000 ${
                                            marginRatio > 0.7 ? 'bg-rose-500' : marginRatio > 0.4 ? 'bg-zinc-400' : 'bg-white'
                                        }`}
                                        style={{ width: `${Math.min(marginRatio * 100, 100).toFixed(0)}%` }}
                                    />
                                </div>
                                <div className="flex justify-between mt-1">
                                    <span className="text-[9px] text-zinc-700 font-bold uppercase">Margin</span>
                                    <span className="text-[9px] text-zinc-400 font-mono font-bold">{(marginRatio * 100).toFixed(1)}%</span>
                                </div>
                            </div>
                        );
                    })}
                </div>
            </div>
        </div>
    );
};

export default AccountSummary;
