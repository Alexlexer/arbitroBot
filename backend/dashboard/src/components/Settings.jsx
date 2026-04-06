import React, { useState, useMemo, useEffect } from 'react';
import { useSignalR } from '../hooks/useSignalR';
import { Save, Key, Shield, AlertCircle, CheckCircle2, ChevronRight, Settings as SettingsIcon } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

// id = sent to bot (snake_case). configKey = key in botConfig.enabled_exchanges (PascalCase)
// isDex = true means it uses a single EVM private key instead of API key + secret
const ALL_EXCHANGES = [
  { id: 'binance',     configKey: 'Binance',     name: 'Binance',     icon: '🔶', isDex: false },
  { id: 'bybit',       configKey: 'Bybit',       name: 'Bybit',       icon: '🟡', isDex: false },
  { id: 'gate',        configKey: 'Gate',        name: 'Gate',        icon: '🟠', isDex: false },
  { id: 'hyperliquid', configKey: 'Hyperliquid', name: 'Hyperliquid', icon: '🔷', isDex: true  },
  { id: 'aster',       configKey: 'Aster',       name: 'Aster',       icon: '⭐', isDex: true  },
];

const Settings = () => {
  const { botConfig, sendBotCommand } = useSignalR();
  const [activeExchange, setActiveExchange] = useState('binance');
  const [formData, setFormData] = useState({
    key: '',
    secret: '',
    passphrase: ''
  });
  const [status, setStatus] = useState({ type: '', message: '' });

  const activeExchanges = useMemo(() => {
    if (!botConfig?.enabled_exchanges) return ALL_EXCHANGES;
    return ALL_EXCHANGES.filter((ex) => botConfig.enabled_exchanges[ex.configKey] === true);
  }, [botConfig?.enabled_exchanges]);

  useEffect(() => {
    const ids = activeExchanges.map((e) => e.id);
    if (ids.length && !ids.includes(activeExchange)) {
      setActiveExchange(ids[0]);
    }
  }, [activeExchanges, activeExchange]);

  const handleSave = (e) => {
    e.preventDefault();
    const isDex = currentEx?.isDex;
    if (!formData.key || (!isDex && !formData.secret)) {
      setStatus({ type: 'error', message: isDex ? 'Private key is required' : 'Key and Secret are required' });
      return;
    }

    sendBotCommand({
      type: 'update_api_keys',
      exchange: activeExchange,
      credentials: {
        // DEX: private key goes into both key and secret fields
        key: formData.key,
        secret: isDex ? formData.key : formData.secret,
        passphrase: null
      }
    });

    setStatus({ type: 'success', message: `API Keys for ${activeExchange} updated successfully!` });
    setFormData({ key: '', secret: '', passphrase: '' });
    setTimeout(() => setStatus({ type: '', message: '' }), 3000);
  };

  const currentEx = activeExchanges.find((e) => e.id === activeExchange) || ALL_EXCHANGES.find((e) => e.id === activeExchange);

  return (
    <div className="p-6 max-w-6xl mx-auto min-h-screen">
      <div className="flex items-center gap-3 mb-8">
        <div className="p-3 bg-white/5 rounded-xl">
          <SettingsIcon className="w-8 h-8 text-white/70" />
        </div>
        <div>
          <h1 className="text-3xl font-bold text-white tracking-tight">Bot Settings</h1>
          <p className="text-white/60">Manage API credentials and exchange connectivity</p>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-4 gap-8">
        <div className="lg:col-span-1 space-y-2">
          <h2 className="text-xs font-semibold text-white/60 uppercase tracking-wider mb-4 px-2">Active Exchanges</h2>
          {activeExchanges.length === 0 ? (
            <p className="text-white/60 text-sm px-2">Enable at least one exchange in Dashboard → Bot Control.</p>
          ) : (
            activeExchanges.map((ex) => (
              <button
                key={ex.id}
                onClick={() => {
                  setActiveExchange(ex.id);
                  setFormData({ key: '', secret: '', passphrase: '' });
                }}
                className={`w-full flex items-center justify-between p-4 rounded-xl transition-all duration-200 ${
                  activeExchange === ex.id
                    ? 'bg-white/10 text-white/70 border border-white/10'
                    : 'text-white/60 hover:bg-white/5 border border-transparent'
                }`}
              >
                <div className="flex items-center gap-3">
                  <span className="text-xl">{ex.icon}</span>
                  <span className="font-medium">{ex.name}</span>
                </div>
                <ChevronRight className={`w-4 h-4 transition-transform ${activeExchange === ex.id ? 'rotate-90 text-white/70' : ''}`} />
              </button>
            ))
          )}
        </div>

        <div className="lg:col-span-3">
          {activeExchanges.length === 0 ? (
            <div className="bg-black/70 backdrop-blur-xl border border-white/10 rounded-3xl p-8 text-center text-white/60">
              <p>Turn on at least one exchange in the Dashboard (Bot Control) to set API keys here.</p>
            </div>
          ) : (
            <motion.div
              key={activeExchange}
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              className="bg-black/70 backdrop-blur-xl border border-white/10 rounded-3xl p-8"
            >
              <div className="flex items-center justify-between mb-8">
                <div className="flex items-center gap-4">
                  <div className="w-12 h-12 rounded-2xl bg-white/5 flex items-center justify-center text-2xl">
                    {currentEx?.icon}
                  </div>
                  <div>
                    <h2 className="text-2xl font-bold text-white">{currentEx?.name ?? activeExchange} API Keys</h2>
                    <p className="text-white/60 text-sm">Configure trading and balance fetching</p>
                  </div>
                </div>
                <div className="flex items-center gap-2 px-4 py-2 bg-white/5 rounded-full text-xs text-white/60 border border-white/10">
                  <Shield className="w-3 h-3 text-white/60" />
                  Encrypted Storage
                </div>
              </div>

              <form onSubmit={handleSave} className="space-y-6">
                {currentEx?.isDex ? (
                  <div className="space-y-2">
                    <label className="text-sm font-medium text-white/70 block">EVM Private Key</label>
                    <div className="relative">
                      <Key className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 text-white/50" />
                      <input
                        type="password"
                        value={formData.key}
                        onChange={(e) => setFormData({ ...formData, key: e.target.value })}
                        className="w-full bg-slate-950/50 border border-slate-800 rounded-xl py-4 pl-12 pr-4 text-white placeholder-slate-600 focus:outline-none focus:ring-2 focus:ring-white/20 focus:border-white/10 transition-all font-mono"
                        placeholder="0x..."
                      />
                    </div>
                    <p className="text-xs text-white/40">Used to sign orders via EIP-712. Never sent off-device.</p>
                  </div>
                ) : (
                  <>
                    <div className="space-y-2">
                      <label className="text-sm font-medium text-white/70 block">API Key</label>
                      <div className="relative">
                        <Key className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 text-white/50" />
                        <input
                          type="text"
                          value={formData.key}
                          onChange={(e) => setFormData({ ...formData, key: e.target.value })}
                          className="w-full bg-slate-950/50 border border-slate-800 rounded-xl py-4 pl-12 pr-4 text-white placeholder-slate-600 focus:outline-none focus:ring-2 focus:ring-white/20 focus:border-white/10 transition-all font-mono"
                          placeholder="Enter your API key..."
                        />
                      </div>
                    </div>
                    <div className="space-y-2">
                      <label className="text-sm font-medium text-white/70 block">API Secret</label>
                      <div className="relative">
                        <Shield className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 text-white/50" />
                        <input
                          type="password"
                          value={formData.secret}
                          onChange={(e) => setFormData({ ...formData, secret: e.target.value })}
                          className="w-full bg-slate-950/50 border border-slate-800 rounded-xl py-4 pl-12 pr-4 text-white placeholder-slate-600 focus:outline-none focus:ring-2 focus:ring-white/20 focus:border-white/10 transition-all font-mono"
                          placeholder="Enter your API secret..."
                        />
                      </div>
                    </div>
                  </>
                )}


                <AnimatePresence>
                  {status.message && (
                    <motion.div
                      initial={{ opacity: 0, height: 0 }}
                      animate={{ opacity: 1, height: 'auto' }}
                      exit={{ opacity: 0, height: 0 }}
                      className={`flex items-center gap-3 p-4 rounded-xl border ${
                        status.type === 'error'
                          ? 'bg-white/5 border-white/10 text-white/70'
                          : 'bg-white/5 border-white/10 text-white/70'
                      }`}
                    >
                      {status.type === 'error' ? <AlertCircle className="w-5 h-5" /> : <CheckCircle2 className="w-5 h-5" />}
                      <span className="text-sm font-medium">{status.message}</span>
                    </motion.div>
                  )}
                </AnimatePresence>

                <button
                  type="submit"
                  className="w-full bg-white/10 hover:bg-white/5 text-white font-bold py-4 rounded-xl flex items-center justify-center gap-2 transition-all shadow-lg shadow-white/10 active:scale-[0.98]"
                >
                  <Save className="w-5 h-5" />
                  Save Credentials
                </button>
              </form>

              <div className="mt-8 p-4 bg-black/40 rounded-2xl border border-white/10">
                <h3 className="text-xs font-semibold text-white/60 uppercase tracking-widest mb-3 px-1">Security Note</h3>
                <ul className="text-xs text-white/50 space-y-2 list-disc pl-4">
                  <li>Credentials are only used for local order execution and balance monitoring.</li>
                  <li>Data is stored locally in the bot's configuration file.</li>
                  <li>Make sure to enable 'Futures' and 'Read' permissions on your API keys.</li>
                  <li>Disable 'Withdrawal' permissions for maximum security.</li>
                </ul>
              </div>
            </motion.div>
          )}
        </div>
      </div>
    </div>
  );
};

export default Settings;
