import React, { useState, useMemo } from 'react';
import { Wallet, Info, Activity, ChevronDown } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

const AccountSummary = ({ state, isConnected, tickers, botConfig }) => {
    const [isExchangesExpanded, setIsExchangesExpanded] = useState(true);

    const lastUpdateByExchange = useMemo(() => {
        const m = {};
        Object.values(tickers || {}).forEach((t) => {
            const ex = (t.exchange != null ? String(t.exchange) : '').toLowerCase();
            if (!ex) return;
            const ts = typeof t.timestamp === 'number' ? t.timestamp : (t.timestamp != null ? parseInt(t.timestamp, 10) : 0);
            if (!m[ex] || ts > m[ex]) m[ex] = ts;
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
            <div className="bg-black/70 backdrop-blur-xl rounded-2xl border border-white/10 p-8 flex flex-col items-center justify-center min-h-[300px] text-white/60">
                <Activity className="w-12 h-12 mb-4 animate-pulse" />
                <p className="font-medium animate-pulse">{statusMessage}</p>
                <p className="text-xs text-white/60 mt-2 max-w-xs text-center">{hint}</p>
            </div>
        );
    }

    const exchanges = botConfig?.enabled_exchanges
        ? Object.keys(botConfig.enabled_exchanges)
        : (state.exchange_states && typeof state.exchange_states === 'object' ? Object.keys(state.exchange_states) : []);
    const now = Date.now();

    return (
        <div className="grid gap-6">
            {/* Global Summary */}
            <div className="bg-gradient-to-br from-white/10 to-white/0 rounded-2xl p-6 text-white shadow-xl relative overflow-hidden group">
                <div className="absolute -right-8 -bottom-8 w-48 h-48 bg-white/10 rounded-full blur-3xl group-hover:bg-white/20 transition-colors" />
                <div className="relative z-10">
                    <div className="flex justify-between items-start mb-4">
                        <div className="p-3 bg-white/20 rounded-xl backdrop-blur-md">
                            <Wallet className="w-6 h-6" />
                        </div>
                        <div className={`px-3 py-1 rounded-full text-[10px] font-bold tracking-widest uppercase border ${isConnected ? 'bg-white/10 text-white/70 border-white/10' : 'bg-white/5 text-white/60 border-white/10'
                            }`}>
                            {isConnected ? '• Real-time' : '• Offline'}
                        </div>
                    </div>
                    <div className="text-white/70 text-sm font-semibold uppercase tracking-wider mb-1">Total Equity (USDT)</div>
                    <div className="text-4xl font-extrabold tracking-tight mb-4 tabular-nums">
                        ${parseFloat(state.total_equity_usdt).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                    </div>
                    <div className="flex gap-4 items-center">
                        <div className="bg-white/10 rounded-lg px-4 py-2 backdrop-blur-md">
                            <div className="text-[10px] text-white/60 uppercase font-bold tracking-tighter">Unrealized PnL</div>
                            <div className={`font-mono font-bold text-white/70`}>
                                {parseFloat(state.total_unrealized_pnl) >= 0 ? '+' : ''}${parseFloat(state.total_unrealized_pnl).toFixed(2)}
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            {/* Exchange Breakdown/Health */}
            <div className="bg-black/70 backdrop-blur-xl rounded-2xl border border-white/10 p-4 border-b-0 overflow-hidden">
                <button 
                    onClick={() => setIsExchangesExpanded(!isExchangesExpanded)}
                    className="w-full flex items-center justify-between group py-2"
                >
                    <div className="flex items-center gap-2 text-white/60">
                        <span className="p-1 px-2 rounded-md bg-white/5 text-[10px] font-bold group-hover:text-white transition-colors">INFO</span>
                        <h3 className="text-sm font-bold uppercase tracking-widest group-hover:text-white transition-colors">Exchanges</h3>
                    </div>
                    <motion.div
                        animate={{ rotate: isExchangesExpanded ? 0 : -90 }}
                        className="text-white/60 group-hover:text-white transition-colors"
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
                                const s = state.exchange_states?.[ex];
                                const exLower = (ex != null ? String(ex) : '').toLowerCase();
                                const configKey = botConfig?.enabled_exchanges && Object.keys(botConfig.enabled_exchanges).find(k => k.toLowerCase() === exLower);
                                const isEnabled = configKey ? botConfig.enabled_exchanges[configKey] !== false : true;
                                const lastUpdate = lastUpdateByExchange[exLower] ?? 0;
                                const isStale = (now - lastUpdate) > 60000;
                                const isConnected_ex = lastUpdate > 0 && !isStale;
                                // OKX has no ticker feed in the bot (not implemented)
                                const noFeedLabel = ex === 'Okx' ? 'No feed' : 'NO DATA';

                                return (
                                    <div key={ex} className={`p-3 rounded-xl border transition-all ${
                                        isEnabled ? 'bg-white/5 border-white/10' : 'bg-white/2 border-white/5 opacity-60 grayscale'
                                    }`}>
                                        <div className="flex justify-between items-center mb-2">
                                            <div className="flex items-center gap-2">
                                                <div className={`w-1.5 h-1.5 rounded-full ${isConnected_ex ? 'bg-white animate-pulse' : 'bg-white/20'}`} />
                                                <div className="text-[10px] text-white uppercase font-black tabular-nums">{ex}</div>
                                            </div>
                                            <div className="text-[9px] font-mono font-bold text-white/50">
                                                {lastUpdate > 0 ? `${((now - lastUpdate)/1000).toFixed(0)}s ago` : noFeedLabel}
                                            </div>
                                        </div>
                                        {s && (
                                            <>
                                                <div className="flex justify-between items-end">
                                                    <div className="text-sm font-bold text-white/70 tabular-nums">${parseFloat(s.total_equity).toFixed(2)}</div>
                                                    <div className="text-[10px] text-white/50 font-mono font-bold">M: {(parseFloat(s.margin_ratio) * 100).toFixed(1)}%</div>
                                                </div>
                                                <div className="w-full bg-white/5 h-1 rounded-full mt-2 overflow-hidden">
                                                    <div
                                                        className="h-full transition-all duration-1000 bg-white/40"
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
