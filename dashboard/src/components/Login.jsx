import React, { useState } from 'react';
import { Zap, Lock, User, KeyRound, AlertCircle } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

const Login = ({ onSuccess, login, register, isConnected }) => {
  const [mode, setMode] = useState('login'); // 'login' | 'register'
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [inviteCode, setInviteCode] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleLogin = async (e) => {
    e.preventDefault();
    setError('');
    if (!username.trim() || !password) {
      setError('Enter username and password');
      return;
    }
    if (!isConnected) {
      setError('Connecting to broker… Try again in a moment.');
      return;
    }
    setLoading(true);
    try {
      const res = await login(username.trim(), password);
      if (res.ok) {
        onSuccess(res.username || username.trim());
      } else {
        setError(res.error || 'Wrong username or password');
      }
    } catch (err) {
      setError(err.message || 'Login failed');
    } finally {
      setLoading(false);
    }
  };

  const handleRegister = async (e) => {
    e.preventDefault();
    setError('');
    if (!username.trim() || !password || !inviteCode.trim()) {
      setError('Fill in all fields');
      return;
    }
    if (password.length < 8) {
      setError('Password must be at least 8 characters');
      return;
    }
    if (!isConnected) {
      setError('Connecting to broker… Try again in a moment.');
      return;
    }
    setLoading(true);
    try {
      const res = await register(username.trim(), password, inviteCode.trim());
      if (res.ok) {
        onSuccess(res.username || username.trim());
      } else {
        setError(res.error || 'Registration failed');
      }
    } catch (err) {
      const msg = err.message || 'Registration failed';
      setError(msg === 'Registration timeout'
        ? 'No response from bot. Ensure the bot is running and rebuilt (docker compose up --build -d), and INVITE_CODE is set in .env.'
        : msg);
    } finally {
      setLoading(false);
    }
  };

  const submit = mode === 'login' ? handleLogin : handleRegister;

  return (
    <div className="min-h-screen bg-black text-white flex items-center justify-center p-6">
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3 }}
        className="w-full max-w-md"
      >
        <div className="bg-black/60 border border-white/10 rounded-2xl p-8 shadow-xl backdrop-blur">
          <div className="flex flex-col items-center mb-8">
            <div className="w-14 h-14 bg-white/10 rounded-xl flex items-center justify-center mb-4">
              <Zap className="w-8 h-8 text-white fill-current" />
            </div>
            <h1 className="text-2xl font-bold text-white tracking-tight">
              Arbitro<span className="text-white">Bot</span>
            </h1>
            <p className="text-slate-400 text-sm mt-1">
              {mode === 'login' ? 'Sign in to the dashboard' : 'Create an account'}
            </p>
          </div>

          {/* Tabs: Sign in | Register */}
          <div className="flex rounded-xl bg-slate-800/50 p-1 mb-4">
            <button
              type="button"
              onClick={() => { setMode('login'); setError(''); }}
              className={`flex-1 py-2.5 rounded-lg text-sm font-semibold transition-all ${mode === 'login' ? 'bg-white/10 text-white shadow' : 'text-white/60 hover:text-white hover:bg-white/5'}`}
            >
              Sign in
            </button>
            <button
              type="button"
              onClick={() => { setMode('register'); setError(''); }}
              className={`flex-1 py-2.5 rounded-lg text-sm font-semibold transition-all ${mode === 'register' ? 'bg-white/10 text-white shadow' : 'text-white/60 hover:text-white hover:bg-white/5'}`}
            >
              Register
            </button>
          </div>
          <p className="text-center text-slate-500 text-xs mb-2">
            {mode === 'login' ? "Don't have an account? Click Register." : 'Have an invite code? Create your account above.'}
          </p>

          <form onSubmit={submit} className="space-y-5">
            <div>
              <label htmlFor="username" className="block text-sm font-medium text-slate-400 mb-2">
                Username
              </label>
              <div className="relative">
                <User className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-500" />
                <input
                  id="username"
                  type="text"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  placeholder="Username"
                  className="w-full pl-10 pr-4 py-3 bg-slate-800/80 border border-slate-700 rounded-xl text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-white/20 focus:border-white/10"
                  autoComplete="username"
                  autoFocus
                  disabled={loading}
                />
              </div>
            </div>

            <div>
              <label htmlFor="password" className="block text-sm font-medium text-slate-400 mb-2">
                Password
              </label>
              <div className="relative">
                <Lock className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-500" />
                <input
                  id="password"
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder={mode === 'register' ? 'At least 8 characters' : 'Password'}
                  className="w-full pl-10 pr-4 py-3 bg-slate-800/80 border border-slate-700 rounded-xl text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-white/20 focus:border-white/10"
                  autoComplete={mode === 'login' ? 'current-password' : 'new-password'}
                  disabled={loading}
                />
              </div>
            </div>

            <AnimatePresence mode="wait">
              {mode === 'register' && (
                <motion.div
                  key="invite"
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: 'auto' }}
                  exit={{ opacity: 0, height: 0 }}
                  transition={{ duration: 0.2 }}
                >
                  <label htmlFor="inviteCode" className="block text-sm font-medium text-slate-400 mb-2">
                    Invite code
                  </label>
                  <div className="relative">
                    <KeyRound className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-500" />
                    <input
                      id="inviteCode"
                      type="password"
                      value={inviteCode}
                      onChange={(e) => setInviteCode(e.target.value)}
                      placeholder="Invite code"
                      className="w-full pl-10 pr-4 py-3 bg-slate-800/80 border border-slate-700 rounded-xl text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-white/20 focus:border-white/10"
                      autoComplete="off"
                      disabled={loading}
                    />
                  </div>
                </motion.div>
              )}
            </AnimatePresence>

            {error && (
              <div className="flex items-center gap-2 text-white/60 text-sm">
                <AlertCircle className="w-4 h-4 flex-shrink-0" />
                <span>{error}</span>
              </div>
            )}

            {/* Connection status bar */}
            <div className={`flex items-center gap-2 px-3 py-2 rounded-lg text-xs font-medium ${
              isConnected 
                ? 'bg-white/5 text-white/60' 
                : 'bg-red-900/20 text-red-300 border border-red-500/20'
            }`}>
              <div className={`w-2 h-2 rounded-full ${isConnected ? 'bg-green-400 animate-pulse' : 'bg-red-400'}`} />
              {isConnected
                ? 'Connected to broker'
                : 'Not connected — ensure port 15674 is open, then refresh'}
            </div>

            <button
              type="submit"
              disabled={loading || !isConnected}
              className="w-full py-3 px-4 bg-white/10 hover:bg-white/15 disabled:bg-white/5 disabled:text-white/30 disabled:cursor-not-allowed text-white font-semibold rounded-xl transition-colors"
            >
              {loading
                ? (mode === 'login' ? 'Signing in…' : 'Creating account…')
                : (mode === 'login' ? 'Sign in' : 'Create account')}
            </button>
          </form>
        </div>
      </motion.div>

      <div className="fixed inset-0 pointer-events-none -z-10 overflow-hidden">
        <div className="absolute top-0 right-0 w-[500px] h-[500px] bg-white/5 blur-[120px] rounded-full" />
        <div className="absolute bottom-0 left-0 w-[500px] h-[500px] bg-white/5 blur-[120px] rounded-full" />
      </div>
    </div>
  );
};

export default Login;
