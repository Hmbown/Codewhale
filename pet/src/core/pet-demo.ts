import type { Category, WhaleEvent } from './model.js';

/** Authored event-v1 fixture, explicitly labelled demo by every driver. */
export function petDemoEvents(): WhaleEvent[] {
  const out: WhaleEvent[] = [];
  const span = (startTime: number, endTime: number, category: Category, name: string, agentId = 'parent', status: WhaleEvent['status'] = 'success') => {
    out.push({ schemaVersion: 1, id: `demo-${out.length}`, traceId: 'pet-demo', startTime, endTime,
      category, name, agentId, status, attributes: {} });
  };
  span(0, 6000, 'reasoning', 'Plan');
  for (let i = 0; i < 5; i++) span(6400 + i * 800, 6900 + i * 800, 'filesystem', `Read source ${i}`);
  span(11_000, 16_000, 'code', 'Check implementation');
  for (const [i, agent] of ['parent', 'reviewer', 'tester'].entries()) span(17_000 + i * 400, 25_000, 'reasoning', 'Parallel work', agent);
  for (let i = 0; i < 7; i++) span(26_000 + i * 700, 26_450 + i * 700, 'tool', 'Repeated tool');
  for (let i = 0; i < 3; i++) span(31_000 + i * 400, 31_000 + i * 400, 'error', 'Reported failure', 'parent', 'error');
  span(34_000, 63_000, 'human', 'Awaiting input', 'parent', 'pending');
  span(63_000, 63_000, 'human', 'User response');
  span(64_000, 69_000, 'memory', 'Recall');
  span(70_000, 75_000, 'reasoning', 'Resolve');
  return out;
}
