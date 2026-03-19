import React, { useEffect, useState } from 'react';

export default function ListenerSettings({ config, onCommand }) {
  const [listenerWsUrlDraft, setListenerWsUrlDraft] = useState('');

  useEffect(() => {
    setListenerWsUrlDraft(config?.listener_ws_url || '');
  }, [config?.listener_ws_url]);

  return (
    <div className="bg-black/70 border border-white/10 rounded-2xl p-6">
      <div className="flex items-center justify-between gap-3 mb-4">
        <div>
          <div className="text-sm font-bold text-white tracking-tight">Listener settings</div>
          <div className="text-xs text-white/60">URL for incoming listing alerts (no token)</div>
        </div>
        <div className="text-[10px] px-2 py-1 rounded-full bg-white/5 text-white/70 border border-white/10">
          WS client
        </div>
      </div>

      <label className="text-[10px] font-bold uppercase text-white/60 mb-2 block">
        Listener WS URL
      </label>
      <input
        type="text"
        value={listenerWsUrlDraft}
        onChange={(e) => setListenerWsUrlDraft(e.target.value)}
        placeholder="ws://listener-host:8083/ws"
        className="w-full px-4 py-3 bg-black/40 border border-white/10 rounded-xl text-white placeholder-white/30 focus:outline-none focus:ring-2 focus:ring-white/20"
      />

      <div className="flex items-center gap-3 mt-3">
        <button
          onClick={() => onCommand({ type: 'update_listener_ws_url', url: listenerWsUrlDraft })}
          className="flex-1 py-2.5 bg-white text-black hover:bg-white/90 rounded-xl text-sm font-semibold transition-colors"
        >
          Set URL
        </button>
      </div>

      <div className="mt-2 text-[10px] text-white/50">
        Leave empty to disable listener integration.
      </div>
    </div>
  );
}

