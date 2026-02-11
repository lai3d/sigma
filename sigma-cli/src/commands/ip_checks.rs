use anyhow::Result;
use uuid::Uuid;

use crate::client::SigmaClient;
use crate::models::*;
use crate::output;

pub async fn list(
    client: &SigmaClient,
    vps_id: Option<Uuid>,
    ip: Option<&str>,
    source: Option<&str>,
    check_type: Option<&str>,
    success: Option<bool>,
    page: i64,
    per_page: i64,
    json: bool,
) -> Result<()> {
    let mut params = format!("page={page}&per_page={per_page}");
    if let Some(v) = vps_id { params.push_str(&format!("&vps_id={v}")); }
    if let Some(v) = ip { params.push_str(&format!("&ip={v}")); }
    if let Some(v) = source { params.push_str(&format!("&source={v}")); }
    if let Some(v) = check_type { params.push_str(&format!("&check_type={v}")); }
    if let Some(v) = success { params.push_str(&format!("&success={v}")); }

    let resp: PaginatedResponse<IpCheck> =
        client.get(&format!("/ip-checks?{params}")).await?;

    if json {
        return output::print_json(&resp.data);
    }

    let rows: Vec<Vec<String>> = resp
        .data
        .iter()
        .map(|c| {
            vec![
                c.id.to_string(),
                c.vps_id.to_string(),
                c.ip.clone(),
                c.check_type.clone(),
                c.source.clone(),
                if c.success { "ok".into() } else { "FAIL".into() },
                c.latency_ms.map_or("-".into(), |l| format!("{}ms", l)),
                c.checked_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            ]
        })
        .collect();

    output::print_table(
        &["ID", "VPS", "IP", "Type", "Source", "Result", "Latency", "Checked"],
        rows,
    );
    output::print_pagination(resp.page, resp.per_page, resp.total);
    Ok(())
}

pub async fn get(client: &SigmaClient, id: Uuid, json: bool) -> Result<()> {
    let check: IpCheck = client.get(&format!("/ip-checks/{id}")).await?;

    if json {
        return output::print_json(&check);
    }

    output::print_table(
        &["Field", "Value"],
        vec![
            vec!["ID".into(), check.id.to_string()],
            vec!["VPS ID".into(), check.vps_id.to_string()],
            vec!["IP".into(), check.ip],
            vec!["Type".into(), check.check_type],
            vec!["Source".into(), check.source],
            vec!["Success".into(), check.success.to_string()],
            vec!["Latency".into(), check.latency_ms.map_or("-".into(), |l| format!("{}ms", l))],
            vec!["Checked At".into(), check.checked_at.to_rfc3339()],
        ],
    );
    Ok(())
}

pub async fn create(
    client: &SigmaClient,
    vps_id: Uuid,
    ip: String,
    success: bool,
    check_type: Option<String>,
    source: Option<String>,
    latency_ms: Option<i32>,
    json: bool,
) -> Result<()> {
    let body = CreateIpCheck {
        vps_id,
        ip,
        check_type,
        source,
        success,
        latency_ms,
    };

    let check: IpCheck = client.post("/ip-checks", &body).await?;

    if json {
        return output::print_json(&check);
    }

    let result = if check.success { "ok" } else { "FAIL" };
    println!(
        "Recorded check {} — {} {} from {} → {}",
        check.id, check.check_type, check.ip, check.source, result
    );
    Ok(())
}

pub async fn delete(client: &SigmaClient, id: Uuid) -> Result<()> {
    client.delete(&format!("/ip-checks/{id}")).await?;
    println!("Deleted check {id}");
    Ok(())
}

pub async fn summary(
    client: &SigmaClient,
    vps_id: Option<Uuid>,
    json: bool,
) -> Result<()> {
    let path = match vps_id {
        Some(id) => format!("/ip-checks/summary?vps_id={id}"),
        None => "/ip-checks/summary".into(),
    };

    let summaries: Vec<IpCheckSummary> = client.get(&path).await?;

    if json {
        return output::print_json(&summaries);
    }

    let rows: Vec<Vec<String>> = summaries
        .iter()
        .map(|s| {
            vec![
                s.vps_id.to_string(),
                s.ip.clone(),
                s.total_checks.to_string(),
                format!("{}/{}", s.success_count, s.total_checks),
                format!("{:.1}%", s.success_rate),
                s.avg_latency_ms.map_or("-".into(), |l| format!("{:.0}ms", l)),
                if s.last_success { "ok".into() } else { "FAIL".into() },
                s.last_check.format("%Y-%m-%d %H:%M").to_string(),
            ]
        })
        .collect();

    output::print_table(
        &["VPS", "IP", "Total", "Success", "Rate", "Avg Lat", "Last", "Last Check"],
        rows,
    );
    Ok(())
}

pub async fn purge(
    client: &SigmaClient,
    older_than_days: i32,
) -> Result<()> {
    let result: PurgeResult = client
        .delete_json(&format!("/ip-checks/purge?older_than_days={older_than_days}"))
        .await?;

    println!("Purged {} checks older than {} days", result.deleted, older_than_days);
    Ok(())
}
