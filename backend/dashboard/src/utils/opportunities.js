/** Default taker fees per exchange (as fraction, e.g. 0.0005 = 0.05%). Mirrors model.rs ExchangeId::taker_fee(). */
const DEFAULT_FEES = {
  binance:     0.0005,
  bybit:       0.0006,
  gate:        0.0005,
  hyperliquid: 0.00035,
  aster:       0.0005,
};

/** Taker fee for an exchange, checking botConfig overrides first. */
export function takerFee(exchangeName, botConfig) {
  const key = (exchangeName || '').toLowerCase();
  const overrides = botConfig?.taker_fee_overrides || {};
  // overrides keys are ExchangeId enum names (e.g. "Binance", "Bybit")
  const overrideKey = Object.keys(overrides).find(k => k.toLowerCase() === key);
  return overrideKey != null ? parseFloat(overrides[overrideKey]) : (DEFAULT_FEES[key] ?? 0.0005);
}

/**
 * Net spread after fees (both legs, open+close = 2 round-trips total).
 * net% = gross% - (fee_long + fee_short) * 2 * 100
 */
export function netSpread(grossSpreadPct, longExchange, shortExchange, botConfig) {
  const feeLong  = takerFee(longExchange,  botConfig);
  const feeShort = takerFee(shortExchange, botConfig);
  return grossSpreadPct - (feeLong + feeShort) * 2 * 100;
}

/** Normalize exchange key for lookup (backend may send "Binance", "Bybit", "Gate", "Hyperliquid", "Aster"). */
export function exchangeKey(ex) {
  if (ex == null) return '';
  const s = typeof ex === 'string' ? ex : String(ex);
  return s.toLowerCase();
}

/** Group tickers by symbol. */
export function groupBySymbol(tickers) {
  return Object.values(tickers || {}).reduce((acc, t) => {
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

/** Get sorted opportunities from tickers + botConfig (for live). Limit 10. */
export function getOpportunities(tickers, botConfig, limit = 10) {
  const blacklist = (botConfig?.symbol_blacklist || []).map(s => s.toUpperCase());
  const grouped = groupBySymbol(tickers);
  const opportunities = Object.entries(grouped)
    .filter(([symbol]) => !blacklist.includes(symbol.toUpperCase()))
    .map(([symbol, exts]) => {
      const { bestLong, bestShort, spread } = calculateSpread(exts, botConfig);
      const net = bestLong && bestShort
        ? netSpread(spread, bestLong.exchange, bestShort.exchange, botConfig)
        : spread;
      return { symbol, bestLong, bestShort, spread, net, exts };
    })
    .filter(opp => {
      if (!opp.bestLong || !opp.bestShort) return false;
      const minDepth = botConfig?.depth_usdt ?? 0;
      if (minDepth > 0) {
        const depth = availableDepthUsdt(opp.bestLong, opp.bestShort, minDepth * 2);
        if (depth < minDepth) return false;
      }
      return true;
    });

  opportunities.sort((a, b) => b.net - a.net);
  return opportunities.slice(0, limit);
}

/**
 * Compute available USDT depth at the best ask (long leg) and best bid (short leg).
 * Returns the min of the two — i.e. the max tradeable size at this spread.
 */
export function availableDepthUsdt(bestLong, bestShort, limitUsdt = 5000) {
  let longDepth = 0;
  for (const [price, qty] of (bestLong?.asks || [])) {
    longDepth += parseFloat(price) * parseFloat(qty);
    if (longDepth >= limitUsdt) break;
  }
  let shortDepth = 0;
  for (const [price, qty] of (bestShort?.bids || [])) {
    shortDepth += parseFloat(price) * parseFloat(qty);
    if (shortDepth >= limitUsdt) break;
  }
  return Math.min(longDepth, shortDepth);
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
