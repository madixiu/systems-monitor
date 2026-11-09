ALTER TABLE systems
    ADD COLUMN IF NOT EXISTS uptime_secs bigint,
    ADD COLUMN IF NOT EXISTS last_boot timestamptz,
    ADD COLUMN IF NOT EXISTS pending_reboot boolean,
    ADD COLUMN IF NOT EXISTS clock_skew_secs bigint,
    ADD COLUMN IF NOT EXISTS gpu_model text,
    ADD COLUMN IF NOT EXISTS windows_activation text;
