import React from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Zap, TrendingUp, TrendingDown, ArrowRight, BarChart3, Clock } from 'lucide-react';

const ArbitrageMatrix = ({ tickers }) => {
    const num = (val) => {
        if (typeof val === 'number') return val;
        if (typeof val === 'string') return parseFloat(val) || 0;
        return 0;
    };

    const grouped = groupBySymbol(tickers);
    const opportunities = Object.entries(grouped)
        .map(([symbol, exts]) => ({ symbol, ...calculateSpread(exts) }))
        .filter(opt => opt.bestLong && opt.bestShort)
        .sort((a, b) => b.spread - a.spread);

    return (
        <div className="space-y-6">
            <div className="flex items-center justify-between px-2">
                <div className="flex items-center gap-3">
                    <div className="p-2 bg-indigo-500/10 rounded-xl">
                        <BarChart3 className="w-5 h-5 text-indigo-400" />
                    </div>
                    <div>
                        <h2 className="text-xl font-black text-white tracking-tight uppercase">Opportunity Feed</h2>
                        <p className="text-[10px] text-slate-500 font-bold tracking-widest uppercase">Live Spreads Across {Object.keys(tickers).length} Exchange Points</p>
                    </div>
                </div>
                <div className="flex items-center gap-2 glass px-3 py-1 rounded-full border-slate-800">
                    <Clock className="w-3 h-3 text-slate-500" />
                    <span className="text-[9px] font-black text-slate-400 uppercase tracking-tighter">Real-time Pulse</span>
                </div>
            </div>

            <div className="overflow-y-auto pr-2 max-h-[calc(100vh-280px)] scrollbar-thin scrollbar-thumb-slate-800 scrollbar-track-transparent">
                <div className="grid grid-cols-1 md:grid-cols-2 gap-6 pb-6">
                    <AnimatePresence mode="popLayout">
                        {opportunities.slice(0, 4).map((opt, idx) => {
                            const longPrice = opt.bestLong?.asks?.[0]?.[0];
                            const shortPrice = opt.bestShort?.bids?.[0]?.[0];

                            return (
                                <motion.div
                                    key={opt.symbol}
                                    layout
                                    initial={{ opacity: 0, scale: 0.9, y: 20 }}
                                    animate={{ opacity: 1, scale: 1, y: 0 }}
                                    exit={{ opacity: 0, scale: 0.9, y: -20 }}
                                    transition={{ duration: 0.4, delay: idx * 0.05 }}
                                    className="group relative"
                                >
                                    {/* Card Glow */}
                                    <div className={`absolute -inset-0.5 rounded-[2rem] blur opacity-0 group-hover:opacity-20 transition duration-500 ${opt.spread > 0.5 ? 'bg-emerald-500' : 'bg-indigo-500'
                                        }`} />

                                    <div className="relative glass-morphism rounded-[2rem] p-6 overflow-hidden border border-slate-800/50 hover:border-slate-700 transition-all">
                                        {/* Side Accent */}
                                        <div className={`absolute left-0 top-0 bottom-0 w-1.5 ${opt.spread > 0.5 ? 'bg-emerald-500' : 'bg-indigo-500'
                                            }`} />

                                        <div className="flex justify-between items-start mb-6">
                                            <div className="flex items-center gap-3">
                                                <div className="w-12 h-12 rounded-2xl bg-slate-950 flex items-center justify-center border border-slate-800 group-hover:border-indigo-500/50 transition-colors">
                                                    <span className="text-lg font-black text-white">{opt.symbol[0]}</span>
                                                </div>
                                                <div>
                                                    <div className="text-xl font-black text-white tracking-tighter">{opt.symbol}</div>
                                                    <div className="text-[10px] text-slate-500 font-bold uppercase tracking-widest">Market Pair</div>
                                                </div>
                                            </div>
                                            <div className={`px-4 py-2 rounded-2xl font-black font-mono text-lg flex flex-col items-end ${opt.spread > 0 ? 'text-emerald-400 bg-emerald-500/10' : 'text-rose-400 bg-rose-500/10'
                                                }`}>
                                                <div className="flex items-center gap-1">
                                                    {opt.spread > 0 ? <TrendingUp className="w-4 h-4" /> : <TrendingDown className="w-4 h-4" />}
                                                    {opt.spread.toFixed(2)}%
                                                </div>
                                                <span className="text-[9px] uppercase tracking-tighter opacity-50">Gross Spread</span>
                                            </div>
                                        </div>

                                        <div className="grid grid-cols-7 gap-2 items-center">
                                            <div className="col-span-3 bg-slate-950/50 rounded-2xl p-3 border border-slate-900">
                                                <div className="text-[9px] font-black text-slate-500 uppercase mb-1 flex items-center gap-1">
                                                    <div className="w-1 h-1 rounded-full bg-emerald-500" /> BUY AT
                                                </div>
                                                <div className="text-xs font-bold text-slate-300 mb-1 truncate">{opt.bestLong.exchange}</div>
                                                <div className="text-lg font-black text-white font-mono tracking-tighter">${num(longPrice).toFixed(4)}</div>
                                            </div>

                                            <div className="col-span-1 flex justify-center">
                                                <div className="w-8 h-8 rounded-full bg-slate-800/50 flex items-center justify-center">
                                                    <ArrowRight className="w-4 h-4 text-slate-500 group-hover:text-indigo-400 transition-colors" />
                                                </div>
                                            </div>

                                            <div className="col-span-3 bg-slate-950/50 rounded-2xl p-3 border border-slate-900 text-right">
                                                <div className="text-[9px] font-black text-slate-500 uppercase mb-1 flex items-center gap-1 justify-end">
                                                    SELL AT <div className="w-1 h-1 rounded-full bg-rose-500" />
                                                </div>
                                                <div className="text-xs font-bold text-slate-300 mb-1 truncate">{opt.bestShort.exchange}</div>
                                                <div className="text-lg font-black text-white font-mono tracking-tighter">${num(shortPrice).toFixed(4)}</div>
                                            </div>
                                        </div>

                                        {/* Progress Bar Mini */}
                                        <div className="mt-6 flex items-center gap-3">
                                            <Zap className="w-3 h-3 text-amber-500 animate-pulse" />
                                            <div className="flex-1 h-1 bg-slate-800 rounded-full overflow-hidden">
                                                <motion.div
                                                    initial={{ width: 0 }}
                                                    animate={{ width: `${Math.min(100, opt.spread * 100)}%` }}
                                                    className="h-full bg-gradient-to-r from-indigo-500 to-emerald-500"
                                                />
                                            </div>
                                        </div>
                                    </div>
                                </motion.div>
                            );
                        })}
                    </AnimatePresence>
                </div>
            </div>
        </div>
    );
};

const groupBySymbol = (tickers) => {
    if (!tickers) return {};
    return Object.values(tickers).reduce((acc, t) => {
        if (!t || !t.symbol) return acc;
        if (!acc[t.symbol]) acc[t.symbol] = [];
        acc[t.symbol].push(t);
        return acc;
    }, {});
};

const calculateSpread = (exchanges) => {
    const num = (val) => {
        if (typeof val === 'number') return val;
        if (typeof val === 'string') return parseFloat(val) || 0;
        return 0;
    };

    // Filter out exchanges that don't have valid bid/ask data or are muted (like MEXC)
    const validExchanges = exchanges.filter(ex =>
        ex.exchange !== 'MEXC' && // Mute MEXC for now
        ex.asks && ex.asks.length > 0 && Array.isArray(ex.asks[0]) &&
        ex.bids && ex.bids.length > 0 && Array.isArray(ex.bids[0])
    );

    if (validExchanges.length < 2) return { bestLong: null, bestShort: null, spread: 0 };

    try {
        const bestLong = validExchanges.reduce((a, b) => num(a.asks[0][0]) < num(b.asks[0][0]) ? a : b);
        const bestShort = validExchanges.reduce((a, b) => num(b.bids[0][0]) > num(a.bids[0][0]) ? b : a);

        const askPrice = num(bestLong?.asks?.[0]?.[0]);
        const bidPrice = num(bestShort?.bids?.[0]?.[0]);

        if (askPrice === 0) return { bestLong: null, bestShort: null, spread: 0 };

        const spread = ((bidPrice - askPrice) / askPrice) * 100;

        return { bestLong, bestShort, spread };
    } catch (e) {
        console.error("Spread calculation error:", e);
        return { bestLong: null, bestShort: null, spread: 0 };
    }
};

export default ArbitrageMatrix;
