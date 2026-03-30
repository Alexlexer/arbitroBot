import React, { useState, useEffect } from 'react';
import { History, Calendar, Clock, ChevronLeft } from 'lucide-react';
import ArbitrageMatrix from './ArbitrageMatrix';

// History is now served by the .NET backend (which proxies to the Rust Axum API).
// In dev Vite proxies /api → http://localhost:5000, so relative URLs work everywhere.

function formatDate(ts) {
  return new Date(ts).toLocaleDateString(undefined, {
    weekday: 'short', month: 'short', day: 'numeric', year: 'numeric',
  });
}

function formatTime(ts) {
  return new Date(ts).toLocaleTimeString(undefined, {
    hour: '2-digit', minute: '2-digit', second: '2-digit',
  });
}

export default function HistoryView() {
  const [snapshots, setSnapshots] = useState([]);
  const [loading, setLoading]     = useState(true);
  const [error, setError]         = useState(null);
  const [selectedTs, setSelectedTs] = useState(null);

  useEffect(() => {
    let cancelled = false;
    const token = localStorage.getItem('arbit_token');

    fetch('/api/history?days=30', {
      headers: token ? { Authorization: `Bearer ${token}` } : {},
    })
      .then((res) => {
        if (res.status === 401) throw new Error('Session expired — please sign in again');
        if (!res.ok) throw new Error(res.statusText || `HTTP ${res.status}`);
        return res.json();
      })
      .then((data) => {
        if (!cancelled && Array.isArray(data)) setSnapshots(data);
      })
      .catch((e) => {
        if (!cancelled) setError(e.message || 'Failed to load history');
      })
      .finally(() => { if (!cancelled) setLoading(false); });

    return () => { cancelled = true; };
  }, []);

  const selectedSnapshot = selectedTs != null
    ? snapshots.find((s) => s.timestamp === selectedTs)
    : null;

  // Group snapshots by day (YYYY-MM-DD), most recent first
  const byDay = snapshots.reduce((acc, s) => {
    const key = new Date(s.timestamp).toISOString().slice(0, 10);
    if (!acc[key]) acc[key] = [];
    acc[key].push(s.timestamp);
    return acc;
  }, {});
  const days = Object.entries(byDay).sort(([a], [b]) => b.localeCompare(a));

  const snapshotLabel = selectedTs != null
    ? `Snapshot — ${formatDate(selectedTs)} ${formatTime(selectedTs)}`
    : null;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-2 text-white/70 mb-4">
        <History className="w-5 h-5" />
        <h2 className="text-xl font-bold tracking-tight text-white">History</h2>
        <span className="text-xs text-white/60">Last 30 days · bot snapshots every 5 min</span>
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-12 gap-6">
        {/* Snapshot picker */}
        <div className="xl:col-span-4 space-y-4">
          <div className="bg-black/70 backdrop-blur-xl rounded-2xl border border-white/10 p-4">
            <div className="flex items-center gap-2 text-white/60 text-sm font-medium mb-3">
              <Calendar className="w-4 h-4" />
              Pick a snapshot
            </div>

            {error ? (
              <p className="text-white/60 text-sm">Error: {error}</p>
            ) : loading ? (
              <p className="text-white/60 text-sm">Loading…</p>
            ) : snapshots.length === 0 ? (
              <p className="text-white/60 text-sm">
                No snapshots yet. The bot saves every 5 min when running.
              </p>
            ) : (
              <div className="space-y-4 max-h-[60vh] overflow-y-auto pr-2">
                {days.map(([dateKey, times]) => (
                  <div key={dateKey}>
                    <div className="text-xs font-bold text-white/60 uppercase tracking-wider mb-2">
                      {formatDate(new Date(dateKey).getTime())}
                    </div>
                    <div className="flex flex-wrap gap-2">
                      {times.map((ts) => (
                        <button
                          key={ts}
                          type="button"
                          onClick={() => setSelectedTs(ts)}
                          className={`flex items-center gap-1.5 px-3 py-2 rounded-xl text-sm font-medium transition-all ${
                            selectedTs === ts
                              ? 'bg-white/10 text-white'
                              : 'bg-white/5 text-white/70 hover:bg-white/10'
                          }`}
                        >
                          <Clock className="w-3.5 h-3.5" />
                          {formatTime(ts)}
                        </button>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Snapshot detail */}
        <div className="xl:col-span-8">
          {selectedTs == null ? (
            <div className="bg-black/70 backdrop-blur-xl rounded-2xl border border-white/10 p-12 text-center">
              <ChevronLeft className="w-10 h-10 text-white/50 mx-auto mb-3" />
              <p className="text-white/60">Select a time on the left to view that snapshot</p>
            </div>
          ) : selectedSnapshot?.opportunities ? (
            <ArbitrageMatrix
              snapshotOpportunities={selectedSnapshot.opportunities}
              snapshotLabel={snapshotLabel}
            />
          ) : (
            <div className="bg-black/70 backdrop-blur-xl rounded-2xl border border-white/10 p-12 text-center">
              <p className="text-white/60">Loading snapshot…</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
