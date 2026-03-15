import React, { useState, useMemo } from 'react';
import { Wallet, Info, Activity, ChevronDown } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

const AccountSummary = ({ state, isConnected, tickers, botConfig }) => {
    const [isExchangesExpanded, setIsExchangesExpanded] = useState(true);

    const lastUpdateByExchange = useMemo(() => {
        const m = {};
        Object.values(tickers || {}).forEach((t) => {
            const ex = String(t.exchange).toLowerCase();
            if (!m[ex] || t.timestamp > m[ex]) m[ex] = t.timestamp;
        });
        return m;
    }, [tickers]);

    if (!state) {
        const statusMessage = !isConnected
            ? 'Connecting to broker...'
            : 'Waiting for account data from bot...';
        const hint = !isConnected
            ? 'Check the bot is running; WebSocket uses the same port as the dashboard (/ws).'
            : 'Ensure the bot is running and connected to the same RabbitMQ.';
        return (
            <div className="bg-slate-900/50 backdrop-blur-xl rounded-2xl border border-slate-800 p-8 flex flex-col items-center justify-center min-h-[300px] text-slate-500">
                <Activity className="w-12 h-12 mb-4 animate-pulse" />
                <p className="font-medium animate-pulse">{statusMessage}</p>
                <p className="text-xs text-slate-600 mt-2 max-w-xs text-center">{hint}</p>
            </div>
        );
    }

    const exchanges = botConfig?.enabled_exchanges ? Object.keys(botConfig.enabled_exchanges) : Object.keys(state.exchange_states);
    const now = Date.now();

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

            {/* Exchange Breakdown/Health */}
            <div className="bg-slate-900/50 backdrop-blur-xl rounded-2xl border border-slate-800 p-4 border-b-0 overflow-hidden">
                <button 
                    onClick={() => setIsExchangesExpanded(!isExchangesExpanded)}
                    className="w-full flex items-center justify-between group py-2"
                >
                    <div className="flex items-center gap-2 text-slate-400">
                        <span className="p-1 px-2 rounded-md bg-slate-800 text-[10px] font-bold group-hover:text-indigo-400 transition-colors">INFO</span>
                        <h3 className="text-sm font-bold uppercase tracking-widest group-hover:text-white transition-colors">Exchanges</h3>
                    </div>
                    <motion.div
                        animate={{ rotate: isExchangesExpanded ? 0 : -90 }}
                        className="text-slate-500 group-hover:text-white transition-colors"
                    >
                        <ChevronDown className="w-5 h-5" />
                    </motion.div>
                </button>
                
                <AnimatePresence initial={false}>
                    {isExchangesExpanded && (
                        <motion.div
                            initial={{ height: 0, opacity: 0, marginTop: 0 }}
                            animate={{ height: 'auto', opacity: 1, marginTop: 16 }}
                            exit={{ height: 0, opacity: 0, marginTop: 0 }}
                            transition={{ duration: 0.3, ease: 'easeInOut' }}
                            className="grid grid-cols-1 gap-3 overflow-hidden"
                        >
                            {exchanges.map((ex) => {
                                const s = state.exchange_states[ex];
                                const isEnabled = botConfig?.enabled_exchanges?.[ex] !== false;
                                const exLower = String(ex).toLowerCase();
                                const lastUpdate = lastUpdateByExchange[exLower] ?? 0;
                                const isStale = (now - lastUpdate) > 60000;
                                const isConnected_ex = lastUpdate > 0 && !isStale;
                                // OKX has no ticker feed in the bot (not implemented)
                                const noFeedLabel = ex === 'Okx' ? 'No feed' : 'NO DATA';

                                return (
                                    <div key={ex} className={`p-3 rounded-xl border transition-all ${
                                        isEnabled ? 'bg-slate-800/30 border-slate-800/50' : 'bg-slate-950/20 border-slate-900 opacity-60 grayscale'
                                    }`}>
                                        <div className="flex justify-between items-center mb-2">
                                            <div className="flex items-center gap-2">
                                                <div className={`w-1.5 h-1.5 rounded-full ${isConnected_ex ? 'bg-emerald-500 animate-pulse' : 'bg-rose-500'}`} />
                                                <div className="text-[10px] text-white uppercase font-black tabular-nums">{ex}</div>
                                            </div>
                                            <div className="text-[9px] font-mono font-bold text-slate-500">
                                                {lastUpdate > 0 ? `${((now - lastUpdate)/1000).toFixed(0)}s ago` : noFeedLabel}
                                            </div>
                                        </div>
                                        {s && (
                                            <>
                                                <div className="flex justify-between items-end">
                                                    <div className="text-sm font-bold text-slate-300 tabular-nums">${parseFloat(s.total_equity).toFixed(2)}</div>
                                                    <div className="text-[10px] text-slate-500 font-mono font-bold">M: {(parseFloat(s.margin_ratio) * 100).toFixed(1)}%</div>
                                                </div>
                                                <div className="w-full bg-slate-800 h-1 rounded-full mt-2 overflow-hidden">
                                                    <div
                                                        className={`h-full transition-all duration-1000 ${parseFloat(s.margin_ratio) > 0.7 ? 'bg-rose-500' : parseFloat(s.margin_ratio) > 0.4 ? 'bg-amber-500' : 'bg-emerald-500'
                                                            }`}
                                                        style={{ width: `${(parseFloat(s.margin_ratio) * 100).toFixed(0)}%` }}
                                                    />
                                                </div>
                                            </>
                                        )}
                                    </div>
                                );
                            })}
                        </motion.div>
                    )}
                </AnimatePresence>
            </div>
        </div>
    );
};

export default AccountSummary;
