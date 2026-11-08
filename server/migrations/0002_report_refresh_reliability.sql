ALTER TABLE systems
    ADD COLUMN IF NOT EXISTS force_refresh_generation bigint NOT NULL DEFAULT 0;

UPDATE settings
SET value = '180'::jsonb, updated_at = now()
WHERE key = 'heartbeat_interval_secs';

UPDATE settings
SET value = '86400'::jsonb, updated_at = now()
WHERE key = 'report_interval_secs';
