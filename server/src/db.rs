use anyhow::Result;
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::auth::{generate_token, hash_token};

pub async fn connect(database_url: &str) -> Result<PgPool> {
    Ok(PgPoolOptions::new()
        .max_connections(20)
        .connect(database_url)
        .await?)
}

pub async fn migrate(pool: &PgPool) -> Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

pub async fn seed_api_key_if_missing(pool: &PgPool) -> Result<()> {
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM api_keys WHERE revoked_at IS NULL")
            .fetch_one(pool)
            .await?;
    if count > 0 {
        return Ok(());
    }
    let key = generate_token();
    sqlx::query("INSERT INTO api_keys (key_hash, label) VALUES ($1, $2)")
        .bind(hash_token(&key))
        .bind("bootstrap")
        .execute(pool)
        .await?;
    eprintln!();
    eprintln!("============================================================");
    eprintln!("Generated API key (save this, will not be shown again):");
    eprintln!();
    eprintln!("    {}", key);
    eprintln!();
    eprintln!("Use as:  X-API-Key: <key>");
    eprintln!("============================================================");
    eprintln!();
    Ok(())
}

pub async fn seed_enrollment_secret_if_missing(pool: &PgPool) -> Result<()> {
    let (val,): (Value,) =
        sqlx::query_as("SELECT value FROM settings WHERE key = 'enrollment_secret'")
            .fetch_one(pool)
            .await?;
    let current = val.as_str().unwrap_or("");
    if !current.is_empty() {
        return Ok(());
    }
    let secret = generate_token();
    sqlx::query(
        "UPDATE settings SET value = $1, updated_at = now() WHERE key = 'enrollment_secret'"
    )
    .bind(serde_json::json!(secret))
    .execute(pool)
    .await?;
    eprintln!();
    eprintln!("============================================================");
    eprintln!("Generated enrollment secret (put into agent.toml):");
    eprintln!();
    eprintln!("    {}", secret);
    eprintln!();
    eprintln!("============================================================");
    eprintln!();
    Ok(())
}
