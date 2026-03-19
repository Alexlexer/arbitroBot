import React from 'react';
import { AlertTriangle } from 'lucide-react';

const formatPrice = (price) => {
  const p = Number(price);
  if (!Number.isFinite(p)) return '$0.00';
  if (p === 0) return '$0.00';
  if (Math.abs(p) < 0.0001) return `$${p.toFixed(12)}`;
  if (Math.abs(p) < 0.01) return `$${p.toFixed(8)}`;
  if (Math.abs(p) < 1) return `$${p.toFixed(6)}`;
  return `$${p.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 4 })}`;
};

export default function ListenerWatch({ alert, opportunity }) {
  return (
    <div className="bg-slate-900/50 backdrop-blur-xl rounded-2xl border border-slate-800 p-6 shadow-2xl">
      <div className="flex items-center gap-2 text-indigo-400 mb-4">
        <AlertTriangle className="w-5 h-5" />
        <h2 className="text-lg font-bold tracking-tight text-white">Listener Watch</h2>
        <span className="text-[10px] bg-slate-800 px-2 py-0.5 rounded text-slate-500 uppercase font-bold ml-auto">
          {alert ? 'Alert received' : 'Waiting'}
        </span>
      </div>

      {!alert ? (
        <p className="text-slate-500 text-sm">No Listener alerts yet. Configure Listener WS URL in Bot Control.</p>
      ) : (
        <div className="space-y-2">
          <div className="text-sm text-slate-300">
            <span className="text-slate-500">Symbol:</span> <span className="font-mono font-semibold text-white">{alert.symbol}</span>
          </div>
          <div className="text-sm text-slate-300">
            <span className="text-slate-500">Exchange:</span> <span className="font-mono font-semibold text-white">{alert.exchange}</span>
          </div>
          <div className="text-sm text-slate-300">
            <span className="text-slate-500">Kind:</span> <span className="font-mono font-semibold text-white">{alert.kind}</span>
          </div>
          <div className="text-xs text-slate-500">
            {typeof alert.at === 'number' ? new Date(alert.at).toLocaleString() : ''}
          </div>

          {opportunity ? (
            <div className="mt-4 rounded-xl border border-slate-800 p-4 bg-slate-800/20">
              <div className="text-xs text-slate-500 uppercase font-bold tracking-wider">Best cross-exchange</div>
              <div className="flex justify-between items-end mt-2 gap-4">
                <div>
                  <div className="text-[11px] text-slate-500">Long</div>
                  <div className="text-sm font-bold text-emerald-300 font-mono">{opportunity.longExchange}</div>
                  <div className="text-sm font-bold text-emerald-300 font-mono">{formatPrice(opportunity.longPrice)}</div>
                </div>
                <div>
                  <div className="text-[11px] text-slate-500 text-right">Short</div>
                  <div className="text-sm font-bold text-rose-300 font-mono">{opportunity.shortExchange}</div>
                  <div className="text-sm font-bold text-rose-300 font-mono">{formatPrice(opportunity.shortPrice)}</div>
                </div>
              </div>
              <div
                className={`mt-3 inline-flex items-center gap-2 rounded-full px-3 py-1 text-[11px] font-mono font-bold border ${
                  opportunity.spread >= 0
                    ? 'bg-emerald-500/10 text-emerald-300 border-emerald-500/20'
                    : 'bg-rose-500/10 text-rose-300 border-rose-500/20'
                }`}
              >
                Spread: {Number(opportunity.spread).toFixed(2)}%
              </div>
            </div>
          ) : (
            <p className="text-slate-500 text-sm mt-3">Waiting for tickers across exchanges for this symbol.</p>
          )}
        </div>
      )}
    </div>
  );
}

