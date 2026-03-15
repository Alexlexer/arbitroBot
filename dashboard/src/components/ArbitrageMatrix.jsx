import React from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Zap, TrendingUp, TrendingDown } from 'lucide-react';
import { getOpportunities } from '../utils/opportunities';

const formatPrice = (price) => {
    if (price === 0) return '$0.00';
    if (price < 0.0001) return `$${price.toFixed(12)}`;
    if (price < 0.01) return `$${price.toFixed(8)}`;
    if (price < 1) return `$${price.toFixed(6)}`;
    return `$${price.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 4 })}`;
};

/** Row can be live shape { symbol, bestLong, bestShort, spread } or snapshot shape { symbol, longExchange, longPrice, shortExchange, shortPrice, spread }. */
const Row = ({ item, sortDirection }) => {
    const isSnapshot = 'longExchange' in item;
    const symbol = item.symbol;
    const spread = item.spread;
    const longLabel = isSnapshot ? item.longExchange : item.bestLong.exchange;
    const longPrice = isSnapshot ? item.longPrice : parseFloat(item.bestLong.asks[0][0]);
    const shortLabel = isSnapshot ? item.shortExchange : item.bestShort.exchange;
    const shortPrice = isSnapshot ? item.shortPrice : parseFloat(item.bestShort.bids[0][0]);

    return (
        <motion.tr
            layout
            initial={{ opacity: 0, scale: 0.98 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.98 }}
            className="border-b border-slate-800/50 hover:bg-white/5 transition-colors group"
        >
            <td className="py-4 px-4 font-mono font-bold text-white group-hover:text-indigo-400 transition-colors">{symbol}</td>
            <td className="py-4 px-4">
                <div className="text-xs text-slate-500 uppercase font-semibold">{longLabel}</div>
                <div className="text-green-400 font-mono text-sm font-medium">{formatPrice(longPrice)}</div>
            </td>
            <td className="py-4 px-4">
                <div className="text-xs text-slate-500 uppercase font-semibold">{shortLabel}</div>
                <div className="text-red-400 font-mono text-sm font-medium">{formatPrice(shortPrice)}</div>
            </td>
            <td className="py-4 px-4 text-right">
                <div className={`inline-flex items-center gap-1 rounded-full px-3 py-1 font-mono text-sm font-bold ${spread > 0 ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20' : 'bg-rose-500/10 text-rose-400 border border-rose-500/20'}`}>
                    {spread > 0 ? <TrendingUp className="w-3 h-3" /> : <TrendingDown className="w-3 h-3" />}
                    {spread.toFixed(2)}%
                </div>
            </td>
        </motion.tr>
    );
};

const ArbitrageMatrix = ({ tickers, botConfig, snapshotOpportunities, snapshotLabel }) => {
    const [sortDirection, setSortDirection] = React.useState('desc');

    const handleSortToggle = () => {
        setSortDirection(prev => prev === 'asc' ? 'desc' : 'asc');
    };

    const limitedOpportunities = React.useMemo(() => {
        if (snapshotOpportunities && snapshotOpportunities.length > 0) {
            const sorted = [...snapshotOpportunities].sort((a, b) => sortDirection === 'asc' ? a.spread - b.spread : b.spread - a.spread);
            return sorted;
        }
        const opportunities = getOpportunities(tickers || {}, botConfig, 50);
        opportunities.sort((a, b) => sortDirection === 'asc' ? a.spread - b.spread : b.spread - a.spread);
        return opportunities;
    }, [tickers, botConfig, snapshotOpportunities, sortDirection]);

    const title = snapshotLabel != null ? snapshotLabel : 'Live Arbitrage Matrix';
    const count = limitedOpportunities.length;

    return (
        <div className="bg-slate-900/50 backdrop-blur-xl rounded-2xl border border-slate-800 p-6 shadow-2xl flex flex-col h-full max-h-[calc(100vh-180px)]">
            <div className="flex items-center gap-2 mb-6 text-indigo-400">
                <Zap className="w-5 h-5 fill-indigo-400" />
                <h2 className="text-xl font-bold tracking-tight text-white">{title}</h2>
                <span className="text-[10px] bg-slate-800 px-2 py-0.5 rounded text-slate-500 uppercase font-bold ml-auto">
                    {count} row{count !== 1 ? 's' : ''}
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
                            {limitedOpportunities.map((opp, idx) => (
                                <Row key={opp.symbol + (opp.timestamp ?? idx)} item={opp} sortDirection={sortDirection} />
                            ))}
                        </AnimatePresence>
                    </tbody>
                </table>
            </div>
        </div>
    );
};

export default ArbitrageMatrix;
