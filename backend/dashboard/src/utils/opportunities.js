/** Normalize exchange key for lookup (backend may send "Binance", "Bybit", "Gate", "Hyperliquid", "Aster"). */
export function exchangeKey(ex) {
  if (ex == null) return '';
  const s = typeof ex === 'string' ? ex : String(ex);
  return s.toLowerCase();
}

/** Group tickers by symbol. */
export function groupBySymbol(tickers) {
  return Object.values(tickers).reduce((acc, t) => {
    if (!acc[t.symbol]) acc[t.symbol] = [];
    acc[t.symbol].push(t);
    return acc;
  }, {});
}

/** Check if exchange is enabled (compare case-insensitively with config keys). */
function isExchangeEnabled(exchangeName, enabledExchanges) {
  if (!enabledExchanges || typeof enabledExchanges !== 'object') return true;
  const key = exchangeKey(exchangeName);
  const keys = Object.keys(enabledExchanges);
  const found = keys.find(k => exchangeKey(k) === key);
  return found === undefined ? true : enabledExchanges[found] !== false;
}

/** Compute best long/short and spread for one symbol's exchanges. */
export function calculateSpread(exchanges, botConfig) {
  if (exchanges.length < 2) return { bestLong: null, bestShort: null, spread: 0 };

  const now = Date.now();
  const FRESH_MS = 90 * 1000;
  const valid = exchanges.filter(e => {
    const ask = parseFloat(e.asks?.[0]?.[0]);
    const bid = parseFloat(e.bids?.[0]?.[0]);
    const isFresh = (now - e.timestamp) < FRESH_MS;
    const isEnabled = isExchangeEnabled(e.exchange, botConfig?.enabled_exchanges);
    return ask > 0.00000001 && bid > 0.00000001 && isFresh && isEnabled;
  });

  if (valid.length < 2) return { bestLong: null, bestShort: null, spread: 0 };

  const bestLong = valid.reduce((a, b) => parseFloat(a.asks[0][0]) < parseFloat(b.asks[0][0]) ? a : b);
  const bestShort = valid.reduce((a, b) => parseFloat(a.bids[0][0]) > parseFloat(b.bids[0][0]) ? a : b);

  const longPrice = parseFloat(bestLong.asks[0][0]);
  const shortPrice = parseFloat(bestShort.bids[0][0]);

  const ratio = shortPrice > longPrice ? shortPrice / longPrice : longPrice / shortPrice;
  if (ratio > 1.5) return { bestLong: null, bestShort: null, spread: 0 };

  const spread = ((shortPrice - longPrice) / longPrice) * 100;

  // Display threshold is always 0 — the bot's min_spread_threshold is for TRADING, not display
  // if (spread < threshold) return { bestLong: null, bestShort: null, spread: 0 };

  if (Math.abs(spread) > 50) return { bestLong: null, bestShort: null, spread: 0 };

  return { bestLong, bestShort, spread };
}

/** Get sorted opportunities from tickers + botConfig (for live). Limit 50. */
export function getOpportunities(tickers, botConfig, limit = 50) {
  const grouped = groupBySymbol(tickers);
  const opportunities = Object.entries(grouped)
    .map(([symbol, exts]) => {
      const { bestLong, bestShort, spread } = calculateSpread(exts, botConfig);
      return { symbol, bestLong, bestShort, spread, exts };
    })
    .filter(opp => opp.bestLong && opp.bestShort);

  opportunities.sort((a, b) => b.spread - a.spread);
  return opportunities.slice(0, limit);
}

/** Serialize one opportunity for history storage. */
export function serializeOpportunity(opp) {
  return {
    symbol: opp.symbol,
    longExchange: opp.bestLong.exchange,
    longPrice: parseFloat(opp.bestLong.asks[0][0]),
    shortExchange: opp.bestShort.exchange,
    shortPrice: parseFloat(opp.bestShort.bids[0][0]),
    spread: opp.spread,
  };
}
