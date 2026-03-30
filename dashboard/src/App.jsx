import React from 'react';
import { useRabbitMQ } from './hooks/useRabbitMQ';
import ArbitrageMatrix from './components/ArbitrageMatrix';
import AccountSummary from './components/AccountSummary';
import SettingsPanel from './components/SettingsPanel';
import { Bot, Terminal, ShieldCheck, Cpu, LayoutGrid, Activity, Command } from 'lucide-react';
import { motion } from 'framer-motion';

function App() {
  const { tickers, accountState, isConnected, sendCommand } = useRabbitMQ();
  const [activeView, setActiveView] = React.useState('dashboard');

  return (
    <div className="min-h-screen bg-slate-950 text-slate-200 selection:bg-indigo-500 selection:text-white pb-20 relative overflow-x-hidden">
      {/* Dynamic Aesthetic Background */}
      <div className="fixed inset-0 pointer-events-none z-0">
        <div className="absolute inset-0 animate-mesh opacity-30" />
        <div className="absolute top-[-10%] left-[-10%] w-[40%] h-[40%] bg-indigo-600/20 rounded-full blur-[120px] animate-float" />
        <div className="absolute bottom-[-10%] right-[-10%] w-[40%] h-[40%] bg-violet-600/20 rounded-full blur-[120px] animate-float" style={{ animationDelay: '-3s' }} />
      </div>

      {/* Header */}
      <header className="sticky top-0 z-50 bg-slate-950/50 backdrop-blur-2xl border-b border-white/5 px-8 py-5">
        <div className="max-w-[1600px] mx-auto flex justify-between items-center">
          <motion.div
            initial={{ opacity: 0, x: -20 }}
            animate={{ opacity: 1, x: 0 }}
            className="flex items-center gap-4"
          >
            <div className="relative group">
              <div className="absolute -inset-1 bg-gradient-to-r from-indigo-500 to-fuchsia-500 rounded-2xl blur opacity-40 group-hover:opacity-100 transition duration-1000 group-hover:duration-200" />
              <div className="relative w-12 h-12 bg-slate-900 rounded-2xl flex items-center justify-center border border-white/10">
                <Bot className="text-white w-7 h-7" />
              </div>
            </div>
            <div>
              <div className="flex items-center gap-2">
                <h1 className="text-2xl font-black tracking-tighter text-white leading-none">ARBITRO<span className="text-indigo-500">BOT</span></h1>
                <div className="px-2 py-0.5 rounded-md bg-indigo-500/10 border border-indigo-500/20 text-[9px] font-black text-indigo-400 uppercase tracking-widest">Enterprise</div>
              </div>
              <div className="flex items-center gap-2 mt-1.5 opacity-40">
                <Cpu className="w-3 h-3" />
                <span className="text-[9px] font-black uppercase tracking-[0.3em]">HFT Liquidity Arb Hub</span>
              </div>
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

              <div className={`group relative h-4 w-4 rounded-full flex items-center justify-center transition-all ${isConnected ? 'bg-emerald-500/20' : 'bg-rose-500/20'}`}>
                <div className={`h-2 w-2 rounded-full ring-2 ${isConnected ? 'bg-emerald-500 ring-emerald-500/40 animate-pulse' : 'bg-rose-500 ring-rose-500/40'}`} />
                {/* Tooltip */}
                <div className="absolute top-10 right-0 glass px-3 py-1.5 rounded-lg opacity-0 group-hover:opacity-100 transition-opacity whitespace-nowrap text-[10px] font-bold pointer-events-none">
                  Status: {isConnected ? 'Synchronized' : 'Reconnecting...'}
                </div>
              </div>
            </div>
          </div>
        </div>
      </header>

      <main className="max-w-[1600px] mx-auto px-8 mt-12 relative z-10">
        {activeView === 'dashboard' && (
          <div className="grid grid-cols-1 lg:grid-cols-12 gap-10">
            {/* Left Column: Metrics & Control */}
            <aside className="lg:col-span-4 flex flex-col gap-10">
              <section>
                <AccountSummary state={accountState} isConnected={isConnected} />
              </section>
              <section>
                <SettingsPanel config={accountState?.config} secrets={accountState?.secrets} sendCommand={sendCommand} />
              </section>
            </aside>

            {/* Right Column: Active Feed */}
            <div className="lg:col-span-8 space-y-10">
              <section>
                <ArbitrageMatrix tickers={tickers} />
              </section>

              {/* System Logs Placeholder */}
              <section className="glass-morphism rounded-[2.5rem] p-8 border border-slate-800/50">
                <div className="flex items-center gap-2 mb-4">
                  <Terminal className="w-4 h-4 text-slate-500" />
                  <h3 className="text-xs font-black uppercase tracking-widest text-slate-400">System Logs</h3>
                </div>
                <div className="font-mono text-[11px] text-slate-600 space-y-1">
                  <div>[08:42:12] <span className="text-indigo-500/50">INFO:</span> Handshake established for Cluster_A</div>
                  <div>[08:42:15] <span className="text-emerald-500/50">OK:</span> Position sync completed for Binance-USDT</div>
                  <div>[08:42:18] <span className="text-amber-500/50">WARN:</span> High volatility detected on OKX-PEPE</div>
                </div>
              </section>
            </div>
          </div>
        )}

        {activeView === 'analytics' && <AnalyticsView />}
        {activeView === 'terminal' && <TerminalView />}
      </main>

      {/* Dynamic Background Noise */}
      <div className="fixed inset-0 pointer-events-none z-[1] opacity-[0.03]" style={{ backgroundImage: 'url("https://grainy-gradients.vercel.app/noise.svg")' }} />
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

const Globe = ({ className }) => (
  <svg className={className} xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="12" cy="12" r="10" />
    <path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20" />
    <path d="M2 12h20" />
  </svg>
);
