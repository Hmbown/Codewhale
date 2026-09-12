import { type Trace } from '../core/model.js';
import type { PetBucket } from '../core/pet-telemetry.js';
import type { PetInteraction, PetWorldCheckpoint } from '../core/pet-world.js';
export interface SavedHabitat {
  petPersistenceVersion: 1; seconds: number; source: 'wild' | 'demo' | 'replay';
  sourceName: string; still: boolean; tape: readonly PetBucket[]; interactions: readonly PetInteraction[];
  checkpoint?: PetWorldCheckpoint; expressionVersion?: 1 | 2;
}
export interface HabitatEntry { revision: number; habitat: SavedHabitat }
export interface SavedReference { key: string; savedAt: string }
export interface SavedSummary extends SavedReference { name: string; events: number; privacy: Trace['privacy']; source: Trace['source'] }
interface SavedTrace extends SavedReference { trace: Trace }
const summary = (v: SavedTrace): SavedSummary => ({ key: v.key, savedAt: v.savedAt, name: v.trace.name, events: v.trace.events.length, privacy: v.trace.privacy, source: v.trace.source });
const problem = (e?: DOMException | null): Error => new Error(e?.name === 'QuotaExceededError'
  ? 'The browser storage quota is full. Export this trace to a file or delete an older saved trace.'
  : e?.message || 'Browser storage failed. File import and export still work.');
const missing = () => new Error('This saved trace was deleted in another tab. Use Save separate copy to keep the loaded recording.');
const changed = () => new Error('This saved trace changed in another tab. Reopen it before updating, or use Save separate copy.');
/** Existing library with an additive habitat store, no destructive migration. savedAt is both a display time
 * and an optimistic version; every mutation advances it by at least 1 ms.
 * Trace and summary are committed together. A request success is not a commit. */
export class TraceLibrary {
  private db?: Promise<IDBDatabase>;
  constructor(private factory?: IDBFactory) {}
  private open(): Promise<IDBDatabase> {
    if (this.db) return this.db;
    const opened = new Promise<IDBDatabase>((resolve, reject) => {
      let abandoned = false, request: IDBOpenDBRequest;
      try { request = (this.factory ?? indexedDB).open('whalesong-local', 3); }
      catch { reject(new Error('This browser origin does not permit local persistence. Run npm start, or use JSONL export.')); return; }
      request.onupgradeneeded = () => {
        if (abandoned) { request.transaction?.abort(); return; }
        const db = request.result;
        if (!db.objectStoreNames.contains('habitats')) db.createObjectStore('habitats');
        if (!db.objectStoreNames.contains('traces')) db.createObjectStore('traces', { keyPath: 'key' });
        if (!db.objectStoreNames.contains('summaries')) {
          const metas = db.createObjectStore('summaries', { keyPath: 'key' });
          const cursor = request.transaction!.objectStore('traces').openCursor();
          cursor.onsuccess = () => { const c = cursor.result; if (c) { try { metas.put(summary(c.value)); c.continue(); } catch { request.transaction?.abort(); } } };
        }
      };
      request.onsuccess = () => {
        if (abandoned) { request.result.close(); return; }
        const db = request.result;
        db.onversionchange = () => { db.close(); if (this.db === opened) this.db = undefined; };
        db.onclose = () => { if (this.db === opened) this.db = undefined; };
        resolve(db);
      };
      request.onblocked = () => { abandoned = true; reject(new Error('An older Whalesong tab is blocking the library upgrade. Close it, then reopen the library.')); };
      request.onerror = () => reject(problem(request.error));
    });
    this.db = opened;
    void opened.catch(() => { if (this.db === opened) this.db = undefined; });
    return opened;
  }
  private async transaction<T>(mode: IDBTransactionMode, action: (traces: IDBObjectStore, metas: IDBObjectStore, done: (value: T) => void, fail: (error: Error) => void, habitats: IDBObjectStore) => void): Promise<T> {
    const db = await this.open();
    return new Promise<T>((resolve, reject) => {
      let tx: IDBTransaction;
      try { tx = db.transaction(['traces', 'summaries', 'habitats'], mode); }
      catch (e) { this.db = undefined; reject(problem(e as DOMException)); return; }
      let result: T, failure: Error | undefined;
      const fail = (error: Error) => { failure ??= error; try { tx.abort(); } catch { reject(failure); } };
      tx.oncomplete = () => failure ? reject(failure) : resolve(result);
      tx.onerror = () => { failure ??= problem(tx.error); };
      tx.onabort = () => reject(failure ?? problem(tx.error));
      try { action(tx.objectStore('traces'), tx.objectStore('summaries'), value => { result = value; }, fail, tx.objectStore('habitats')); }
      catch (e) { fail(e as Error); }
    });
  }
  async save(trace: Trace, expected?: SavedReference): Promise<SavedReference> {
    await this.open(); // Origin error before secure-context-only randomUUID.
    const key = expected?.key ?? crypto.randomUUID();
    return this.transaction('readwrite', (traces, metas, done, fail) => {
      const req = traces.get(key);
      req.onsuccess = () => {
        try {
          const previous: SavedTrace | undefined = req.result;
          if (expected && !previous) throw missing();
          if (expected && previous!.savedAt !== expected.savedAt) throw changed();
          if (!expected && previous) throw new Error('A saved trace identifier collided. Try saving again.');
          const savedAt = this.nextTime(previous?.savedAt), item: SavedTrace = { key, savedAt, trace };
          traces.put(item); metas.put(summary(item)); done({ key, savedAt });
        } catch (e) { fail(e as Error); }
      };
    });
  }
  private nextTime(previous?: string): string {
    const last = previous ? Date.parse(previous) : 0;
    return new Date(Math.max(Date.now(), Number.isFinite(last) ? last + 1 : 0)).toISOString();
  }
  async list(): Promise<SavedSummary[]> {
    return this.transaction('readonly', (_traces, metas, done) => {
      const req = metas.getAll();
      req.onsuccess = () => done((req.result as SavedSummary[]).sort((a, b) => b.savedAt.localeCompare(a.savedAt)));
    });
  }
  async getEntry(key: string): Promise<SavedTrace> {
    return this.transaction('readonly', (traces, _metas, done, fail) => {
      const req = traces.get(key); req.onsuccess = () => req.result ? done(req.result) : fail(missing());
    });
  }
  async get(key: string): Promise<Trace> { return (await this.getEntry(key)).trace; }
  async rename(expected: SavedReference, name: string): Promise<SavedReference> {
    const trimmed = name.trim(); if (!trimmed || trimmed.length > 200) throw new Error('Use a name between 1 and 200 characters.');
    return this.transaction('readwrite', (traces, metas, done, fail) => {
      const req = traces.get(expected.key);
      req.onsuccess = () => {
        try {
          if (!req.result) throw missing();
          const item: SavedTrace = req.result;
          if (item.savedAt !== expected.savedAt) throw changed();
          item.trace.name = trimmed; item.savedAt = this.nextTime(item.savedAt);
          traces.put(item); metas.put(summary(item)); done({ key: item.key, savedAt: item.savedAt });
        } catch (e) { fail(e as Error); }
      };
    });
  }
  async delete(expected: SavedReference): Promise<void> {
    await this.transaction<void>('readwrite', (traces, metas, done, fail) => {
      const req = traces.get(expected.key);
      req.onsuccess = () => {
        if (req.result && req.result.savedAt !== expected.savedAt) { fail(changed()); return; }
        traces.delete(expected.key); metas.delete(expected.key); done(undefined);
      };
    });
  }
  async clear(): Promise<void> {
    await this.transaction<void>('readwrite', (traces, metas, done) => { traces.clear(); metas.clear(); done(undefined); });
  }
  async getHabitat(): Promise<HabitatEntry | undefined> {
    return this.transaction('readonly', (_traces, _metas, done, _fail, habitats) => {
      const request = habitats.get('pet'); request.onsuccess = () => done(request.result);
    });
  }
  async saveHabitat(habitat: SavedHabitat, expectedRevision?: number): Promise<number> {
    // Freeze the whole recording/checkpoint together before the first IDB await.
    // A live world's arrays may otherwise advance while the transaction opens.
    habitat = structuredClone(habitat);
    return this.transaction('readwrite', (_traces, _metas, done, fail, habitats) => {
      const request = habitats.get('pet');
      request.onsuccess = () => {
        const previous: HabitatEntry | undefined = request.result;
        if (previous?.revision !== expectedRevision) {
          fail(new Error('Another pet tab saved this habitat. Save a replay file to keep this version, or reload the saved habitat.')); return;
        }
        const revision = (previous?.revision ?? 0) + 1;
        habitats.put({ revision, habitat }, 'pet'); done(revision);
      };
    });
  }
}
