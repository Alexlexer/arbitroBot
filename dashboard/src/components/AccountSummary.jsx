import React from 'react';
import { Wallet, Info, Activity, ArrowUpRight, Shield, Globe } from 'lucide-react';
import { motion } from 'framer-motion';

const AccountSummary = ({ state, isConnected }) => {
    const num = (val) => {
        if (typeof val === 'number') return val;
        if (typeof val === 'string') return parseFloat(val) || 0;
        return 0;
    };

    if (!state) return (
        <div className="glass-morphism rounded-[2rem] p-12 flex flex-col items-center justify-center min-h-[400px] text-slate-500 border-dashed border-2 border-slate-800/50">
            <div className="relative">
                <Activity className="w-16 h-16 mb-6 text-indigo-500/50 animate-pulse" />
                <div className="absolute inset-0 bg-indigo-500/20 blur-2xl rounded-full animate-pulse" />
            </div>
            <p className="font-bold tracking-widest uppercase text-xs animate-pulse text-indigo-400/70">Initializing Data Stream</p>
            <p className="text-[10px] mt-2 opacity-40 uppercase font-bold tracking-tighter">Waiting for RabbitMQ handshake...</p>
        </div>
    );

    const totalEquity = num(state.total_equity_usdt);
    const totalPnl = num(state.total_unrealized_pnl);

    return (
        <div className="flex flex-col gap-8">
            {/* Main Crypto Card */}
            <motion.div
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                className="relative group h-[260px]"
            >
                {/* Visual Flair */}
                <div className="absolute -inset-0.5 bg-gradient-to-r from-indigo-500 to-violet-600 rounded-[2.5rem] blur opacity-20 group-hover:opacity-40 transition duration-1000 group-hover:duration-200"></div>

                <div className="relative h-full bg-slate-900 rounded-[2.5rem] p-8 overflow-hidden border border-slate-800 shadow-2xl flex flex-col justify-between">
                    {/* Background Texture */}
                    <div className="absolute top-0 right-0 w-64 h-64 bg-indigo-600/10 rounded-full blur-[80px] -mr-32 -mt-32 group-hover:bg-indigo-600/20 transition-all duration-700" />
                    <div className="absolute bottom-0 left-0 w-32 h-32 bg-violet-600/10 rounded-full blur-[60px] -ml-16 -mb-16" />

                    <div className="relative z-10 flex justify-between items-start">
                        <div className="flex flex-col gap-1">
                            <span className="text-[10px] font-black uppercase tracking-[0.2em] text-indigo-400/80">Net Worth</span>
                            <div className="flex items-center gap-2">
                                <Globe className="w-3 h-3 text-slate-600" />
                                <span className="text-[10px] font-bold text-slate-500 uppercase tracking-tighter">Global Account State</span>
                            </div>
                        </div>
                        <div className={`flex items-center gap-2 px-4 py-1.5 rounded-full glass border ${isConnected ? 'border-emerald-500/30' : 'border-rose-500/30'}`}>
                            <div className={`w-1.5 h-1.5 rounded-full ${isConnected ? 'bg-emerald-500 animate-pulse shadow-[0_0_8px_rgba(16,185,129,0.8)]' : 'bg-rose-500'}`} />
                            <span className={`text-[10px] font-black uppercase tracking-widest ${isConnected ? 'text-emerald-400' : 'text-rose-400'}`}>
                                {isConnected ? 'Live' : 'Sync Error'}
                            </span>
                        </div>
                    </div>

                    <div className="relative z-10">
                        <div className="flex items-baseline gap-2">
                            <span className="text-2xl font-light text-slate-400">$</span>
                            <h2 className="text-5xl font-black tracking-tighter text-white tabular-nums">
                                {totalEquity.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                            </h2>
                        </div>
                        <div className="mt-4 flex items-center gap-6">
                            <div className="flex flex-col">
                                <span className="text-[9px] font-black uppercase text-slate-500 mb-1">Unrealized PnL</span>
                                <div className={`flex items-center gap-1 font-mono font-bold text-sm ${totalPnl >= 0 ? 'text-emerald-400' : 'text-rose-400'}`}>
                                    {totalPnl >= 0 ? '+' : ''}${totalPnl.toFixed(2)}
                                    <ArrowUpRight className={`w-3 h-3 ${totalPnl < 0 ? 'rotate-90' : ''}`} />
                                </div>
                            </div>
                            <div className="w-[1px] h-8 bg-slate-800/50" />
                            <div className="flex flex-col">
                                <span className="text-[9px] font-black uppercase text-slate-500 mb-1">Safety Index</span>
                                <div className="flex items-center gap-1.5">
                                    <Shield className="w-3.5 h-3.5 text-indigo-500" />
                                    <span className="text-sm font-black text-white">94<span className="text-[10px] text-slate-500">/100</span></span>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </motion.div>

            {/* Exchange Micro-Cards */}
            <div className="space-y-4">
                <div className="flex items-center gap-2 px-2">
                    <div className="h-[1px] flex-1 bg-slate-800/50" />
                    <span className="text-[10px] font-black uppercase tracking-[0.3em] text-slate-600">Liquidity Distribution</span>
                    <div className="h-[1px] flex-1 bg-slate-800/50" />
                </div>

                <div className="grid grid-cols-2 gap-4">
                    {Object.entries(state.exchange_states).map(([ex, s], idx) => {
                        const equity = num(s.total_equity);
                        const marginRatio = num(s.margin_ratio);
                        return (
                            <motion.div
                                key={ex}
                                initial={{ opacity: 0, x: -10 }}
                                animate={{ opacity: 1, x: 0 }}
                                transition={{ delay: idx * 0.1 }}
                                className="glass-morphism rounded-2xl p-4 border border-slate-800/50 hover:border-indigo-500/30 transition-all group relative overflow-hidden"
                            >
                                <div className="absolute top-0 right-0 w-16 h-16 bg-gradient-to-br from-indigo-500/5 to-transparent rounded-full -mr-8 -mt-8" />
                                <div className="relative z-10">
                                    <div className="flex justify-between items-center mb-2">
                                        <span className="text-[10px] font-black text-slate-500 uppercase tracking-tighter group-hover:text-indigo-400 transition-colors">{ex}</span>
                                        <span className="text-[10px] font-mono text-slate-600">{(marginRatio * 100).toFixed(1)}%</span>
                                    </div>
                                    <div className="text-xl font-black text-white tabular-nums">${equity.toFixed(0)}</div>

                                    <div className="mt-3 relative h-1 bg-slate-800/50 rounded-full overflow-hidden">
                                        <motion.div
                                            initial={{ width: 0 }}
                                            animate={{ width: `${(marginRatio * 100).toFixed(0)}%` }}
                                            className={`absolute h-full rounded-full ${marginRatio > 0.7 ? 'bg-rose-500 shadow-[0_0_10px_rgba(244,63,94,0.5)]' :
                                                marginRatio > 0.4 ? 'bg-amber-500 shadow-[0_0_10px_rgba(245,158,11,0.5)]' :
                                                    'bg-emerald-500 shadow-[0_0_10px_rgba(16,185,129,0.5)]'
                                                }`}
                                        />
                                    </div>
                                </div>
                            </motion.div>
                        );
                    })}
                </div>
            </div>
        </div>
    );
};

export default AccountSummary;
