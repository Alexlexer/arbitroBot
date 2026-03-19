import React, { useState, useEffect } from 'react';
import { useRabbitMQ } from './hooks/useRabbitMQ';
import ArbitrageMatrix from './components/ArbitrageMatrix';
import AccountSummary from './components/AccountSummary';
import BotConfig from './components/BotConfig';
import HistoryView from './components/HistoryView';
import Settings from './components/Settings';
import Login from './components/Login';
import ListenerWatch from './components/ListenerWatch';
import ListenerSettings from './components/ListenerSettings';
import { LayoutDashboard, Settings as SettingsIcon, Zap, ShieldCheck, LogOut, Activity, History, AlertTriangle } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

const AUTH_KEY = 'arbitro_dashboard_auth';
const USERNAME_KEY = 'arbitro_dashboard_username';

function App() {
  const { tickers, accountState, botConfig, listenerAlert, listenerOpportunity, isConnected, sendBotCommand, login, register } = useRabbitMQ();
  const [activeTab, setActiveTab] = useState('dashboard');
  const [dashboardMode, setDashboardMode] = useState('live'); // 'live' | 'history'
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [username, setUsername] = useState('');

  useEffect(() => {
    setIsAuthenticated(sessionStorage.getItem(AUTH_KEY) === '1');
    setUsername(sessionStorage.getItem(USERNAME_KEY) || '');
  }, []);

  const handleLoginSuccess = (loggedInUsername) => {
    sessionStorage.setItem(AUTH_KEY, '1');
    sessionStorage.setItem(USERNAME_KEY, loggedInUsername || '');
    setIsAuthenticated(true);
    setUsername(loggedInUsername || '');
  };

  const handleLogout = () => {
    sessionStorage.removeItem(AUTH_KEY);
    sessionStorage.removeItem(USERNAME_KEY);
    setIsAuthenticated(false);
    setUsername('');
  };

  if (!isAuthenticated) {
    return (
      <Login
        onSuccess={handleLoginSuccess}
        login={login}
        register={register}
        isConnected={isConnected}
      />
    );
  }

  return (
    <div className="min-h-screen bg-black text-white font-sans selection:bg-white/15">
      {/* Premium Top Navigation */}
      <nav className="sticky top-0 z-50 bg-black/80 backdrop-blur-xl border-b border-white/10">
        <div className="max-w-[1600px] mx-auto px-6 h-20 flex items-center justify-between">
          <div className="flex items-center gap-8">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 bg-white/10 rounded-xl flex items-center justify-center">
                <Zap className="w-6 h-6 text-white fill-current" />
              </div>
              <div>
                <span className="text-xl font-bold text-white tracking-tight">Arbitro<span className="text-white">Bot</span></span>
                <div className="flex items-center gap-2 mt-0.5">
                  <div className={`w-1.5 h-1.5 rounded-full ${isConnected ? 'bg-white animate-pulse' : 'bg-white/20'}`} />
                  <span className="text-[10px] text-white/60 font-semibold uppercase tracking-widest">{isConnected ? 'System Live' : 'Connecting...'}</span>
                </div>
              </div>
            </div>

          <div className="h-8 w-px bg-white/10" />

            <div className="flex items-center gap-1">
              <button
                onClick={() => { setActiveTab('dashboard'); setDashboardMode('live'); }}
                className={`flex items-center gap-2 px-6 py-2.5 rounded-xl text-sm font-semibold transition-all ${
                  activeTab === 'dashboard' 
                  ? 'bg-white/10 text-white border border-white/10'
                  : 'text-white/60 hover:text-white hover:bg-white/5'
                }`}
              >
                <LayoutDashboard className="w-4 h-4" />
                Dashboard
              </button>
              <button
                onClick={() => { setActiveTab('listener'); }}
                className={`flex items-center gap-2 px-6 py-2.5 rounded-xl text-sm font-semibold transition-all ${
                  activeTab === 'listener'
                  ? 'bg-white/10 text-white border border-white/10'
                  : 'text-white/60 hover:text-white hover:bg-white/5'
                }`}
              >
                <AlertTriangle className="w-4 h-4" />
                Listener
              </button>
              {activeTab === 'dashboard' && (
                <>
                  <button
                    onClick={() => setDashboardMode('live')}
                    className={`flex items-center gap-2 px-4 py-2.5 rounded-xl text-sm font-semibold transition-all ${
                      dashboardMode === 'live'
                    ? 'bg-white/10 text-white border border-white/10'
                    : 'text-white/60 hover:text-white hover:bg-white/5'
                    }`}
                  >
                    <Activity className="w-4 h-4" />
                    Live
                  </button>
                  <button
                    onClick={() => setDashboardMode('history')}
                    className={`flex items-center gap-2 px-4 py-2.5 rounded-xl text-sm font-semibold transition-all ${
                      dashboardMode === 'history'
                    ? 'bg-white/10 text-white border border-white/10'
                    : 'text-white/60 hover:text-white hover:bg-white/5'
                    }`}
                  >
                    <History className="w-4 h-4" />
                    History
                  </button>
                </>
              )}
              <button
                onClick={() => setActiveTab('settings')}
                className={`flex items-center gap-2 px-6 py-2.5 rounded-xl text-sm font-semibold transition-all ${
                  activeTab === 'settings' 
                    ? 'bg-white/10 text-white border border-white/10'
                    : 'text-white/60 hover:text-white hover:bg-white/5'
                }`}
              >
                <SettingsIcon className="w-4 h-4" />
                Settings
              </button>
            </div>
          </div>

          <div className="flex items-center gap-4">
            <div className="hidden xl:flex items-center gap-2 px-4 py-2 bg-white/5 border border-white/10 rounded-full">
              <ShieldCheck className="w-4 h-4 text-white/60" />
              <span className="text-xs font-medium text-white/60">Risk Manager <span className="text-white/60 uppercase">Active</span></span>
            </div>
            {username && (
              <span className="text-sm text-white/60 truncate max-w-[120px]" title={username}>
                {username}
              </span>
            )}
            <button
              onClick={handleLogout}
              className="flex items-center gap-2 px-4 py-2 rounded-xl text-sm font-medium text-white/60 hover:text-white hover:bg-white/5 transition-all"
              title="Sign out"
            >
              <LogOut className="w-4 h-4" />
              Sign out
            </button>
          </div>
        </div>
      </nav>

      {/* Main Content Area */}
      <main className="max-w-[1600px] mx-auto pt-8 pb-12 px-6">
        <AnimatePresence mode="wait">
          {activeTab === 'dashboard' ? (
            <motion.div
              key={`dashboard-${dashboardMode}`}
              initial={{ opacity: 0, scale: 0.98 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.98 }}
              transition={{ duration: 0.2 }}
              className={dashboardMode === 'history' ? '' : 'grid grid-cols-1 xl:grid-cols-12 gap-8'}
            >
              {dashboardMode === 'live' ? (
                <>
                  <div className="xl:col-span-8 space-y-8">
                    <ArbitrageMatrix tickers={tickers} botConfig={botConfig} />
                  </div>
                  <div className="xl:col-span-4 space-y-8">
                    <AccountSummary state={accountState} isConnected={isConnected} tickers={tickers} botConfig={botConfig} />
                    <BotConfig config={botConfig} onCommand={sendBotCommand} />
                  </div>
                </>
              ) : (
                <HistoryView />
              )}
            </motion.div>
          ) : (
            <>
              {activeTab === 'listener' ? (
                <motion.div
                  key="listener"
                  initial={{ opacity: 0, y: 20 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: 20 }}
                  transition={{ duration: 0.2 }}
                  className="max-w-[900px]"
                >
                  <div className="space-y-4">
                    <ListenerSettings config={botConfig} onCommand={sendBotCommand} />
                    <ListenerWatch alert={listenerAlert} opportunity={listenerOpportunity} />
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
            </>
          )}
        </AnimatePresence>
      </main>

      {/* Decorative Background Elements */}
      <div className="fixed inset-0 pointer-events-none -z-10 overflow-hidden">
        <div className="absolute top-0 right-0 w-[500px] h-[500px] bg-white/5 blur-[120px] rounded-full" />
        <div className="absolute bottom-0 left-0 w-[500px] h-[500px] bg-white/5 blur-[120px] rounded-full" />
      </div>
    </div>
  );
}

export default App;
