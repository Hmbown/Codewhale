// Checked by npm test (or npm run test:types).
import { CodeWhaleRuntimeClient, isThreadStreamEnd } from '../index.js';
import type { ThreadRuntimeEvent, ThreadStreamEnd } from '../index.js';

async function checkThreadEvents(client: CodeWhaleRuntimeClient, includeProgress: boolean) {
  for await (const item of client.threadEvents('thread')) {
    if (isThreadStreamEnd(item)) {
      const end: ThreadStreamEnd = item;
      const cursor: number = item.last_seq;
      const retryable: boolean = item.retryable;
      if (retryable) client.threadEvents(end.thread_id, { sinceSeq: cursor });
    } else {
      const journal: ThreadRuntimeEvent = item;
      journal.payload;
    }
  }
  for await (const item of client.threadEvents('thread', { includeProgress: true })) {
    if (isThreadStreamEnd(item)) {
      const end: ThreadStreamEnd = item;
      end.last_seq;
      end.retryable;
    } else {
      const cursor: number = item.seq;
      client.threadEvents(item.thread_id, { sinceSeq: cursor });
    }
  }
  for await (const item of client.threadEvents('thread', { includeProgress })) {
    if (isThreadStreamEnd(item)) {
      const end: ThreadStreamEnd = item;
      end.last_seq;
      end.retryable;
    }
  }
}

// Future journal names must remain accepted without closing the protocol union.
const future: ThreadRuntimeEvent = {
  event: 'future.journal.event', seq: 1, thread_id: 'thread', timestamp: '', payload: {},
};
isThreadStreamEnd(future);
void checkThreadEvents;
