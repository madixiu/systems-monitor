CREATE OR REPLACE FUNCTION replace_system_peripherals(target_agent uuid, inventory jsonb)
RETURNS void LANGUAGE plpgsql AS $$
BEGIN
    DELETE FROM system_peripherals WHERE agent_id = target_agent;

    INSERT INTO system_peripherals (agent_id, kind, name, details)
    SELECT target_agent, items.kind, items.item->>'name', items.item
    FROM (
        SELECT 'printer'::text AS kind, item
        FROM jsonb_array_elements(
            CASE WHEN jsonb_typeof(inventory->'printers') = 'array'
                 THEN inventory->'printers' ELSE '[]'::jsonb END
        ) AS item
        UNION ALL
        SELECT item->>'kind', item
        FROM jsonb_array_elements(
            CASE WHEN jsonb_typeof(inventory->'devices') = 'array'
                 THEN inventory->'devices' ELSE '[]'::jsonb END
        ) AS item
        WHERE item->>'kind' IN ('monitor', 'imaging')
    ) AS items
    WHERE jsonb_typeof(items.item) = 'object'
      AND jsonb_typeof(items.item->'name') = 'string'
      AND length(btrim(items.item->>'name')) > 0;
END;
$$;

-- Start fresh: agents repopulate peripherals on their next full report.
DELETE FROM system_peripherals;
UPDATE systems SET raw_json = raw_json - 'peripherals' WHERE raw_json ? 'peripherals';
