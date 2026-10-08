m.scope IN (SELECT value FROM json_each(:scopes))
AND NOT EXISTS (SELECT 1 FROM memory_preferences mp WHERE mp.memory_id=m.id AND mp.suppressed=1)
AND (m.status='active' OR (:include_stale=1 AND m.status='stale'))
AND (:include_stale=1 OR (
    (m.expires_at IS NULL OR m.expires_at>:now)
    AND (json_extract(m.payload,'$.valid_from') IS NULL OR json_extract(m.payload,'$.valid_from')<=:now)
    AND (json_extract(m.payload,'$.valid_until') IS NULL OR json_extract(m.payload,'$.valid_until')>:now)
    AND NOT EXISTS (
        SELECT 1 FROM dependencies d
        WHERE d.memory_id=m.id AND NOT EXISTS (
            SELECT 1 FROM json_each(:files) f
            WHERE f.key=d.path AND f.value=d.digest
        )
    )
    AND NOT EXISTS (
        WITH RECURSIVE ancestors(id) AS (
            SELECT parent_id FROM lineage WHERE child_id=m.id
            UNION
            SELECT l.parent_id FROM lineage l JOIN ancestors a ON l.child_id=a.id
        )
        SELECT 1 FROM ancestors a JOIN memories p ON p.id=a.id
        WHERE p.status!='active'
            OR json_extract(p.payload,'$.valid_from')>:now
            OR json_extract(p.payload,'$.valid_until')<=:now
            OR (p.expires_at IS NOT NULL AND p.expires_at<=:now)
            OR NOT EXISTS (SELECT 1 FROM json_each(:scopes) g WHERE g.value=p.scope)
            OR EXISTS (
                SELECT 1 FROM dependencies d WHERE d.memory_id=p.id
                AND NOT EXISTS (SELECT 1 FROM json_each(:files) f WHERE f.key=d.path AND f.value=d.digest)
            )
            OR (p.repo_revision IS NOT NULL AND (p.repo_revision!=:repo OR :repo IS NULL)
                AND NOT EXISTS (SELECT 1 FROM dependencies d WHERE d.memory_id=p.id))
    )
    AND (
        m.repo_revision IS NULL OR m.repo_revision=:repo
        OR EXISTS(SELECT 1 FROM dependencies d WHERE d.memory_id=m.id)
    )
))
