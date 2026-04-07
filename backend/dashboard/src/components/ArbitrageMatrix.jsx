import React from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Zap, TrendingUp, TrendingDown } from 'lucide-react';
import { getOpportunities, availableDepthUsdt } from '../utils/opportunities';

const ArbitrageMatrix = ({ tickers, botConfig }) => {
    const opportunities = getOpportunities(tickers, botConfig);

    return (
        <div className="bg-white/5 border border-white/10 rounded-2xl p-6">
            <div className="flex items-center justify-between mb-6">
                <div className="flex items-center gap-2">
                    <Zap className="w-4 h-4 text-white fill-white" />
                    <h2 className="text-base font-bold tracking-tight text-white">Live Arbitrage Matrix</h2>
                </div>
                <span className="text-[10px] text-zinc-600 font-mono">
                    {opportunities.length} pairs
                </span>
            </div>

            {opportunities.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-16 text-zinc-700">
                    <Zap className="w-8 h-8 mb-3 opacity-30" />
                    <p className="text-sm font-medium">No opportunities yet</p>
                    <p className="text-xs mt-1 opacity-60">Waiting for ticker data…</p>
                </div>
            ) : (
                <div className="overflow-x-auto">
                    <table className="w-full text-left border-collapse">
                        <thead>
                            <tr className="border-b border-white/10 text-zinc-600 text-[10px] font-bold uppercase tracking-widest">
                                <th className="pb-3 px-3">Symbol</th>
                                <th className="pb-3 px-3">Long</th>
                                <th className="pb-3 px-3">Short</th>
                                <th className="pb-3 px-3 text-right">Depth</th>
                                <th className="pb-3 px-3 text-right">Spread</th>
                            </tr>
                        </thead>
                        <tbody>
                            <AnimatePresence mode="popLayout">
                                {opportunities.map(({ symbol, bestLong, bestShort, spread }) => {
                                  const depth = availableDepthUsdt(bestLong, bestShort, botConfig?.depth_usdt ?? 5000);
                                  const depthLabel = depth >= 1000 ? `$${(depth / 1000).toFixed(1)}k` : `$${depth.toFixed(0)}`;
                                  return (
                                    <motion.tr
                                        key={symbol}
                                        layout
                                        initial={{ opacity: 0 }}
                                        animate={{ opacity: 1 }}
                                        exit={{ opacity: 0 }}
                                        className="border-b border-white/5 hover:bg-white/5 transition-colors group"
                                    >
                                        <td className="py-3 px-3 font-mono font-bold text-white text-sm">
                                            {symbol}
                                        </td>
                                        <td className="py-3 px-3">
                                            <div className="text-[10px] text-zinc-600 uppercase font-bold">{bestLong.exchange}</div>
                                            <div className="text-emerald-400 font-mono text-sm">
                                                ${parseFloat(bestLong.asks[0][0]).toFixed(4)}
                                            </div>
                                        </td>
                                        <td className="py-3 px-3">
                                            <div className="text-[10px] text-zinc-600 uppercase font-bold">{bestShort.exchange}</div>
                                            <div className="text-rose-400 font-mono text-sm">
                                                ${parseFloat(bestShort.bids[0][0]).toFixed(4)}
                                            </div>
                                        </td>
                                        <td className="py-3 px-3 text-right">
                                            <span className={`font-mono text-xs font-bold ${
                                                depth >= (botConfig?.depth_usdt ?? 1000)
                                                    ? 'text-white'
                                                    : 'text-zinc-500'
                                            }`}>
                                                {depthLabel}
                                            </span>
                                        </td>
                                        <td className="py-3 px-3 text-right">
                                            <span className={`inline-flex items-center gap-1 font-mono text-xs font-bold px-2 py-1 rounded-md border ${
                                                spread > 0
                                                    ? 'text-emerald-400 bg-emerald-500/10 border-emerald-500/20'
                                                    : 'text-zinc-400 bg-white/5 border-white/10'
                                            }`}>
                                                {spread > 0 ? <TrendingUp className="w-3 h-3" /> : <TrendingDown className="w-3 h-3" />}
                                                {spread.toFixed(3)}%
                                            </span>
                                        </td>
                                    </motion.tr>
                                  );
                                })}
                            </AnimatePresence>
                        </tbody>
                    </table>
                </div>
            )}
        </div>
    );
};

export default ArbitrageMatrix;
