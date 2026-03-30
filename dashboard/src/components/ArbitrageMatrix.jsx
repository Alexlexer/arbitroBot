import React from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Zap, TrendingUp, TrendingDown } from 'lucide-react';

const ArbitrageMatrix = ({ tickers }) => {
    // Group tickers by symbol
    const symbols = [...new Set(Object.values(tickers).map(t => t.symbol))];

    return (
        <div className="bg-slate-900/50 backdrop-blur-xl rounded-2xl border border-slate-800 p-6 shadow-2xl">
            <div className="flex items-center gap-2 mb-6 text-indigo-400">
                <Zap className="w-5 h-5 fill-indigo-400" />
                <h2 className="text-xl font-bold tracking-tight text-white">Live Arbitrage Matrix</h2>
            </div>

            <div className="overflow-x-auto">
                <table className="w-full text-left border-collapse">
                    <thead>
                        <tr className="border-b border-slate-800 text-slate-400 text-sm font-medium">
                            <th className="pb-4 px-4">Symbol</th>
                            <th className="pb-4 px-4">Best Long</th>
                            <th className="pb-4 px-4">Best Short</th>
                            <th className="pb-4 px-4 text-right">Spread %</th>
                        </tr>
                    </thead>
                    <tbody>
                        <AnimatePresence mode="popLayout">
                            {Object.entries(groupBySymbol(tickers)).map(([symbol, exts]) => {
                                const { bestLong, bestShort, spread } = calculateSpread(exts);
                                if (!bestLong || !bestShort) return null;

                                return (
                                    <motion.tr
                                        key={symbol}
                                        layout
                                        initial={{ opacity: 0, scale: 0.98 }}
                                        animate={{ opacity: 1, scale: 1 }}
                                        exit={{ opacity: 0, scale: 0.98 }}
                                        className="border-b border-slate-800/50 hover:bg-white/5 transition-colors group"
                                    >
                                        <td className="py-4 px-4 font-mono font-bold text-white group-hover:text-indigo-400 transition-colors">
                                            {symbol}
                                        </td>
                                        <td className="py-4 px-4">
                                            <div className="text-xs text-slate-500 uppercase font-semibold">{bestLong.exchange}</div>
                                            <div className="text-green-400 font-mono text-sm font-medium">
                                                ${bestLong.best_ask[0].toFixed(4)}
                                            </div>
                                        </td>
                                        <td className="py-4 px-4">
                                            <div className="text-xs text-slate-500 uppercase font-semibold">{bestShort.exchange}</div>
                                            <div className="text-red-400 font-mono text-sm font-medium">
                                                ${bestShort.best_bid[0].toFixed(4)}
                                            </div>
                                        </td>
                                        <td className="py-4 px-4 text-right">
                                            <div className={`inline-flex items-center gap-1 rounded-full px-3 py-1 font-mono text-sm font-bold ${spread > 0 ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20' : 'bg-rose-500/10 text-rose-400 border border-rose-500/20'
                                                }`}>
                                                {spread > 0 ? <TrendingUp className="w-3 h-3" /> : <TrendingDown className="w-3 h-3" />}
                                                {spread.toFixed(2)}%
                                            </div>
                                        </td>
                                    </motion.tr>
                                );
                            })}
                        </AnimatePresence>
                    </tbody>
                </table>
            </div>
        </div>
    );
};

const groupBySymbol = (tickers) => {
    return Object.values(tickers).reduce((acc, t) => {
        if (!acc[t.symbol]) acc[t.symbol] = [];
        acc[t.symbol].push(t);
        return acc;
    }, {});
};

const calculateSpread = (exchanges) => {
    if (exchanges.length < 2) return { bestLong: null, bestShort: null, spread: 0 };

    const bestLong = exchanges.reduce((a, b) => a.best_ask[0] < b.best_ask[0] ? a : b);
    const bestShort = exchanges.reduce((a, b) => a.best_bid[0] > b.best_bid[0] ? a : b);

    const spread = ((bestShort.best_bid[0] - bestLong.best_ask[0]) / bestLong.best_ask[0]) * 100;

    return { bestLong, bestShort, spread };
};

export default ArbitrageMatrix;
