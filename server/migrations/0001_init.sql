-- ============================================================
-- PAK Monitor — initial schema
-- Postgres 13+ (gen_random_uuid is built-in)
-- ============================================================

-- ---------- settings ----------
CREATE TABLE settings (
    key        text PRIMARY KEY,
    value      jsonb NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now()
);

INSERT INTO settings (key, value) VALUES
    ('timezone',                '"Asia/Tehran"'),
    ('offline_threshold_secs',  '90'),
    ('heartbeat_interval_secs', '30'),
    ('report_interval_secs',    '300'),
    ('enrollment_secret',       '""')
ON CONFLICT (key) DO NOTHING;

-- ---------- agents ----------
CREATE TABLE agents (
    agent_id      uuid PRIMARY KEY,
    token_hash    text NOT NULL,
    computer_name text,
    enrolled_at   timestamptz NOT NULL DEFAULT now(),
    last_seen     timestamptz,
    revoked_at    timestamptz
);

CREATE INDEX agents_active_idx ON agents (agent_id) WHERE revoked_at IS NULL;

-- ---------- systems ----------
CREATE TABLE systems (
    agent_id            uuid PRIMARY KEY REFERENCES agents(agent_id) ON DELETE CASCADE,
    computer_name       text NOT NULL,
    fqdn                text,
    domain              text,
    ip_addresses        inet[],
    mac_addresses       macaddr[],
    platform            text,
    console_user        text,

    os_name             text,
    os_version          text,
    os_build            text,
    os_arch             text,

    cpu_model           text,
    cpu_cores           int,
    ram_bytes           bigint,
    system_type         text,            -- Desktop | VM
    motherboard         text,

    drives              jsonb,           -- [{letter,label,fs,free,total,is_system}]
    disk_count          int,
    system_drive_free   bigint,
    system_drive_total  bigint,
    main_storage_size   bigint,

    installed_software  jsonb,           -- [{name,version,publisher,install_date}]
    software_count      int,

    updates             jsonb,           -- [{kb,description,installed_on}]
    update_count        int,

    last_seen           timestamptz,
    reported_at         timestamptz,
    maintenance         boolean NOT NULL DEFAULT false,
    maintenance_since   timestamptz,
    force_refresh       boolean NOT NULL DEFAULT false,
    raw_json            jsonb,
    created_at          timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX systems_hostname_idx     ON systems (lower(computer_name));
CREATE INDEX systems_last_seen_idx    ON systems (last_seen DESC);
CREATE INDEX systems_domain_idx       ON systems (domain);
CREATE INDEX systems_maintenance_idx  ON systems (maintenance);
CREATE INDEX systems_ip_gin           ON systems USING gin (ip_addresses);

-- ---------- notes ----------
CREATE TABLE system_notes (
    id         bigserial PRIMARY KEY,
    agent_id   uuid NOT NULL REFERENCES systems(agent_id) ON DELETE CASCADE,
    body       text NOT NULL,
    author     text,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX system_notes_agent_idx ON system_notes (agent_id, created_at DESC);

-- ---------- external API keys ----------
CREATE TABLE api_keys (
    key_hash    text PRIMARY KEY,
    label       text NOT NULL,
    created_at  timestamptz NOT NULL DEFAULT now(),
    last_used   timestamptz,
    revoked_at  timestamptz
);

-- ---------- convenience view (current snapshot only) ----------
-- Nothing fancy, just a single source of truth for the dashboard's grid.
CREATE VIEW systems_current AS
SELECT
    s.*,
    CASE
        WHEN s.maintenance THEN 'maintenance'
        WHEN s.last_seen IS NULL THEN 'never'
        WHEN s.last_seen < now() - (
            (SELECT (value)::text::int FROM settings WHERE key='offline_threshold_secs') * interval '1 second'
        ) THEN 'offline'
        ELSE 'online'
    END AS live_state,
    (
        (SELECT (value)::text::int FROM settings WHERE key='offline_threshold_secs') * interval '1 second'
    ) AS offline_threshold
FROM systems s;
