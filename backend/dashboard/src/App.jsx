import React, { useState, useEffect } from 'react';
import { useRabbitMQ } from './hooks/useRabbitMQ';
import ArbitrageMatrix from './components/ArbitrageMatrix';
import AccountSummary from './components/AccountSummary';
import { Bot, Terminal, ShieldCheck, Cpu } from 'lucide-react';

function App() {
  const { tickers, accountState, isConnected } = useRabbitMQ();

  return (
    <div className="min-h-screen bg-slate-950 text-slate-200 selection:bg-indigo-500 selection:text-white pb-20">
      {/* Dynamic Background */}
      <div className="fixed inset-0 pointer-events-none overflow-hidden">
        <div className="absolute -top-24 -left-24 w-96 h-96 bg-indigo-600/10 rounded-full blur-[120px]" />
        <div className="absolute top-1/2 -right-48 w-80 h-80 bg-violet-600/10 rounded-full blur-[100px]" />
      </div>

      {/* Header */}
      <header className="sticky top-0 z-50 bg-slate-950/80 backdrop-blur-md border-b border-slate-900 px-6 py-4">
        <div className="max-w-7xl mx-auto flex justify-between items-center">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 bg-indigo-600 rounded-xl flex items-center justify-center shadow-lg shadow-indigo-500/20">
              <Bot className="text-white w-6 h-6" />
            </div>
            <div>
              <h1 className="text-xl font-bold tracking-tight text-white leading-none">ArbitroBot <span className="text-indigo-500 text-xs font-black uppercase ml-1">v2</span></h1>
              <p className="text-[10px] text-slate-500 font-mono mt-1 flex items-center gap-1">
                <Cpu className="w-2.5 h-2.5" /> HIGH-FREQUENCY ARBITRAGE HUB
              </p>
            </div>
          </motion.div>

          <div className="flex items-center gap-10">
            <nav className="hidden xl:flex items-center gap-8 text-[10px] font-black uppercase tracking-widest">
              <button
                onClick={() => setActiveView('dashboard')}
                className={`flex items-center gap-2 transition-all hover:text-indigo-400 ${activeView === 'dashboard' ? 'text-indigo-400 scale-110' : 'text-slate-500'}`}
              >
                <LayoutGrid className="w-3.5 h-3.5" /> Dashboard
              </button>
              <button
                onClick={() => setActiveView('analytics')}
                className={`flex items-center gap-2 transition-all hover:text-indigo-400 ${activeView === 'analytics' ? 'text-indigo-400 scale-110' : 'text-slate-500'}`}
              >
                <Activity className="w-3.5 h-3.5" /> Analytics
              </button>
              <button
                onClick={() => setActiveView('terminal')}
                className={`flex items-center gap-2 transition-all hover:text-indigo-400 ${activeView === 'terminal' ? 'text-indigo-400 scale-110' : 'text-slate-500'}`}
              >
                <Command className="w-3.5 h-3.5" /> Terminal
              </button>
            </nav>

            <div className="h-8 w-[1px] bg-white/5 hidden xl:block" />

            <div className="flex items-center gap-6">
              <div className="hidden md:flex flex-col items-end gap-1">
                <div className="flex items-center gap-1.5 text-[9px] font-black uppercase tracking-widest text-emerald-500/80">
                  <ShieldCheck className="w-3 h-3" /> SECURE_NODE_01
                </div>
                <div className="text-[8px] font-mono text-slate-600 font-bold">LATENCY: 12ms</div>
              </div>

              <div className="flex items-center gap-6">
                <div className="hidden md:flex gap-4 items-center text-[10px] font-black uppercase tracking-widest text-slate-500">
                  <div className="flex items-center gap-1.5"><ShieldCheck className="w-3 h-3 text-emerald-500" /> Secure Node</div>
                  <div className="flex items-center gap-1.5"><Terminal className="w-3 h-3 text-indigo-500" /> System: OK</div>
                </div>
                <div className={`h-2.5 w-2.5 rounded-full ring-4 ${isConnected ? 'bg-emerald-500 ring-emerald-500/20 animate-pulse' : 'bg-rose-500 ring-rose-500/20'}`} />
              </div>
            </div>
          </nav>

          <main className="max-w-7xl mx-auto px-6 mt-8 grid grid-cols-1 lg:grid-cols-12 gap-8 relative z-10">
            {/* Left Column: Account Summary */}
            <aside className="lg:col-span-4 flex flex-col gap-8">
              <AccountSummary state={accountState} isConnected={isConnected} />
            </aside>

            {/* Right Column: Main Matrix */}
            <section className="lg:col-span-8">
              <ArbitrageMatrix tickers={tickers} />
            </section>
          </main>

          {/* Navigation Footer for Mobile */}
          <footer className="fixed bottom-0 left-0 right-0 bg-slate-950/80 backdrop-blur-md border-t border-slate-900 px-6 py-4 md:hidden">
            {/* Placeholder for mobile navigation */}
          </footer>
        </div>
        );
}

        export default App;

const AnalyticsView = () => (
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          className="grid grid-cols-1 lg:grid-cols-2 gap-10"
        >
          <div className="glass-morphism rounded-[2.5rem] p-10 border border-slate-800/50 min-h-[400px]">
            <div className="flex items-center gap-3 mb-8">
              <div className="p-3 bg-indigo-500/10 rounded-2xl border border-indigo-500/20">
                <Activity className="w-5 h-5 text-indigo-400" />
              </div>
              <h3 className="text-xl font-black uppercase tracking-tighter text-white">Profit Dynamics</h3>
            </div>
            <div className="bg-slate-950/50 rounded-3xl h-64 border border-slate-800/50 flex flex-col items-center justify-center text-slate-600 font-bold uppercase tracking-widest text-[10px] gap-4">
              <div className="w-12 h-12 border-4 border-indigo-500/20 border-t-indigo-500 rounded-full animate-spin" />
              Chart Engine Initializing...
            </div>
          </div>
          <div className="glass-morphism rounded-[2.5rem] p-10 border border-slate-800/50 min-h-[400px]">
            <div className="flex items-center gap-3 mb-8">
              <div className="p-3 bg-fuchsia-500/10 rounded-2xl border border-fuchsia-500/20">
                <Globe className="w-5 h-5 text-fuchsia-400" />
              </div>
              <h3 className="text-xl font-black uppercase tracking-tighter text-white">Global Volume</h3>
            </div>
            <div className="bg-slate-950/50 rounded-3xl h-64 border border-slate-800/50 flex flex-col items-center justify-center text-slate-600 font-bold uppercase tracking-widest text-[10px] gap-4">
              <div className="relative w-16 h-16">
                <Activity className="w-16 h-16 text-fuchsia-500/20 animate-pulse" />
              </div>
              Volume Heatmap Standby
            </div>
          </div>
        </motion.div>
        );

const TerminalView = () => (
        <motion.div
          initial={{ opacity: 0, scale: 0.98 }}
          animate={{ opacity: 1, scale: 1 }}
          className="glass-morphism rounded-[2.5rem] p-8 border border-slate-800/50 min-h-[600px] font-mono relative overflow-hidden"
        >
          <div className="absolute top-0 left-0 w-full h-1 bg-gradient-to-r from-transparent via-indigo-500/20 to-transparent" />

          <div className="flex items-center justify-between mb-8 pb-4 border-b border-white/5">
            <div className="flex items-center gap-3">
              <div className="w-3 h-3 rounded-full bg-rose-500 shadow-[0_0_10px_rgba(244,63,94,0.5)]" />
              <div className="w-3 h-3 rounded-full bg-amber-500" />
              <div className="w-3 h-3 rounded-full bg-emerald-500" />
              <span className="text-[10px] uppercase font-black tracking-widest text-slate-500 ml-4">ArbitroBot Central Terminal v2.0.4</span>
            </div>
            <div className="text-[10px] text-slate-600 font-black tracking-[0.2em] uppercase">Node: Secure_01</div>
          </div>

          <div className="text-xs space-y-3 leading-relaxed">
            <div className="flex gap-4">
              <span className="text-slate-600">[08:42:01]</span>
              <span className="text-indigo-400/80">system starting kernel...</span>
            </div>
            <div className="flex gap-4">
              <span className="text-slate-600">[08:42:02]</span>
              <span className="text-indigo-400/80">mounting remote exchanges (MEXC, Gate, OKX)...</span>
            </div>
            <div className="flex gap-4">
              <span className="text-slate-600">[08:42:05]</span>
              <span className="text-indigo-400/80">checking rabbitmq cluster health... <span className="text-emerald-500 font-black uppercase">online</span></span>
            </div>
            <div className="flex gap-4">
              <span className="text-slate-600">[08:42:08]</span>
              <span className="text-indigo-400/80">scanning for liquidity skew... detected 4 opportunities</span>
            </div>
            <div className="pt-4 flex gap-4">
              <span className="text-emerald-500 font-black tracking-tighter">root@arbitrobot:~$</span>
              <span className="text-white animate-pulse">|</span>
            </div>
          </div>
        </motion.div>
        );

        const Globe = ({className}) => (
        <svg className={className} xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="10" />
          <path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20" />
          <path d="M2 12h20" />
        </svg>
        );
