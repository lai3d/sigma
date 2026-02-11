use anyhow::Result;
use uuid::Uuid;

use crate::client::SigmaClient;
use crate::models::*;
use crate::output;

pub async fn list(client: &SigmaClient, page: i64, per_page: i64, json: bool) -> Result<()> {
    let resp: PaginatedResponse<Provider> =
        client.get(&format!("/providers?page={page}&per_page={per_page}")).await?;

    if json {
        return output::print_json(&resp.data);
    }

    let rows: Vec<Vec<String>> = resp
        .data
        .iter()
        .map(|p| {
            vec![
                p.id.to_string(),
                p.name.clone(),
                p.country.clone(),
                p.website.clone(),
                if p.api_supported { "yes".into() } else { "no".into() },
                p.rating.map_or("-".into(), |r| r.to_string()),
            ]
        })
        .collect();

    output::print_table(
        &["ID", "Name", "Country", "Website", "API", "Rating"],
        rows,
    );
    output::print_pagination(resp.page, resp.per_page, resp.total);
    Ok(())
}

pub async fn get(client: &SigmaClient, id: Uuid, json: bool) -> Result<()> {
    let provider: Provider = client.get(&format!("/providers/{id}")).await?;

    if json {
        return output::print_json(&provider);
    }

    output::print_table(
        &["Field", "Value"],
        vec![
            vec!["ID".into(), provider.id.to_string()],
            vec!["Name".into(), provider.name],
            vec!["Country".into(), provider.country],
            vec!["Website".into(), provider.website],
            vec!["Panel URL".into(), provider.panel_url],
            vec!["API Supported".into(), provider.api_supported.to_string()],
            vec!["Rating".into(), provider.rating.map_or("-".into(), |r| r.to_string())],
            vec!["Notes".into(), provider.notes],
            vec!["Created".into(), provider.created_at.to_rfc3339()],
            vec!["Updated".into(), provider.updated_at.to_rfc3339()],
        ],
    );
    Ok(())
}

pub async fn create(
    client: &SigmaClient,
    name: String,
    country: Option<String>,
    website: Option<String>,
    panel_url: Option<String>,
    api_supported: bool,
    rating: Option<i16>,
    notes: Option<String>,
    json: bool,
) -> Result<()> {
    let body = CreateProvider {
        name,
        country,
        website,
        panel_url,
        api_supported: if api_supported { Some(true) } else { None },
        rating,
        notes,
    };

    let provider: Provider = client.post("/providers", &body).await?;

    if json {
        return output::print_json(&provider);
    }

    println!("Created provider {} ({})", provider.name, provider.id);
    Ok(())
}

pub async fn update(
    client: &SigmaClient,
    id: Uuid,
    name: Option<String>,
    country: Option<String>,
    website: Option<String>,
    panel_url: Option<String>,
    api_supported: Option<bool>,
    rating: Option<i16>,
    notes: Option<String>,
    json: bool,
) -> Result<()> {
    let body = UpdateProvider {
        name,
        country,
        website,
        panel_url,
        api_supported,
        rating: rating.map(Some),
        notes,
    };

    let provider: Provider = client.put(&format!("/providers/{id}"), &body).await?;

    if json {
        return output::print_json(&provider);
    }

    println!("Updated provider {} ({})", provider.name, provider.id);
    Ok(())
}

pub async fn delete(client: &SigmaClient, id: Uuid) -> Result<()> {
    client.delete(&format!("/providers/{id}")).await?;
    println!("Deleted provider {id}");
    Ok(())
}

pub async fn export(
    client: &SigmaClient,
    format: &str,
    output_file: Option<&str>,
) -> Result<()> {
    let (body, _) = client.get_text(&format!("/providers/export?format={format}")).await?;

    match output_file {
        Some(path) => {
            std::fs::write(path, &body)?;
            println!("Exported providers to {path}");
        }
        None => print!("{body}"),
    }
    Ok(())
}

pub async fn import(client: &SigmaClient, file: &str, format: &str) -> Result<()> {
    let data = std::fs::read_to_string(file)?;
    let body = ImportRequest {
        format: format.to_string(),
        data,
    };

    let result: ImportResult = client.post("/providers/import", &body).await?;

    println!("Imported {} providers", result.imported);
    if !result.errors.is_empty() {
        println!("Errors:");
        for err in &result.errors {
            println!("  - {err}");
        }
    }
    Ok(())
}
