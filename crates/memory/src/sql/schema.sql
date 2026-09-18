-- New, authoritative v2 store. NEVER apply this to native index.sqlite3.
CREATE TABLE memories (
    rowid INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    scope TEXT NOT NULL,
    kind TEXT NOT NULL,
    semantic_key TEXT,
    status TEXT NOT NULL CHECK(status IN ('candidate','active','stale','superseded','rejected')),
    revision INTEGER NOT NULL CHECK(revision > 0),
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    tags TEXT NOT NULL,
    payload TEXT NOT NULL CHECK(json_valid(payload)),
    content_hash TEXT NOT NULL,
    repo_revision TEXT,
    importance REAL NOT NULL CHECK(importance BETWEEN 0 AND 1),
    confidence REAL NOT NULL CHECK(confidence BETWEEN 0 AND 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    expires_at INTEGER
);
CREATE INDEX memories_scope_status ON memories(scope,status,updated_at);
CREATE UNIQUE INDEX one_active_key ON memories(scope,semantic_key)
    WHERE status='active' AND semantic_key IS NOT NULL;
CREATE TABLE dependencies (
    memory_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    digest TEXT NOT NULL,
    PRIMARY KEY(memory_id,path)
);
CREATE VIRTUAL TABLE memory_fts USING fts5(title,body,tags, content='memories',content_rowid='rowid',tokenize='unicode61 remove_diacritics 2');
CREATE VIRTUAL TABLE memory_grams USING fts5(title,body, content='memories',content_rowid='rowid',tokenize='trigram');
INSERT INTO memory_fts(memory_fts,rank) VALUES('secure-delete',1);
INSERT INTO memory_grams(memory_grams,rank) VALUES('secure-delete',1);
CREATE TRIGGER memory_insert AFTER INSERT ON memories BEGIN
    INSERT INTO memory_fts(rowid,title,body,tags) VALUES(new.rowid,new.title,new.body,new.tags);
    INSERT INTO memory_grams(rowid,title,body) VALUES(new.rowid,new.title,new.body);
END;
CREATE TRIGGER memory_delete AFTER DELETE ON memories BEGIN
    INSERT INTO memory_fts(memory_fts,rowid,title,body,tags) VALUES('delete',old.rowid,old.title,old.body,old.tags);
    INSERT INTO memory_grams(memory_grams,rowid,title,body) VALUES('delete',old.rowid,old.title,old.body);
END;
-- Text is immutable. Corrections create a replacement, never edit in place.
CREATE TRIGGER immutable_memory_text BEFORE UPDATE OF title,body,tags,payload,scope,kind,semantic_key,content_hash,repo_revision,expires_at,importance,confidence,created_at ON memories BEGIN
    SELECT RAISE(ABORT,'memory content is immutable; create a revision');
END;
CREATE TABLE requests (
    scope TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    draft_hash TEXT NOT NULL,
    memory_id TEXT REFERENCES memories(id) ON DELETE SET NULL,
    PRIMARY KEY(scope,request_hash)
);
CREATE TABLE tombstones (
    scope TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    forgotten_at INTEGER NOT NULL,
    PRIMARY KEY(scope,content_hash)
);
CREATE TABLE events (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_id TEXT NOT NULL,
    action TEXT NOT NULL,
    actor_hash TEXT NOT NULL,
    revision INTEGER NOT NULL,
    at INTEGER NOT NULL
);
-- Events intentionally contain NO memory text, URI, free-form reason, or payload.
CREATE TABLE validations (
    memory_id TEXT PRIMARY KEY REFERENCES memories(id) ON DELETE CASCADE,
    receipt TEXT NOT NULL CHECK(json_valid(receipt))
);
CREATE TABLE lineage (
    parent_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    child_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    PRIMARY KEY(parent_id,child_id), CHECK(parent_id != child_id)
);
CREATE INDEX lineage_child ON lineage(child_id);
CREATE TABLE replacements (
    old_id TEXT PRIMARY KEY REFERENCES memories(id) ON DELETE CASCADE,
    new_id TEXT NOT NULL UNIQUE REFERENCES memories(id) ON DELETE CASCADE,
    CHECK(old_id != new_id)
);
CREATE TABLE links (
    from_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    to_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    relation TEXT NOT NULL CHECK(relation IN ('related','supports','contradicts','decision_outcome')),
    PRIMARY KEY(from_id,to_id,relation), CHECK(from_id != to_id)
);
CREATE INDEX links_to ON links(to_id);
CREATE TABLE embeddings (
    memory_id TEXT PRIMARY KEY REFERENCES memories(id) ON DELETE CASCADE,
    model TEXT NOT NULL,
    dimensions INTEGER NOT NULL CHECK(dimensions BETWEEN 1 AND 8192),
    content_hash TEXT NOT NULL,
    vector BLOB NOT NULL,
    CHECK(length(vector) = dimensions * 4)
);
CREATE INDEX embedding_model ON embeddings(model,dimensions);
CREATE TABLE checkpoints (
    id TEXT PRIMARY KEY,
    scope TEXT NOT NULL,
    checkpoint_key TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    payload TEXT NOT NULL CHECK(json_valid(payload)),
    updated_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    UNIQUE(scope,checkpoint_key)
);
CREATE TABLE checkpoint_refs (
    checkpoint_id TEXT NOT NULL REFERENCES checkpoints(id) ON DELETE CASCADE,
    memory_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    PRIMARY KEY(checkpoint_id,memory_id)
);
CREATE INDEX checkpoint_refs_memory ON checkpoint_refs(memory_id);
PRAGMA application_id=1129794866;
PRAGMA user_version=1;
