import React from 'react';
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
          </div>

          <div className="flex items-center gap-6">
            <div className="hidden md:flex gap-4 items-center text-[10px] font-black uppercase tracking-widest text-slate-500">
              <div className="flex items-center gap-1.5"><ShieldCheck className="w-3 h-3 text-emerald-500" /> Secure Node</div>
              <div className="flex items-center gap-1.5"><Terminal className="w-3 h-3 text-indigo-500" /> System: OK</div>
            </div>
            <div className={`h-2.5 w-2.5 rounded-full ring-4 ${isConnected ? 'bg-emerald-500 ring-emerald-500/20 animate-pulse' : 'bg-rose-500 ring-rose-500/20'}`} />
          </div>
        </div>
      </header>

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
