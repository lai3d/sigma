use chrono::NaiveDate;
use redis::AsyncCommands;
use serde::Serialize;
use uuid::Uuid;

use crate::config::Config;
use crate::db::Db;
use crate::models::IpEntry;

/// Background worker that checks for expiring VPS and sends notifications.
pub async fn run(
    db: Db,
    redis: redis::aio::ConnectionManager,
    http_client: reqwest::Client,
    cfg: Config,
) {
    let interval = std::time::Duration::from_secs(cfg.notify_interval_secs);
    let thresholds = &cfg.notify_before_days;

    tracing::info!(
        "Notification worker started (interval={}s, thresholds={:?})",
        cfg.notify_interval_secs,
        thresholds,
    );

    loop {
        if let Err(e) = check_and_notify(&db, &redis, &http_client, &cfg, thresholds).await {
            tracing::error!("Notification check error: {e}");
        }
        tokio::time::sleep(interval).await;
    }
}

async fn check_and_notify(
    db: &Db,
    redis: &redis::aio::ConnectionManager,
    http_client: &reqwest::Client,
    cfg: &Config,
    thresholds: &[i32],
) -> anyhow::Result<()> {
    let max_days = thresholds.iter().copied().max().unwrap_or(7);

    let rows = sqlx::query_as::<_, ExpiringVps>(
        r#"SELECT
            v.id, v.hostname, v.alias, v.ip_addresses, v.country,
            v.expire_date, v.status,
            COALESCE(p.name, '') as provider_name
           FROM vps v
           LEFT JOIN providers p ON p.id = v.provider_id
           WHERE v.expire_date IS NOT NULL
             AND v.expire_date <= CURRENT_DATE + $1 * INTERVAL '1 day'
             AND v.expire_date >= CURRENT_DATE
             AND v.status IN ('active', 'provisioning')
           ORDER BY v.expire_date ASC"#,
    )
    .bind(max_days)
    .fetch_all(db)
    .await?;

    if rows.is_empty() {
        tracing::debug!("No expiring VPS found within {max_days} days");
        return Ok(());
    }

    let today = chrono::Utc::now().date_naive();
    let mut conn = redis.clone();

    for row in &rows {
        let expire = match row.expire_date {
            Some(d) => d,
            None => continue,
        };
        let days_remaining = (expire - today).num_days();

        // Find which thresholds apply (expire within N days)
        for &threshold in thresholds {
            if days_remaining > threshold as i64 {
                continue;
            }

            let dedup_key = format!("notif:{}:{}", row.id, threshold);

            // Check if already notified
            let exists: bool = conn.exists(&dedup_key).await.unwrap_or(false);
            if exists {
                continue;
            }

            tracing::info!(
                "Sending notification: {} expires in {} days (threshold={})",
                row.hostname,
                days_remaining,
                threshold,
            );

            let payload = NotificationPayload {
                event: "vps_expiring".into(),
                hostname: row.hostname.clone(),
                alias: row.alias.clone(),
                provider: row.provider_name.clone(),
                country: row.country.clone(),
                ip_addresses: row.ip_addresses.0.clone(),
                expire_date: expire,
                days_remaining: days_remaining as i32,
            };

            // Send Telegram
            if let (Some(token), Some(chat_id)) =
                (&cfg.telegram_bot_token, &cfg.telegram_chat_id)
            {
                let message = format_telegram_message(&payload);
                if let Err(e) = send_telegram(http_client, token, chat_id, &message).await {
                    tracing::error!("Telegram send error for {}: {e}", row.hostname);
                }
            }

            // Send Webhook
            if let Some(url) = &cfg.webhook_url {
                if let Err(e) = send_webhook(http_client, url, &payload).await {
                    tracing::error!("Webhook send error for {}: {e}", row.hostname);
                }
            }

            // Mark as notified with TTL
            let ttl_secs = (threshold as i64 + 1) * 86400;
            let now = chrono::Utc::now().to_rfc3339();
            if let Err(e) = conn
                .set_ex::<_, _, ()>(&dedup_key, &now, ttl_secs as u64)
                .await
            {
                tracing::error!("Redis SET error for {dedup_key}: {e}");
            }
        }
    }

    Ok(())
}

async fn send_telegram(
    client: &reqwest::Client,
    token: &str,
    chat_id: &str,
    message: &str,
) -> anyhow::Result<()> {
    let url = format!("https://api.telegram.org/bot{token}/sendMessage");
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": message,
            "parse_mode": "HTML",
        }))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Telegram API {status}: {body}");
    }

    Ok(())
}

async fn send_webhook(
    client: &reqwest::Client,
    url: &str,
    payload: &NotificationPayload,
) -> anyhow::Result<()> {
    let resp = client.post(url).json(payload).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Webhook {status}: {body}");
    }

    Ok(())
}

fn format_telegram_message(p: &NotificationPayload) -> String {
    let ip_list = p
        .ip_addresses
        .iter()
        .map(|e| {
            if e.label.is_empty() {
                e.ip.clone()
            } else {
                format!("{} ({})", e.ip, e.label)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    let host_display = if p.alias.is_empty() {
        p.hostname.clone()
    } else {
        format!("{} ({})", p.hostname, p.alias)
    };

    format!(
        "\u{26a0}\u{fe0f} <b>VPS Expiring Soon</b>\n\
         Host: <code>{host_display}</code>\n\
         Provider: {provider} / {country}\n\
         IPs: <code>{ip_list}</code>\n\
         Expires: <b>{expire_date}</b> ({days} days)",
        provider = p.provider,
        country = p.country,
        expire_date = p.expire_date,
        days = p.days_remaining,
    )
}

#[derive(Debug, Serialize)]
struct NotificationPayload {
    event: String,
    hostname: String,
    alias: String,
    provider: String,
    country: String,
    ip_addresses: Vec<IpEntry>,
    expire_date: NaiveDate,
    days_remaining: i32,
}

#[derive(Debug, sqlx::FromRow)]
struct ExpiringVps {
    id: Uuid,
    hostname: String,
    alias: String,
    ip_addresses: sqlx::types::Json<Vec<IpEntry>>,
    country: String,
    expire_date: Option<NaiveDate>,
    #[allow(dead_code)]
    status: String,
    provider_name: String,
}
