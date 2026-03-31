import React, { useState } from 'react';
import { useSignalR } from './hooks/useSignalR';
import ArbitrageMatrix from './components/ArbitrageMatrix';
import AccountSummary from './components/AccountSummary';
import SettingsPanel from './components/SettingsPanel';
import HistoryView from './components/HistoryView';
import ListenerWatch from './components/ListenerWatch';
import Login from './components/Login';
import { Bot, Cpu, ShieldCheck, Terminal, LayoutGrid, Settings, History, Activity, LogOut } from 'lucide-react';

function App() {
  const [user, setUser] = useState(() => localStorage.getItem('arbit_user') || null);
  const [activeView, setActiveView] = useState('dashboard');

  const handleLogin = (username) => {
    setUser(username);
  };

  const handleLogout = () => {
    localStorage.removeItem('arbit_token');
    localStorage.removeItem('arbit_user');
    setUser(null);
  };

  if (!user) {
    return <Login onSuccess={handleLogin} />;
  }

  return <Dashboard user={user} activeView={activeView} setActiveView={setActiveView} onLogout={handleLogout} />;
}

function Dashboard({ user, activeView, setActiveView, onLogout }) {
  const { tickers, accountState, botConfig, listenerAlert, listenerOpportunity, isConnected, sendBotCommand } = useSignalR();

  const navItems = [
    { id: 'dashboard', label: 'Dashboard', icon: LayoutGrid },
    { id: 'history', label: 'History', icon: History },
    { id: 'listener', label: 'Listener', icon: Activity },
    { id: 'settings', label: 'Settings', icon: Settings },
  ];

  return (
    <div className="min-h-screen bg-slate-950 text-slate-200 selection:bg-indigo-500 selection:text-white pb-20">
      {/* Background */}
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
              <h1 className="text-xl font-bold tracking-tight text-white leading-none">
                ArbitroBot <span className="text-indigo-500 text-xs font-black uppercase ml-1">v2</span>
              </h1>
              <p className="text-[10px] text-slate-500 font-mono mt-1 flex items-center gap-1">
                <Cpu className="w-2.5 h-2.5" /> HIGH-FREQUENCY ARBITRAGE HUB
              </p>
            </div>
          </div>

          <nav className="hidden md:flex items-center gap-1">
            {navItems.map(({ id, label, icon: Icon }) => (
              <button
                key={id}
                onClick={() => setActiveView(id)}
                className={`flex items-center gap-2 px-4 py-2 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all ${
                  activeView === id
                    ? 'bg-indigo-600/20 text-indigo-400'
                    : 'text-slate-500 hover:text-slate-300 hover:bg-white/5'
                }`}
              >
                <Icon className="w-3.5 h-3.5" /> {label}
              </button>
            ))}
          </nav>

          <div className="flex items-center gap-4">
            <div className="hidden md:flex gap-3 items-center text-[10px] font-bold uppercase tracking-widest text-slate-500">
              <div className="flex items-center gap-1.5">
                <ShieldCheck className="w-3 h-3 text-emerald-500" /> Secure
              </div>
              <div className="flex items-center gap-1.5">
                <Terminal className="w-3 h-3 text-indigo-500" /> System: OK
              </div>
            </div>
            <div
              className={`h-2.5 w-2.5 rounded-full ring-4 ${
                isConnected
                  ? 'bg-emerald-500 ring-emerald-500/20 animate-pulse'
                  : 'bg-rose-500 ring-rose-500/20'
              }`}
            />
            <span className="text-xs text-slate-500 hidden md:block">{user}</span>
            <button
              onClick={onLogout}
              className="p-2 rounded-lg text-slate-500 hover:text-slate-300 hover:bg-white/5 transition-all"
              title="Sign out"
            >
              <LogOut className="w-4 h-4" />
            </button>
          </div>
        </div>
      </header>

      {/* Main Content */}
      <main className="max-w-7xl mx-auto px-6 mt-8 relative z-10">
        {activeView === 'dashboard' && (
          <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">
            <aside className="lg:col-span-4 flex flex-col gap-8">
              <AccountSummary state={accountState} isConnected={isConnected} />
            </aside>
            <section className="lg:col-span-8">
              <ArbitrageMatrix tickers={tickers} />
            </section>
          </div>
        )}

        {activeView === 'history' && (
          <HistoryView />
        )}

        {activeView === 'listener' && (
          <ListenerWatch alert={listenerAlert} opportunity={listenerOpportunity} />
        )}

        {activeView === 'settings' && (
          <SettingsPanel config={botConfig} sendCommand={sendBotCommand} />
        )}
      </main>

      {/* Mobile bottom nav */}
      <footer className="fixed bottom-0 left-0 right-0 bg-slate-950/90 backdrop-blur-md border-t border-slate-900 px-4 py-3 md:hidden flex justify-around">
        {navItems.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            onClick={() => setActiveView(id)}
            className={`flex flex-col items-center gap-1 text-[10px] font-bold uppercase tracking-wider transition-all ${
              activeView === id ? 'text-indigo-400' : 'text-slate-600'
            }`}
          >
            <Icon className="w-5 h-5" />
            {label}
          </button>
        ))}
      </footer>
    </div>
  );
}

export default App;
