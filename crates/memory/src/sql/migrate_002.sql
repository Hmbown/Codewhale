-- Apply only to application_id 1129794866, user_version 1, in an IMMEDIATE tx.
-- Source text remains immutable. All new timeline records are metadata-only.
CREATE TABLE memory_observer (
    singleton INTEGER PRIMARY KEY CHECK(singleton=1),
    producer TEXT NOT NULL,
    epoch TEXT NOT NULL
);
INSERT INTO memory_observer VALUES(1,lower(hex(randomblob(16))),lower(hex(randomblob(16))));
CREATE TABLE memory_preferences (
    memory_id TEXT PRIMARY KEY REFERENCES memories(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision>0),
    pinned INTEGER NOT NULL DEFAULT 0 CHECK(pinned IN(0,1)),
    suppressed INTEGER NOT NULL DEFAULT 0 CHECK(suppressed IN(0,1))
);
CREATE TABLE memory_history (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    memory_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL,
    status TEXT NOT NULL,
    recorded_at INTEGER NOT NULL,
    UNIQUE(memory_id,revision)
);
-- Old installations have only their current version, not fabricated history.
INSERT INTO memory_history(memory_id,revision,status,recorded_at)
SELECT id,revision,status,updated_at FROM memories;
CREATE TABLE memory_outbox (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    scope TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    action TEXT NOT NULL,
    recorded_at INTEGER NOT NULL,
    trace_id TEXT,
    context_id TEXT,
    units INTEGER,
    unit TEXT,
    reason_code TEXT
);
CREATE INDEX memory_outbox_scope ON memory_outbox(scope,seq);
CREATE TRIGGER memory_history_insert AFTER INSERT ON memories BEGIN
    INSERT INTO memory_history(memory_id,revision,status,recorded_at)
    VALUES(new.id,new.revision,new.status,new.updated_at);
END;
CREATE TRIGGER memory_history_update AFTER UPDATE OF revision ON memories
WHEN new.revision<>old.revision BEGIN
    INSERT INTO memory_history(memory_id,revision,status,recorded_at)
    VALUES(new.id,new.revision,new.status,new.updated_at);
END;
-- Existing audited operations and their outbox entries commit together.
CREATE TRIGGER memory_event_outbox AFTER INSERT ON events
WHEN EXISTS(SELECT 1 FROM memories WHERE id=new.entity_id) BEGIN
    INSERT INTO memory_outbox(scope,entity_id,revision,action,recorded_at)
    SELECT scope,new.entity_id,new.revision,new.action,new.at FROM memories WHERE id=new.entity_id;
END;
CREATE TABLE memory_contexts (
    id TEXT PRIMARY KEY,
    scope TEXT NOT NULL,
    trace_id TEXT NOT NULL,
    request_key TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    packet_hash TEXT NOT NULL,
    stage TEXT NOT NULL CHECK(stage IN('prepared','appended','dispatched')),
    created_at INTEGER NOT NULL,
    appended_at INTEGER,
    dispatched_at INTEGER,
    history_sequence INTEGER,
    transport_key TEXT,
    used_units INTEGER NOT NULL CHECK(used_units>=0),
    unit TEXT NOT NULL,
    omitted INTEGER NOT NULL CHECK(omitted>=0),
    UNIQUE(scope,trace_id,request_key)
);
CREATE TABLE memory_context_refs (
    context_id TEXT NOT NULL REFERENCES memory_contexts(id) ON DELETE CASCADE,
    memory_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY(context_id,memory_id)
);
CREATE INDEX memory_context_refs_memory ON memory_context_refs(memory_id);
-- Forget removes the whole dependent context, not just one index row.
CREATE TRIGGER forget_context BEFORE DELETE ON memories BEGIN
    DELETE FROM memory_contexts WHERE id IN
      (SELECT context_id FROM memory_context_refs WHERE memory_id=old.id);
END;
CREATE TABLE memory_hook_receipts (
    scope TEXT NOT NULL,
    event_id TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    recorded_at INTEGER NOT NULL,
    PRIMARY KEY(scope,event_id)
);
PRAGMA user_version=2;
-- Compatibility IDs are stable and never reused after forgetting. The advanced
-- API uses UUIDs; existing numeric memory_get callers go through this alias.
CREATE TABLE memory_numeric_aliases (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    memory_id TEXT UNIQUE REFERENCES memories(id) ON DELETE SET NULL
);
INSERT INTO memory_numeric_aliases(memory_id) SELECT id FROM memories ORDER BY rowid;
CREATE TRIGGER memory_numeric_insert AFTER INSERT ON memories BEGIN
    INSERT INTO memory_numeric_aliases(memory_id) VALUES(new.id);
END;
