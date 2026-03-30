import React, { useEffect, useState } from 'react';
import { Settings, CheckCircle2, Circle } from 'lucide-react';

const BotConfig = ({ config, onCommand }) => {
  if (!config) return null;

  const [listenerWsUrlDraft, setListenerWsUrlDraft] = useState(config.listener_ws_url || '');
  useEffect(() => {
    setListenerWsUrlDraft(config.listener_ws_url || '');
  }, [config.listener_ws_url]);

  const handleToggle = (exchange) => {
    onCommand({
      type: 'toggle_exchange',
      exchange: exchange,
      enabled: !config.enabled_exchanges[exchange]
    });
  };

  const handleSpreadChange = (e) => {
    const val = parseFloat(e.target.value);
    if (!isNaN(val)) {
      onCommand({
        type: 'update_spread',
        threshold: val
      });
    }
  };

  const handleDepthChange = (e) => {
    const val = parseFloat(e.target.value);
    if (!isNaN(val)) {
      onCommand({
        type: 'update_depth',
        depth_usdt: val,
      });
    }
  };

  const handleToggleVwap = () => {
    onCommand({
      type: 'update_depth', // pricing mode flag is only in config for now; kept simple to avoid protocol changes
      depth_usdt: config.depth_usdt ?? 50,
    });
    // use_vwap_pricing currently toggled via config.json; UI shows hint only.
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
          <div className="flex justify-between items-center mb-2">
            <label className="text-[10px] font-bold uppercase text-white/60">Min Net Spread %</label>
            <span className="text-xs font-mono font-bold text-white/70">{config.min_spread_threshold}%</span>
          </div>
          <input 
            type="range" 
            min="0" 
            max="50" 
            step="0.1"
            value={config.min_spread_threshold}
            onChange={handleSpreadChange}
            className="w-full h-1.5 bg-white/5 rounded-lg appearance-none cursor-pointer accent-white"
          />
        </div>

        {/* Depth in USDT (VWAP / liquidity) */}
        <div>
          <div className="flex justify-between items-center mb-2">
            <label className="text-[10px] font-bold uppercase text-white/60">Depth (USDT)</label>
            <span className="text-xs font-mono font-bold text-white/70">
              {Math.min(5000, Math.max(100, config.depth_usdt ?? 50))}
            </span>
          </div>
          <input
            type="range"
            min={100}
            max={5000}
            step={100}
            value={Math.min(5000, Math.max(100, config.depth_usdt ?? 50))}
            onChange={handleDepthChange}
            className="w-full h-1.5 bg-white/5 rounded-lg appearance-none cursor-pointer accent-white"
          />
          <p className="mt-1 text-[10px] text-white/60">
            Диапазон 100 – 5000 USDT. VWAP-диагностика и оценка ликвидности.
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

        {/* Exchanges Toggle */}
        <div>
          <label className="text-[10px] font-bold uppercase text-white/60 mb-3 block">Active Exchanges</label>
          <div className="grid grid-cols-2 gap-2">
            {Object.keys(config.enabled_exchanges).map((ex) => (
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
