import React, { useEffect, useState } from 'react';
import { Settings, CheckCircle2, Circle, TrendingUp, ShieldOff, X, Plus } from 'lucide-react';

const BotConfig = ({ config, onCommand }) => {
  if (!config) return null;

  const [listenerWsUrlDraft, setListenerWsUrlDraft] = useState(config.listener_ws_url || '');
  const [spreadDraft, setSpreadDraft] = useState(String(config.min_spread_threshold ?? ''));
  const [depthDraft, setDepthDraft] = useState(String(config.depth_usdt ?? ''));
  const [blacklistInput, setBlacklistInput] = useState('');

  useEffect(() => {
    setListenerWsUrlDraft(config.listener_ws_url || '');
  }, [config.listener_ws_url]);

  useEffect(() => {
    setSpreadDraft(String(config.min_spread_threshold ?? ''));
  }, [config.min_spread_threshold]);

  useEffect(() => {
    setDepthDraft(String(config.depth_usdt ?? ''));
  }, [config.depth_usdt]);

  const handleToggle = (exchange) => {
    onCommand({
      type: 'toggle_exchange',
      exchange: exchange,
      enabled: !config.enabled_exchanges[exchange]
    });
  };

  const applySpread = () => {
    const val = parseFloat(spreadDraft);
    if (!isNaN(val) && val >= 0) {
      onCommand({ type: 'update_spread', threshold: val });
    } else {
      setSpreadDraft(String(config.min_spread_threshold ?? ''));
    }
  };

  const applyDepth = () => {
    const val = parseFloat(depthDraft);
    if (!isNaN(val) && val >= 100) {
      onCommand({ type: 'update_depth', depth_usdt: val });
    } else {
      setDepthDraft(String(config.depth_usdt ?? ''));
    }
  };

  return (
    <div className="bg-black/70 backdrop-blur-xl rounded-2xl border border-white/10 p-6">
      <div className="flex items-center gap-3 mb-6">
        <div className="p-2 bg-white/5 rounded-lg">
          <Settings className="w-5 h-5 text-white/70" />
        </div>
        <h2 className="text-sm font-black uppercase tracking-widest text-white">Bot Control</h2>
      </div>

      <div className="space-y-6">
        {/* Spread Threshold */}
        <div>
          <div className="flex items-center gap-2 mb-3">
            <TrendingUp className="w-3.5 h-3.5 text-white/50" />
            <label className="text-[10px] font-bold uppercase tracking-widest text-white/60">
              Min Spread to Trade
            </label>
          </div>
          <div className="flex gap-2">
            <div className="relative flex-1">
              <input
                type="number"
                min="0"
                max="100"
                step="0.1"
                value={spreadDraft}
                onChange={(e) => setSpreadDraft(e.target.value)}
                onBlur={applySpread}
                onKeyDown={(e) => e.key === 'Enter' && applySpread()}
                className="w-full bg-black/40 border border-white/10 rounded-xl py-3 px-4 pr-10 text-white font-mono text-sm focus:outline-none focus:ring-2 focus:ring-white/20 focus:border-white/20 transition-all"
                placeholder="e.g. 0.5"
              />
              <span className="absolute right-3 top-1/2 -translate-y-1/2 text-white/40 text-sm font-bold">%</span>
            </div>
            <button
              onClick={applySpread}
              className="px-4 py-3 bg-white/10 hover:bg-white/15 border border-white/10 rounded-xl text-xs font-bold text-white transition-colors"
            >
              Apply
            </button>
          </div>
          <p className="mt-2 text-[10px] text-white/40 leading-relaxed">
            Bot opens a trade only when the spread between two exchanges exceeds this value after fees.
            Current: <span className="text-white/70 font-mono">{config.min_spread_threshold}%</span>
          </p>
        </div>

        {/* Depth in USDT */}
        <div>
          <label className="text-[10px] font-bold uppercase tracking-widest text-white/60 mb-3 block">
            Order Size (USDT)
          </label>
          <div className="flex gap-2">
            <div className="relative flex-1">
              <input
                type="number"
                min="100"
                step="100"
                value={depthDraft}
                onChange={(e) => setDepthDraft(e.target.value)}
                onBlur={applyDepth}
                onKeyDown={(e) => e.key === 'Enter' && applyDepth()}
                className="w-full bg-black/40 border border-white/10 rounded-xl py-3 px-4 pr-14 text-white font-mono text-sm focus:outline-none focus:ring-2 focus:ring-white/20 focus:border-white/20 transition-all"
                placeholder="e.g. 1000"
              />
              <span className="absolute right-3 top-1/2 -translate-y-1/2 text-white/40 text-xs font-bold">USDT</span>
            </div>
            <button
              onClick={applyDepth}
              className="px-4 py-3 bg-white/10 hover:bg-white/15 border border-white/10 rounded-xl text-xs font-bold text-white transition-colors"
            >
              Apply
            </button>
          </div>
          <p className="mt-2 text-[10px] text-white/40">
            Amount per leg. Current: <span className="text-white/70 font-mono">${config.depth_usdt}</span>
          </p>
        </div>

        {/* Exchanges Toggle */}
        {/* Listener WS URL */}
        <div>
          <label className="text-[10px] font-bold uppercase text-white/60 mb-2 block">
            Listener WS URL (alerts)
          </label>
          <input
            type="text"
            value={listenerWsUrlDraft}
            onChange={(e) => setListenerWsUrlDraft(e.target.value)}
            placeholder="ws://listener-host:8083/ws"
            className="w-full px-4 py-3 bg-slate-800/80 border border-slate-700 rounded-xl text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-white/20 focus:border-white/10"
          />
          <div className="flex items-center gap-3 mt-3">
            <button
              onClick={() => onCommand({ type: 'update_listener_ws_url', url: listenerWsUrlDraft })}
              className="flex-1 py-2.5 bg-white/10 hover:bg-white/5 rounded-xl text-sm font-semibold text-white transition-colors"
            >
              Set Listener URL
            </button>
          </div>
          <p className="mt-2 text-[10px] text-white/60">
            Leave empty to disable listener integration.
          </p>
        </div>

        {/* Symbol Blacklist */}
        <div>
          <div className="flex items-center gap-2 mb-3">
            <ShieldOff className="w-3.5 h-3.5 text-white/50" />
            <label className="text-[10px] font-bold uppercase tracking-widest text-white/60">
              Symbol Blacklist
            </label>
          </div>
          <div className="flex gap-2 mb-2">
            <input
              type="text"
              value={blacklistInput}
              onChange={(e) => setBlacklistInput(e.target.value.toUpperCase())}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && blacklistInput.trim()) {
                  const current = config.symbol_blacklist || [];
                  if (!current.includes(blacklistInput.trim())) {
                    onCommand({ type: 'update_blacklist', symbols: [...current, blacklistInput.trim()] });
                  }
                  setBlacklistInput('');
                }
              }}
              placeholder="e.g. DRIFTUSDT"
              className="flex-1 bg-black/40 border border-white/10 rounded-xl py-2.5 px-4 text-white font-mono text-sm focus:outline-none focus:ring-2 focus:ring-white/20 transition-all placeholder:text-white/20"
            />
            <button
              onClick={() => {
                const sym = blacklistInput.trim();
                if (!sym) return;
                const current = config.symbol_blacklist || [];
                if (!current.includes(sym)) {
                  onCommand({ type: 'update_blacklist', symbols: [...current, sym] });
                }
                setBlacklistInput('');
              }}
              className="px-3 py-2.5 bg-white/10 hover:bg-white/15 border border-white/10 rounded-xl text-white transition-colors"
            >
              <Plus className="w-4 h-4" />
            </button>
          </div>
          <div className="flex flex-wrap gap-1.5">
            {(config.symbol_blacklist || []).map((sym) => (
              <span key={sym} className="flex items-center gap-1 px-2 py-1 bg-red-900/30 border border-red-500/30 rounded-lg text-[10px] font-mono text-red-300">
                {sym}
                <button
                  onClick={() => onCommand({ type: 'update_blacklist', symbols: (config.symbol_blacklist || []).filter(s => s !== sym) })}
                  className="text-red-400 hover:text-red-200 transition-colors"
                >
                  <X className="w-3 h-3" />
                </button>
              </span>
            ))}
            {(config.symbol_blacklist || []).length === 0 && (
              <p className="text-[10px] text-white/30 italic">No symbols blacklisted</p>
            )}
          </div>
        </div>

        {/* Exchanges Toggle */}
        <div>
          <label className="text-[10px] font-bold uppercase text-white/60 mb-3 block">Active Exchanges</label>
          <div className="grid grid-cols-2 gap-2">
            {Object.keys(config.enabled_exchanges || {}).map((ex) => (
              <button
                key={ex}
                onClick={() => handleToggle(ex)}
                className={`flex items-center justify-between p-2 rounded-lg border text-[10px] font-bold transition-all ${
                  config.enabled_exchanges[ex] 
                    ? 'bg-white/5 border-white/10 text-white/70' 
                    : 'bg-white/2 border-white/5 text-white/40 grayscale'
                }`}
              >
                {ex}
                {config.enabled_exchanges[ex] ? <CheckCircle2 className="w-3 h-3" /> : <Circle className="w-3 h-3" />}
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};

export default BotConfig;
