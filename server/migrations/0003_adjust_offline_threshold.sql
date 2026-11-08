UPDATE settings
SET value = '600'::jsonb, updated_at = now()
WHERE key = 'offline_threshold_secs';
