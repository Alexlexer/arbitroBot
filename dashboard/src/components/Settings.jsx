import React, { useState } from 'react';
import { useRabbitMQ } from '../hooks/useRabbitMQ';
import { Save, Key, Shield, AlertCircle, CheckCircle2, ChevronRight, Settings as SettingsIcon } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

const Settings = () => {
  const { botConfig, sendBotCommand } = useRabbitMQ();
  const [activeExchange, setActiveExchange] = useState('binance');
  const [formData, setFormData] = useState({
    key: '',
    secret: '',
    passphrase: ''
  });
  const [status, setStatus] = useState({ type: '', message: '' });

  const exchanges = [
    { id: 'binance', name: 'Binance', icon: '🔶' },
    { id: 'bybit', name: 'Bybit', icon: '🟡' },
    { id: 'bitget', name: 'Bitget', icon: '🔵' },
    { id: 'mexc', name: 'MEXC', icon: '🟢' },
    { id: 'okx', name: 'OKX', icon: '⚪️' },
    { id: 'kraken', name: 'Kraken', icon: '🐙' },
  ];

  const handleSave = (e) => {
    e.preventDefault();
    if (!formData.key || !formData.secret) {
      setStatus({ type: 'error', message: 'Key and Secret are required' });
      return;
    }

    sendBotCommand({
      type: 'update_api_keys',
      exchange: activeExchange,
      credentials: {
        key: formData.key,
        secret: formData.secret,
        passphrase: formData.passphrase || null
      }
    });

    setStatus({ type: 'success', message: `API Keys for ${activeExchange} updated successfully!` });
    setFormData({ key: '', secret: '', passphrase: '' });
    
    setTimeout(() => setStatus({ type: '', message: '' }), 3000);
  };

  return (
    <div className="p-6 max-w-6xl mx-auto min-h-screen">
      <div className="flex items-center gap-3 mb-8">
        <div className="p-3 bg-blue-500/10 rounded-xl">
          <SettingsIcon className="w-8 h-8 text-blue-400" />
        </div>
        <div>
          <h1 className="text-3xl font-bold text-white tracking-tight">Bot Settings</h1>
          <p className="text-slate-400">Manage API credentials and exchange connectivity</p>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-4 gap-8">
        {/* Sidebar Navigation */}
        <div className="lg:col-span-1 space-y-2">
          <h2 className="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-4 px-2">Exchanges</h2>
          {exchanges.map((ex) => (
            <button
              key={ex.id}
              onClick={() => {
                setActiveExchange(ex.id);
                setFormData({ key: '', secret: '', passphrase: '' });
              }}
              className={`w-full flex items-center justify-between p-4 rounded-xl transition-all duration-200 ${
                activeExchange === ex.id 
                  ? 'bg-blue-600/20 text-blue-300 border border-blue-500/30' 
                  : 'text-slate-400 hover:bg-slate-800/50 border border-transparent'
              }`}
            >
              <div className="flex items-center gap-3">
                <span className="text-xl">{ex.icon}</span>
                <span className="font-medium">{ex.name}</span>
              </div>
              <ChevronRight className={`w-4 h-4 transition-transform ${activeExchange === ex.id ? 'rotate-90 text-blue-400' : ''}`} />
            </button>
          ))}
        </div>

        {/* Main Configuration Area */}
        <div className="lg:col-span-3">
          <motion.div
            key={activeExchange}
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            className="bg-slate-900/50 backdrop-blur-xl border border-slate-800 rounded-3xl p-8"
          >
            <div className="flex items-center justify-between mb-8">
              <div className="flex items-center gap-4">
                <div className="w-12 h-12 rounded-2xl bg-slate-800 flex items-center justify-center text-2xl">
                  {exchanges.find(e => e.id === activeExchange)?.icon}
                </div>
                <div>
                  <h2 className="text-2xl font-bold text-white capitalize">{activeExchange} API Keys</h2>
                  <p className="text-slate-400 text-sm">Configure trading and balance fetching</p>
                </div>
              </div>
              <div className="flex items-center gap-2 px-4 py-2 bg-slate-800/50 rounded-full text-xs text-slate-300 border border-slate-700">
                <Shield className="w-3 h-3 text-green-400" />
                Encrypted Storage
              </div>
            </div>

            <form onSubmit={handleSave} className="space-y-6">
              <div className="space-y-2">
                <label className="text-sm font-medium text-slate-300 block">API Key</label>
                <div className="relative">
                  <Key className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 text-slate-500" />
                  <input
                    type="text"
                    value={formData.key}
                    onChange={(e) => setFormData({ ...formData, key: e.target.value })}
                    className="w-full bg-slate-950/50 border border-slate-800 rounded-xl py-4 pl-12 pr-4 text-white placeholder-slate-600 focus:outline-none focus:ring-2 focus:ring-blue-500/50 focus:border-blue-500 transition-all font-mono"
                    placeholder="Enter your API key..."
                  />
                </div>
              </div>

              <div className="space-y-2">
                <label className="text-sm font-medium text-slate-300 block">API Secret</label>
                <div className="relative">
                  <Shield className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 text-slate-500" />
                  <input
                    type="password"
                    value={formData.secret}
                    onChange={(e) => setFormData({ ...formData, secret: e.target.value })}
                    className="w-full bg-slate-950/50 border border-slate-800 rounded-xl py-4 pl-12 pr-4 text-white placeholder-slate-600 focus:outline-none focus:ring-2 focus:ring-blue-500/50 focus:border-blue-500 transition-all font-mono"
                    placeholder="Enter your API secret..."
                  />
                </div>
              </div>

              {(activeExchange === 'bitget' || activeExchange === 'okx') && (
                <div className="space-y-2">
                  <label className="text-sm font-medium text-slate-300 block">Passphrase</label>
                  <div className="relative">
                    <Key className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 text-slate-500" />
                    <input
                      type="password"
                      value={formData.passphrase}
                      onChange={(e) => setFormData({ ...formData, passphrase: e.target.value })}
                      className="w-full bg-slate-950/50 border border-slate-800 rounded-xl py-4 pl-12 pr-4 text-white placeholder-slate-600 focus:outline-none focus:ring-2 focus:ring-blue-500/50 focus:border-blue-500 transition-all font-mono"
                      placeholder="Enter API passphrase..."
                    />
                  </div>
                </div>
              )}

              <AnimatePresence>
                {status.message && (
                  <motion.div
                    initial={{ opacity: 0, height: 0 }}
                    animate={{ opacity: 1, height: 'auto' }}
                    exit={{ opacity: 0, height: 0 }}
                    className={`flex items-center gap-3 p-4 rounded-xl border ${
                      status.type === 'error' 
                        ? 'bg-red-500/10 border-red-500/20 text-red-400' 
                        : 'bg-green-500/10 border-green-500/20 text-green-400'
                    }`}
                  >
                    {status.type === 'error' ? <AlertCircle className="w-5 h-5" /> : <CheckCircle2 className="w-5 h-5" />}
                    <span className="text-sm font-medium">{status.message}</span>
                  </motion.div>
                )}
              </AnimatePresence>

              <button
                type="submit"
                className="w-full bg-blue-600 hover:bg-blue-500 text-white font-bold py-4 rounded-xl flex items-center justify-center gap-2 transition-all shadow-lg shadow-blue-600/20 active:scale-[0.98]"
              >
                <Save className="w-5 h-5" />
                Save Credentials
              </button>
            </form>

            <div className="mt-8 p-4 bg-slate-950/30 rounded-2xl border border-slate-800/50">
              <h3 className="text-xs font-semibold text-slate-500 uppercase tracking-widest mb-3 px-1">Security Note</h3>
              <ul className="text-xs text-slate-400 space-y-2 list-disc pl-4">
                <li>Credentials are only used for local order execution and balance monitoring.</li>
                <li>Data is stored locally in the bot's configuration file.</li>
                <li>Make sure to enable 'Futures' and 'Read' permissions on your API keys.</li>
                <li>Disable 'Withdrawal' permissions for maximum security.</li>
              </ul>
            </div>
          </motion.div>
        </div>
      </div>
    </div>
  );
};

export default Settings;
