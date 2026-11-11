CREATE TABLE system_peripherals (
    id bigserial PRIMARY KEY,
    agent_id uuid NOT NULL REFERENCES systems(agent_id) ON DELETE CASCADE,
    kind text NOT NULL,
    name text NOT NULL,
    details jsonb NOT NULL
);

CREATE INDEX system_peripherals_agent_idx ON system_peripherals (agent_id);
CREATE INDEX system_peripherals_kind_agent_idx ON system_peripherals (kind, agent_id);
CREATE INDEX system_peripherals_name_prefix_idx ON system_peripherals (lower(name) text_pattern_ops);

CREATE FUNCTION replace_system_peripherals(target_agent uuid, inventory jsonb)
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
        WHERE item->>'kind' IN ('monitor', 'keyboard', 'mouse', 'imaging')
    ) AS items
    WHERE jsonb_typeof(items.item) = 'object'
      AND jsonb_typeof(items.item->'name') = 'string'
      AND length(btrim(items.item->>'name')) > 0;
END;
$$;

SELECT replace_system_peripherals(agent_id, raw_json->'peripherals')
FROM systems
WHERE jsonb_typeof(raw_json->'peripherals') = 'object';
