/**
 * Test double for the DRAFT_CLAIM_LOCK namespace. Each name gets its own
 * storage, and each object runs one `act` at a time to completion, which is
 * the guarantee a Durable Object's input gate gives the real class. Storage
 * calls still yield to the event loop, so callers genuinely overlap.
 */
import {
  applyDraftLock,
  type DraftClaimLockId,
  type DraftClaimLockNamespace,
  type DraftClaimLockStub,
  type DraftLockRequest,
  type DraftLockResponse,
  type DraftLockStorage,
} from "./draft-claim-lock";

class MapStorage implements DraftLockStorage {
  readonly values = new Map<string, unknown>();
  private async tick() {
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  async get<T>(key: string) {
    await this.tick();
    return this.values.get(key) as T | undefined;
  }
  async put<T>(key: string, value: T) {
    await this.tick();
    this.values.set(key, structuredClone(value));
  }
  async delete(key: string) {
    await this.tick();
    return this.values.delete(key);
  }
}

class FakeObject implements DraftClaimLockStub {
  readonly storage = new MapStorage();
  private queue: Promise<unknown> = Promise.resolve();
  constructor(private readonly clock: () => number, private readonly calls: DraftLockRequest[]) {}

  act(req: DraftLockRequest): Promise<DraftLockResponse> {
    this.calls.push(req);
    const run = this.queue.then(() => applyDraftLock(this.storage, this.clock(), req));
    this.queue = run.catch(() => undefined);
    return run;
  }
}

export class FakeDraftClaimLock implements DraftClaimLockNamespace {
  now = 1_700_000_000_000;
  readonly calls: DraftLockRequest[] = [];
  private readonly objects = new Map<string, FakeObject>();

  idFromName(name: string): DraftClaimLockId {
    return { toString: () => name };
  }

  get(id: DraftClaimLockId): FakeObject {
    const name = id.toString();
    let obj = this.objects.get(name);
    if (!obj) {
      obj = new FakeObject(() => this.now, this.calls);
      this.objects.set(name, obj);
    }
    return obj;
  }
}
