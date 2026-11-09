ALTER TABLE systems
    ADD COLUMN IF NOT EXISTS console_user_logged_in boolean;
