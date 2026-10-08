WITH RECURSIVE edges(parent,child) AS (
    SELECT parent_id,child_id FROM lineage
    UNION SELECT old_id,new_id FROM replacements
    UNION SELECT new_id,old_id FROM replacements
), forgotten(id) AS (
    SELECT :id
    UNION SELECT e.child FROM edges e JOIN forgotten f ON e.parent=f.id
)
SELECT id FROM forgotten ORDER BY id
