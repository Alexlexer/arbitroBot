import React, { useState } from 'react';
import { useRabbitMQ } from './hooks/useRabbitMQ';
import ArbitrageMatrix from './components/ArbitrageMatrix';
import AccountSummary from './components/AccountSummary';
import BotConfig from './components/BotConfig';
import Settings from './components/Settings';
import { LayoutDashboard, Settings as SettingsIcon, Zap, ShieldCheck, Bot, Terminal, Cpu } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

function App() {
  const { tickers, accountState, botConfig, isConnected, sendBotCommand } = useRabbitMQ();
  const [activeTab, setActiveTab] = useState('dashboard');

  return (
    <div className="min-h-screen bg-[#020617] text-slate-200 font-sans selection:bg-indigo-500/30">
      {/* Premium Top Navigation */}
      <nav className="sticky top-0 z-50 bg-[#020617]/80 backdrop-blur-xl border-b border-slate-900/50">
        <div className="max-w-[1600px] mx-auto px-6 h-20 flex items-center justify-between">
          <div className="flex items-center gap-8">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 bg-gradient-to-tr from-indigo-600 to-violet-500 rounded-xl flex items-center justify-center shadow-lg shadow-indigo-500/20">
                <Zap className="w-6 h-6 text-white fill-current" />
              </div>
              <div>
                <span className="text-xl font-bold text-white tracking-tight">Arbitro<span className="text-indigo-500">Bot</span></span>
                <div className="flex items-center gap-2 mt-0.5">
                  <div className={`w-1.5 h-1.5 rounded-full ${isConnected ? 'bg-green-500 animate-pulse' : 'bg-red-500'}`} />
                  <span className="text-[10px] text-slate-500 font-semibold uppercase tracking-widest">{isConnected ? 'System Live' : 'Connecting...'}</span>
                </div>
              </div>
            </div>

            <div className="h-8 w-px bg-slate-800" />

            <div className="flex items-center gap-1">
              <button
                onClick={() => setActiveTab('dashboard')}
                className={`flex items-center gap-2 px-6 py-2.5 rounded-xl text-sm font-semibold transition-all ${
                  activeTab === 'dashboard' 
                    ? 'bg-indigo-600/10 text-indigo-400 border border-indigo-500/20' 
                    : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800/50'
                }`}
              >
                <LayoutDashboard className="w-4 h-4" />
                Dashboard
              </button>
              <button
                onClick={() => setActiveTab('settings')}
                className={`flex items-center gap-2 px-6 py-2.5 rounded-xl text-sm font-semibold transition-all ${
                  activeTab === 'settings' 
                    ? 'bg-indigo-600/10 text-indigo-400 border border-indigo-500/20' 
                    : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800/50'
                }`}
              >
                <SettingsIcon className="w-4 h-4" />
                Settings
              </button>
            </div>
          </div>

          <div className="flex items-center gap-4">
            <div className="hidden xl:flex items-center gap-2 px-4 py-2 bg-slate-900/50 border border-slate-800 rounded-full">
              <ShieldCheck className="w-4 h-4 text-green-500" />
              <span className="text-xs font-medium text-slate-400">Risk Manager <span className="text-green-500 uppercase">Active</span></span>
            </div>
          </div>
        </div>
      </nav>

      {/* Main Content Area */}
      <main className="max-w-[1600px] mx-auto pt-8 pb-12 px-6">
        <AnimatePresence mode="wait">
          {activeTab === 'dashboard' ? (
            <motion.div
              key="dashboard"
              initial={{ opacity: 0, scale: 0.98 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.98 }}
              transition={{ duration: 0.2 }}
              className="grid grid-cols-1 xl:grid-cols-12 gap-8"
            >
              <div className="xl:col-span-8 space-y-8">
                <ArbitrageMatrix tickers={tickers} botConfig={botConfig} />
              </div>
              <div className="xl:col-span-4 space-y-8">
                <AccountSummary state={accountState} isConnected={isConnected} tickers={tickers} botConfig={botConfig} />
                <BotConfig config={botConfig} onCommand={sendBotCommand} />
              </div>
            </motion.div>
          ) : (
            <motion.div
              key="settings"
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: 20 }}
              transition={{ duration: 0.2 }}
            >
              <Settings />
            </motion.div>
          )}
        </AnimatePresence>
      </main>

      {/* Decorative Background Elements */}
      <div className="fixed inset-0 pointer-events-none -z-10 overflow-hidden">
        <div className="absolute top-0 right-0 w-[500px] h-[500px] bg-indigo-600/5 blur-[120px] rounded-full" />
        <div className="absolute bottom-0 left-0 w-[500px] h-[500px] bg-blue-600/5 blur-[120px] rounded-full" />
      </div>
    </div>
  );
}

export default App;
