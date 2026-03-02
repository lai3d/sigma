use crate::models::DnsAccount;
use crate::routes::AppState;

/// Background worker that periodically syncs all DNS accounts.
/// Reads the sync interval from the `system_settings` table each iteration,
/// allowing runtime changes from the Settings page.
pub async fn run(state: AppState, default_interval_secs: u64) {
    tracing::info!("DNS background sync started (default_interval={default_interval_secs}s)");

    // Wait one interval before first sync (server just booted, let things settle)
    let initial = get_interval_secs(&state, default_interval_secs).await;
    tokio::time::sleep(std::time::Duration::from_secs(initial)).await;

    loop {
        let interval_secs = get_interval_secs(&state, default_interval_secs).await;

        if interval_secs == 0 {
            tracing::info!("DNS sync: disabled via settings, will re-check in 60s");
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            continue;
        }

        if let Err(e) = sync_all_accounts(&state).await {
            tracing::error!("DNS background sync error: {e}");
        }

        let sleep_secs = get_interval_secs(&state, default_interval_secs).await;
        let sleep_secs = if sleep_secs == 0 { 60 } else { sleep_secs };
        tokio::time::sleep(std::time::Duration::from_secs(sleep_secs)).await;
    }
}

/// Query the DNS sync interval from system_settings, falling back to the startup default.
async fn get_interval_secs(state: &AppState, default: u64) -> u64 {
    let result: Result<Option<(String,)>, _> = sqlx::query_as(
        "SELECT value FROM system_settings WHERE key = 'dns_sync_interval_secs'",
    )
    .fetch_optional(&state.db)
    .await;

    match result {
        Ok(Some((val,))) => val.parse::<u64>().unwrap_or(default),
        _ => default,
    }
}

async fn sync_all_accounts(state: &AppState) -> anyhow::Result<()> {
    let accounts: Vec<DnsAccount> = sqlx::query_as(
        "SELECT id, name, provider_type, config, created_at, updated_at FROM dns_accounts ORDER BY name",
    )
    .fetch_all(&state.db)
    .await?;

    if accounts.is_empty() {
        tracing::debug!("DNS sync: no accounts configured, skipping");
        return Ok(());
    }

    tracing::info!("DNS sync: starting sync for {} account(s)", accounts.len());

    for account in &accounts {
        match crate::routes::dns::sync_provider(state, account).await {
            Ok(result) => {
                tracing::info!(
                    "DNS sync: {} ({}) — {} zones, {} records, {} linked, {} deleted",
                    account.name,
                    account.provider_type,
                    result.zones_count,
                    result.records_count,
                    result.records_linked,
                    result.records_deleted,
                );
            }
            Err(e) => {
                tracing::error!(
                    "DNS sync: failed for {} ({}): {e}",
                    account.name,
                    account.provider_type,
                );
            }
        }
    }

    Ok(())
}
