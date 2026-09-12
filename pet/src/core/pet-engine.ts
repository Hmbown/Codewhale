import type { Category, WhaleEvent } from './model.js';
import { toolCategory } from './codewhale.js';
import { compilePetTelemetry, PET_BIN_MS, type PetBucket } from './pet-telemetry.js';

/** Read-only adapter for codewhale_protocol::EventMsg metadata. The foreground
 * Engine is the event owner. This replaces no turn loop: it only translates
 * lifecycle observations to event-v1 for the same pet bucketer used by imports.
 * Text, inputs and results are neither accepted nor retained. */
export class PetEngineTelemetry {
  private events: WhaleEvent[] = [];
  private active = new Map<string, WhaleEvent>();
  private waiting: WhaleEvent | undefined;
  private sequence = 0;
  private lastTime = 0;

  private add(name: string, category: Category, at: number, agentId = 'parent', continuation = false): WhaleEvent {
    if (this.events.length >= 8192) throw new Error('Pet Engine observation window is full.');
    const e: WhaleEvent = { schemaVersion: 1, id: `engine:${this.sequence++}`, traceId: 'foreground',
      startTime: at, endTime: at, name, category, agentId, status: 'running',
      attributes: continuation ? { 'whalesong.continuation': true } : {} };
    this.events.push(e); return e;
  }

  private pulse(key: string, at: number): void {
    const e = this.active.get(key);
    if (!e) return;
    // A resumed stream does not assert coverage across its silent interval.
    if (at - e.endTime > PET_BIN_MS * 2) {
      this.active.set(key, this.add(e.name, e.category, at, e.agentId, true));
    } else e.endTime = at;
  }

  observe(value: unknown, at: number): void {
    if (!Number.isFinite(at) || at < this.lastTime || at > 86_400_000) throw new Error('Invalid Engine pet clock.');
    if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid Engine pet metadata.');
    const e = value as Record<string, unknown>;
    const allowed = ['event', 'index', 'channel', 'tool_call_id', 'tool_name', 'id', 'worker_status', 'failed'];
    if (Object.keys(e).some(k => !allowed.includes(k)) || typeof e.event !== 'string'
      || Object.values(e).some(v => typeof v === 'string' && v.length > 4096)
      || e.channel !== undefined && !['text', 'reasoning'].includes(e.channel as string)
      || ['tool_call_id', 'tool_name', 'id', 'worker_status'].some(k => e[k] !== undefined && typeof e[k] !== 'string')
      || e.failed !== undefined && typeof e.failed !== 'boolean'
      || e.index !== undefined && (!Number.isSafeInteger(e.index) || (e.index as number) < 0))
      throw new Error('Invalid Engine pet metadata fields.');
    this.lastTime = at;
    this.events = this.events.filter(span => span.endTime >= at - 12_800);
    const id = (field: string) => { const s = e[field]; if (typeof s !== 'string' || !s) throw new Error(`Missing Engine ${field}.`); return s; };
    const index = () => { if (!Number.isSafeInteger(e.index)) throw new Error('Missing Engine index.'); return String(e.index); };
    const start = (key: string, name: string, category: Category, agentId?: string) => {
      if (this.active.size >= 256 && !this.active.has(key)) throw new Error('Too many active Engine pet spans.');
      this.active.set(key, this.add(name, category, at, agentId));
    };
    const finish = (key: string) => { this.pulse(key, at); this.active.delete(key); };
    switch (e.event) {
      case 'turn_started': this.active.clear(); this.waiting = undefined; break;
      case 'message_started': start(`message:${index()}`, 'assistant_message', 'communication'); this.waiting = undefined; break;
      case 'thinking_started': start(`thinking:${index()}`, 'thinking', 'reasoning'); this.waiting = undefined; break;
      case 'response_delta': {
        const reasoning = e.channel === 'reasoning';
        const key = `${reasoning ? 'thinking' : 'message'}:${index()}`;
        if (!this.active.has(key)) start(key, reasoning ? 'thinking' : 'assistant_message', reasoning ? 'reasoning' : 'communication');
        else this.pulse(key, at);
        this.waiting = undefined; break;
      }
      case 'message_complete': finish(`message:${index()}`); break;
      case 'thinking_complete': finish(`thinking:${index()}`); break;
      case 'tool_call_started': start(`tool:${id('tool_call_id')}`, id('tool_name'), toolCategory(id('tool_name'))); this.waiting = undefined; break;
      case 'tool_call_heartbeat': for (const key of this.active.keys()) if (key.startsWith('tool:')) this.pulse(key, at); break;
      case 'tool_call_complete': finish(`tool:${id('tool_call_id')}`); this.waiting = undefined; break;
      case 'agent_spawned': start(`agent:${id('id')}`, 'agent', 'agent', id('id')); break;
      case 'agent_progress': {
        const key = `agent:${id('id')}`;
        if (['completed', 'failed', 'cancelled', 'interrupted', 'budget_exhausted'].includes(e.worker_status as string)) { finish(key); break; }
        if (!this.active.has(key)) start(key, 'agent', 'agent', id('id')); else this.pulse(key, at);
        break;
      }
      case 'agent_complete': finish(`agent:${id('id')}`); break;
      case 'approval_required': case 'user_input_required': this.waiting = this.add('human', 'human', at); this.waiting.status = 'pending'; break;
      case 'turn_complete': this.active.clear(); this.waiting = undefined; break;
      case 'error': break;
      default: throw new Error('Unsupported Engine pet event.');
    }
    // Receipt time is the error onset; never rewrite the operation's old start.
    if (e.event === 'error' || e.failed === true) this.add('error', 'error', at).status = 'error';
  }

  /** Waiting coverage comes from the existing typed shell's current request.
   * It can extend a witnessed request, never invent one on a mid-turn attach. */
  confirmWaiting(at: number, waiting: boolean): void {
    if (!waiting) { this.waiting = undefined; return; }
    if (this.waiting && at >= this.waiting.endTime) this.waiting.endTime = at;
  }

  bucket(sequence: number): PetBucket {
    const end = (sequence + 1) * PET_BIN_MS;
    const input = this.events.filter(e => e.startTime < end && e.endTime >= end - 12_400)
      .map(e => ({ ...e, endTime: Math.min(e.endTime, end) }));
    return compilePetTelemetry(input, end, sequence)[0];
  }
}
