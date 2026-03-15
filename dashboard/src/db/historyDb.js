const DB_NAME = 'arbitro_history';
const STORE = 'snapshots';
const RETENTION_DAYS = 30;

function openDb() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1);
    req.onerror = () => reject(req.error);
    req.onsuccess = () => resolve(req.result);
    req.onupgradeneeded = (e) => {
      const db = e.target.result;
      if (!db.objectStoreNames.contains(STORE)) {
        const store = db.createObjectStore(STORE, { keyPath: 'timestamp' });
        store.createIndex('byDate', 'timestamp', { unique: true });
      }
    };
  });
}

/** Save one snapshot. opportunities: [{ symbol, longExchange, longPrice, shortExchange, shortPrice, spread }]. Deletes entries older than RETENTION_DAYS. */
export async function addSnapshot(opportunities) {
  const timestamp = Date.now();
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE, 'readwrite');
    const store = tx.objectStore(STORE);
    store.put({ timestamp, opportunities });
    const cutoff = timestamp - RETENTION_DAYS * 24 * 60 * 60 * 1000;
    const range = IDBKeyRange.upperBound(cutoff, true);
    const req = store.openCursor(range);
    req.onsuccess = () => {
      const cursor = req.result;
      if (cursor) {
        store.delete(cursor.primaryKey);
        cursor.continue();
      }
    };
    tx.oncomplete = () => { db.close(); resolve(); };
    tx.onerror = () => { db.close(); reject(tx.error); };
  });
}

/** Get all snapshot timestamps (for last RETENTION_DAYS), descending. */
export async function getSnapshotTimestamps() {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE, 'readonly');
    const store = tx.objectStore(STORE);
    const req = store.getAllKeys();
    req.onsuccess = () => {
      const keys = (req.result || []).sort((a, b) => b - a);
      db.close();
      resolve(keys);
    };
    req.onerror = () => { db.close(); reject(req.error); };
  });
}

/** Get one snapshot by timestamp. */
export async function getSnapshot(timestamp) {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE, 'readonly');
    const req = tx.objectStore(STORE).get(timestamp);
    req.onsuccess = () => {
      db.close();
      resolve(req.result || null);
    };
    req.onerror = () => { db.close(); reject(req.error); };
  });
}
