use comfy_table::{ContentArrangement, Table};

pub fn print_table(headers: &[&str], rows: Vec<Vec<String>>) {
    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(headers);
    for row in rows {
        table.add_row(row);
    }
    println!("{table}");
}

pub fn print_json<T: serde::Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub fn print_pagination(page: i64, per_page: i64, total: i64) {
    let total_pages = (total + per_page - 1) / per_page;
    println!("Page {} of {} ({} total)", page, total_pages.max(1), total);
}
