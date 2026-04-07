import React, { useState } from 'react';
import { useRabbitMQ } from './hooks/useRabbitMQ';
import ArbitrageMatrix from './components/ArbitrageMatrix';
import AccountSummary from './components/AccountSummary';
import BotConfig from './components/BotConfig';
import SettingsPanel from './components/SettingsPanel';
import HistoryView from './components/HistoryView';
import ListenerWatch from './components/ListenerWatch';
import Login from './components/Login';
import { Bot, Cpu, LayoutGrid, Settings, History, Activity, LogOut } from 'lucide-react';

function App() {
  const [user, setUser] = useState(() => localStorage.getItem('arbit_user') || null);
  const [activeView, setActiveView] = useState('dashboard');

  const handleLogin = (username) => setUser(username);

  const handleLogout = () => {
    localStorage.removeItem('arbit_token');
    localStorage.removeItem('arbit_user');
    setUser(null);
  };

  if (!user) return <Login onSuccess={handleLogin} />;

  return <Dashboard user={user} activeView={activeView} setActiveView={setActiveView} onLogout={handleLogout} />;
}

function Dashboard({ user, activeView, setActiveView, onLogout }) {
  const { tickers, accountState, botConfig, listenerAlert, listenerOpportunity, isConnected, sendBotCommand } = useRabbitMQ();

  const navItems = [
    { id: 'dashboard', label: 'Dashboard', icon: LayoutGrid },
    { id: 'history',   label: 'History',   icon: History },
    { id: 'listener',  label: 'Listener',  icon: Activity },
    { id: 'settings',  label: 'Settings',  icon: Settings },
  ];

  return (
    <div className="min-h-screen bg-black text-white selection:bg-white selection:text-black pb-20">
      {/* Header */}
      <header className="sticky top-0 z-50 bg-black/90 backdrop-blur-md border-b border-white/10 px-6 py-4">
        <div className="max-w-7xl mx-auto flex justify-between items-center">
          {/* Logo */}
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 bg-white rounded-lg flex items-center justify-center">
              <Bot className="text-black w-5 h-5" />
            </div>
            <div>
              <h1 className="text-base font-bold tracking-tight text-white leading-none">
                ArbitroBot <span className="text-zinc-500 text-[10px] font-black uppercase ml-1">v2</span>
              </h1>
              <p className="text-[9px] text-zinc-600 font-mono mt-0.5 flex items-center gap-1">
                <Cpu className="w-2 h-2" /> HIGH-FREQUENCY ARBITRAGE
              </p>
            </div>
          </div>

          {/* Nav */}
          <nav className="hidden md:flex items-center gap-1">
            {navItems.map(({ id, label, icon: Icon }) => (
              <button
                key={id}
                onClick={() => setActiveView(id)}
                className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] font-semibold uppercase tracking-wider transition-all ${
                  activeView === id
                    ? 'bg-white text-black'
                    : 'text-zinc-500 hover:text-white hover:bg-white/10'
                }`}
              >
                <Icon className="w-3 h-3" /> {label}
              </button>
            ))}
          </nav>

          {/* Right side */}
          <div className="flex items-center gap-3">
            <div
              className={`h-2 w-2 rounded-full ring-2 ${
                isConnected ? 'bg-white ring-white/20 animate-pulse' : 'bg-zinc-600 ring-zinc-600/20'
              }`}
            />
            <span className="text-xs text-zinc-600 hidden md:block font-mono">{user}</span>
            <button
              onClick={onLogout}
              className="p-1.5 rounded-lg text-zinc-600 hover:text-white hover:bg-white/10 transition-all"
              title="Sign out"
            >
              <LogOut className="w-4 h-4" />
            </button>
          </div>
        </div>
      </header>

      {/* Main */}
      <main className="max-w-7xl mx-auto px-6 mt-8 relative z-10">
        {activeView === 'dashboard' && (
          <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">
            <aside className="lg:col-span-4 flex flex-col gap-8">
              <AccountSummary state={accountState} isConnected={isConnected} />
              {botConfig && <BotConfig config={botConfig} onCommand={sendBotCommand} />}
            </aside>
            <section className="lg:col-span-8">
              <ArbitrageMatrix tickers={tickers} botConfig={botConfig} />
            </section>
          </div>
        )}
        {activeView === 'history'   && <HistoryView />}
        {activeView === 'listener'  && <ListenerWatch alert={listenerAlert} opportunity={listenerOpportunity} />}
        {activeView === 'settings'  && <SettingsPanel config={botConfig} sendCommand={sendBotCommand} />}
      </main>

      {/* Mobile bottom nav */}
      <footer className="fixed bottom-0 left-0 right-0 bg-black/90 backdrop-blur-md border-t border-white/10 px-4 py-3 md:hidden flex justify-around">
        {navItems.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            onClick={() => setActiveView(id)}
            className={`flex flex-col items-center gap-1 text-[10px] font-bold uppercase tracking-wider transition-all ${
              activeView === id ? 'text-white' : 'text-zinc-600'
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
