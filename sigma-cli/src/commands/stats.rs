use anyhow::Result;

use crate::client::SigmaClient;
use crate::models::*;
use crate::output;

pub async fn stats(client: &SigmaClient, json: bool) -> Result<()> {
    let stats: DashboardStats = client.get("/stats").await?;

    if json {
        return output::print_json(&stats);
    }

    println!("=== Dashboard Stats ===\n");
    println!("Total VPS:       {}", stats.total_vps);
    println!("Active VPS:      {}", stats.active_vps);
    println!("Total Providers: {}\n", stats.total_providers);

    if !stats.by_status.is_empty() {
        println!("By Status:");
        output::print_table(
            &["Status", "Count"],
            stats
                .by_status
                .iter()
                .map(|s| {
                    vec![
                        s.label.clone().unwrap_or_default(),
                        s.count.unwrap_or(0).to_string(),
                    ]
                })
                .collect(),
        );
        println!();
    }

    if !stats.by_country.is_empty() {
        println!("By Country:");
        output::print_table(
            &["Country", "Count"],
            stats
                .by_country
                .iter()
                .map(|s| {
                    vec![
                        s.label.clone().unwrap_or_default(),
                        s.count.unwrap_or(0).to_string(),
                    ]
                })
                .collect(),
        );
        println!();
    }

    if !stats.by_provider.is_empty() {
        println!("By Provider:");
        output::print_table(
            &["Provider", "Count"],
            stats
                .by_provider
                .iter()
                .map(|s| {
                    vec![
                        s.label.clone().unwrap_or_default(),
                        s.count.unwrap_or(0).to_string(),
                    ]
                })
                .collect(),
        );
        println!();
    }

    if !stats.expiring_soon.is_empty() {
        println!("Expiring Soon:");
        output::print_table(
            &["ID", "Hostname", "Country", "Expires"],
            stats
                .expiring_soon
                .iter()
                .map(|v| {
                    vec![
                        v.id.to_string(),
                        v.hostname.clone(),
                        v.country.clone(),
                        v.expire_date.map_or("-".into(), |d| d.to_string()),
                    ]
                })
                .collect(),
        );
    }

    Ok(())
}
