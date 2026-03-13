import React from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Zap, TrendingUp, TrendingDown } from 'lucide-react';

const ArbitrageMatrix = ({ tickers, botConfig }) => {
    const [sortDirection, setSortDirection] = React.useState('desc'); // 'asc' or 'desc'

    const handleSortToggle = () => {
        setSortDirection(prev => prev === 'asc' ? 'desc' : 'asc');
    };

    // Prepare data
    const grouped = groupBySymbol(tickers);
    const opportunities = Object.entries(grouped)
        .map(([symbol, exts]) => {
            const { bestLong, bestShort, spread } = calculateSpread(exts, botConfig);
            return { symbol, bestLong, bestShort, spread, exts };
        })
        .filter(opp => opp.bestLong && opp.bestShort);

    // Sort and Limit to 50
    opportunities.sort((a, b) => {
        return sortDirection === 'asc' ? a.spread - b.spread : b.spread - a.spread;
    });
    const limitedOpportunities = opportunities.slice(0, 50);

    return (
        <div className="bg-slate-900/50 backdrop-blur-xl rounded-2xl border border-slate-800 p-6 shadow-2xl flex flex-col h-full max-h-[calc(100vh-180px)]">
            <div className="flex items-center gap-2 mb-6 text-indigo-400">
                <Zap className="w-5 h-5 fill-indigo-400" />
                <h2 className="text-xl font-bold tracking-tight text-white">Live Arbitrage Matrix</h2>
                <span className="text-[10px] bg-slate-800 px-2 py-0.5 rounded text-slate-500 uppercase font-bold ml-auto">
                    Showing Top 50
                </span>
            </div>

            <div className="overflow-y-auto pr-2 custom-scrollbar flex-1">
                <table className="w-full text-left border-collapse">
                    <thead>
                        <tr className="border-b border-slate-800 text-slate-400 text-sm font-medium">
                            <th className="pb-4 px-4">Symbol</th>
                            <th className="pb-4 px-4">Best Long</th>
                            <th className="pb-4 px-4">Best Short</th>
                            <th className="pb-4 px-4 text-right cursor-pointer select-none group/sort" onClick={handleSortToggle}>
                                <div className="flex items-center justify-end gap-1 group-hover:text-white transition-colors">
                                    Spread %
                                    {sortDirection === 'desc' ? <TrendingDown className="w-3 h-3" /> : <TrendingUp className="w-3 h-3" />}
                                </div>
                            </th>
                        </tr>
                    </thead>
                    <tbody>
                        <AnimatePresence mode="popLayout">
                            {limitedOpportunities.map(({ symbol, bestLong, bestShort, spread }) => (
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
                                            ${parseFloat(bestLong.asks[0][0]).toFixed(4)}
                                        </div>
                                    </td>
                                    <td className="py-4 px-4">
                                        <div className="text-xs text-slate-500 uppercase font-semibold">{bestShort.exchange}</div>
                                        <div className="text-red-400 font-mono text-sm font-medium">
                                            ${parseFloat(bestShort.bids[0][0]).toFixed(4)}
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
                            ))}
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

const calculateSpread = (exchanges, botConfig) => {
    if (exchanges.length < 2) return { bestLong: null, bestShort: null, spread: 0 };

    const now = Date.now();
    // Get valid tickers: non-zero AND fresh (last 30s) AND enabled in config
    const valid = exchanges.filter(e => {
        const ask = parseFloat(e.asks?.[0]?.[0]);
        const bid = parseFloat(e.bids?.[0]?.[0]);
        const isFresh = (now - e.timestamp) < 30000;
        const isEnabled = botConfig?.enabled_exchanges?.[e.exchange] !== false;
        return ask > 0.00000001 && bid > 0.00000001 && isFresh && isEnabled;
    });
    
    if (valid.length < 2) return { bestLong: null, bestShort: null, spread: 0 };

    const bestLong = valid.reduce((a, b) => parseFloat(a.asks[0][0]) < parseFloat(b.asks[0][0]) ? a : b);
    const bestShort = valid.reduce((a, b) => parseFloat(a.bids[0][0]) > parseFloat(b.bids[0][0]) ? a : b);

    const longPrice = parseFloat(bestLong.asks[0][0]);
    const shortPrice = parseFloat(bestShort.bids[0][0]);
    
    // Safety Guard: Relaxed to 11x (1000%) to allow 30%+ spreads
    const ratio = shortPrice > longPrice ? shortPrice / longPrice : longPrice / shortPrice;
    if (ratio > 11.0) return { bestLong: null, bestShort: null, spread: 0 };

    const spread = ((shortPrice - longPrice) / longPrice) * 100;
    
    // Threshold Guard: Only show if it meets the bot's minimum spread
    const threshold = botConfig?.min_spread_threshold || 0;
    if (spread < threshold) return { bestLong: null, bestShort: null, spread: 0 };

    // Final sanity check: no spreads > 100% in UI
    if (Math.abs(spread) > 100) return { bestLong: null, bestShort: null, spread: 0 };

    return { bestLong, bestShort, spread };
};

export default ArbitrageMatrix;
