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
    <div className="bg-black/70 backdrop-blur-xl rounded-2xl border border-white/10 p-6 shadow-2xl">
      <div className="flex items-center gap-2 text-white mb-4">
        <AlertTriangle className="w-5 h-5" />
        <h2 className="text-lg font-bold tracking-tight text-white">Listener Watch</h2>
        <span className="text-[10px] bg-white/5 px-2 py-0.5 rounded text-white/60 uppercase font-bold ml-auto border border-white/10">
          {alert ? 'Alert received' : 'Waiting'}
        </span>
      </div>

      {!alert ? (
        <p className="text-white/50 text-sm">No Listener alerts yet. Set Listener WS URL in the Listener tab.</p>
      ) : (
        <div className="space-y-2">
          <div className="text-sm text-white/80">
            <span className="text-white/50">Symbol:</span> <span className="font-mono font-semibold text-white">{alert.symbol}</span>
          </div>
          <div className="text-sm text-white/80">
            <span className="text-white/50">Exchange:</span> <span className="font-mono font-semibold text-white">{alert.exchange}</span>
          </div>
          <div className="text-sm text-white/80">
            <span className="text-white/50">Kind:</span> <span className="font-mono font-semibold text-white">{alert.kind}</span>
          </div>
          <div className="text-xs text-white/40">
            {typeof alert.at === 'number' ? new Date(alert.at).toLocaleString() : ''}
          </div>

          {opportunity ? (
            <div className="mt-4 rounded-xl border border-white/10 p-4 bg-white/5">
              <div className="text-xs text-white/60 uppercase font-bold tracking-wider">Best cross-exchange</div>
              <div className="flex justify-between items-end mt-2 gap-4">
                <div>
                  <div className="text-[11px] text-white/60">Long</div>
                  <div className="text-sm font-bold text-white font-mono">{opportunity.longExchange}</div>
                  <div className="text-sm font-bold text-white font-mono">{formatPrice(opportunity.longPrice)}</div>
                </div>
                <div>
                  <div className="text-[11px] text-white/60 text-right">Short</div>
                  <div className="text-sm font-bold text-white font-mono">{opportunity.shortExchange}</div>
                  <div className="text-sm font-bold text-white font-mono">{formatPrice(opportunity.shortPrice)}</div>
                </div>
              </div>
              <div
                className="mt-3 inline-flex items-center gap-2 rounded-full px-3 py-1 text-[11px] font-mono font-bold border border-white/15 bg-white/5 text-white/80"
              >
                Spread: {Number(opportunity.spread).toFixed(2)}%
              </div>
            </div>
          ) : (
            <p className="text-white/50 text-sm mt-3">Waiting for tickers across exchanges for this symbol.</p>
          )}
        </div>
      )}
    </div>
  );
}

