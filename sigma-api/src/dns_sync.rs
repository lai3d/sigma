use crate::models::DnsAccount;
use crate::routes::AppState;

/// Background worker that periodically syncs all DNS accounts.
pub async fn run(state: AppState, interval_secs: u64) {
    let interval = std::time::Duration::from_secs(interval_secs);

    tracing::info!("DNS background sync started (interval={interval_secs}s)");

    // Wait one interval before first sync (server just booted, let things settle)
    tokio::time::sleep(interval).await;

    loop {
        if let Err(e) = sync_all_accounts(&state).await {
            tracing::error!("DNS background sync error: {e}");
        }
        tokio::time::sleep(interval).await;
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
