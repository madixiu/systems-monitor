use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, Datelike, Timelike, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::postgres::Postgres;
use sqlx::types::Json as SqlJson;
use sqlx::QueryBuilder;

use crate::auth::ApiAuth;
use crate::error::AppError;
use crate::AppState;

#[derive(Debug, Deserialize, Default)]
pub struct ExportParams {
    pub format: Option<String>,          // "json" | "csv" | "xlsx"
    pub full: Option<String>,            // "1" → raw systems_current rows (incl. software, updates, raw_json)
    pub q: Option<String>,
    pub domain: Option<String>,
    pub os: Option<String>,
    pub live_state: Option<String>,
}

const HEADERS: &[&str] = &[
    "Computer Name", "IP Address", "MAC Address", "Last Seen", "Last Seen (Jalali)",
    "Platform", "Console Username", "Operating System", "x64 OS", "Processor",
    "Installed RAM", "System Type", "System Drive Free Space", "System Drive Total Space",
    "Main Storage Size", "Domain Name", "Full Computer Name", "Motherboard Name",
];

pub async fn export(
    State(state): State<AppState>,
    _auth: ApiAuth,
    Query(p): Query<ExportParams>,
) -> Result<Response, AppError> {
    let rows = fetch_rows(&state, &p).await?;
    let items: Vec<Value> = rows.into_iter().map(|j| j.0).collect();

    let format = p.format.as_deref().unwrap_or("json").to_lowercase();
    let full = p.full.as_deref() == Some("1");

    match format.as_str() {
        "csv"  => Ok(csv_response(&items)),
        "xlsx" => xlsx_response(&items).map_err(|e| AppError::Internal(format!("xlsx: {e}"))),
        _ if full => Ok(Json(items).into_response()),
        _         => Ok(Json(project_rows(&items)).into_response()),
    }
}

async fn fetch_rows(state: &AppState, p: &ExportParams) -> Result<Vec<SqlJson<Value>>, AppError> {
    let mut qb: QueryBuilder<Postgres> =
        QueryBuilder::new(format!("{} WHERE 1=1", crate::db::SYSTEMS_SNAPSHOT_SELECT));
    if let Some(q) = &p.q {
        let like = format!("%{}%", q);
        qb.push(" AND (s.computer_name ILIKE ");
        qb.push_bind(like.clone());
        qb.push(" OR s.fqdn ILIKE ");
        qb.push_bind(like);
        qb.push(")");
    }
    if let Some(d) = &p.domain { qb.push(" AND s.domain = "); qb.push_bind(d.clone()); }
    if let Some(o) = &p.os     { qb.push(" AND s.os_name ILIKE "); qb.push_bind(format!("%{}%", o)); }
    if let Some(ls) = &p.live_state {
        if ls != "any" { qb.push(" AND s.live_state = "); qb.push_bind(ls.clone()); }
    }
    qb.push(" ORDER BY s.computer_name ASC");
    Ok(qb.build_query_scalar().fetch_all(&state.pool).await?)
}

fn project_rows(items: &[Value]) -> Vec<Value> { items.iter().map(project_row).collect() }

fn project_row(v: &Value) -> Value {
    json!({
        "Computer Name":           v.get("computer_name").cloned().unwrap_or(Value::Null),
        "IP Address":              v.get("ip_addresses").and_then(join_array),
        "MAC Address":             v.get("mac_addresses").and_then(join_array),
        "Last Seen":               v.get("last_seen").cloned().unwrap_or(Value::Null),
        "Last Seen (Jalali)":      v.get("last_seen").and_then(to_jalali_str),
        "Platform":                v.get("platform").cloned().unwrap_or(Value::Null),
        "Console Username":        v.get("console_user").cloned().unwrap_or(Value::Null),
        "Operating System":        build_os_string(v),
        "x64 OS":                  if v.get("os_arch").and_then(|a| a.as_str()).map(|a| a.contains("64")).unwrap_or(false) { "Yes" } else { "No" },
        "Processor":               v.get("cpu_model").cloned().unwrap_or(Value::Null),
        "Installed RAM":           v.get("ram_bytes").and_then(fmt_gb),
        "System Type":             v.get("system_type").cloned().unwrap_or(Value::Null),
        "System Drive Free Space": v.get("system_drive_free").and_then(fmt_gb),
        "System Drive Total Space":v.get("system_drive_total").and_then(fmt_gb),
        "Main Storage Size":       v.get("main_storage_size").and_then(fmt_gb),
        "Domain Name":             v.get("domain").cloned().unwrap_or(Value::Null),
        "Full Computer Name":      v.get("fqdn").cloned().unwrap_or(Value::Null),
        "Motherboard Name":        v.get("motherboard").cloned().unwrap_or(Value::Null),
    })
}

fn join_array(v: &Value) -> Option<Value> {
    let arr = v.as_array()?;
    let s: Vec<String> = arr.iter().filter_map(|x| x.as_str().map(String::from)).collect();
    if s.is_empty() { None } else { Some(Value::String(s.join(", "))) }
}

fn build_os_string(v: &Value) -> Value {
    let name  = v.get("os_name").and_then(|x| x.as_str()).unwrap_or("");
    let ver   = v.get("os_version").and_then(|x| x.as_str()).unwrap_or("");
    let build = v.get("os_build").and_then(|x| x.as_str()).unwrap_or("");
    let arch  = v.get("os_arch").and_then(|x| x.as_str()).unwrap_or("");
    let mut s = name.to_string();
    if !ver.is_empty()   { s.push_str(&format!(" v{}", ver)); }
    if !build.is_empty() { s.push_str(&format!(" (Build {})", build)); }
    if !arch.is_empty()  { s.push_str(&format!(" {}", arch)); }
    Value::String(s)
}

fn fmt_gb(v: &Value) -> Option<Value> {
    let n = v.as_i64()?;
    if n == 0 { return Some(Value::String("(empty)".into())); }
    let gb = n as f64 / 1024.0 / 1024.0 / 1024.0;
    Some(Value::String(format!("{:.2} GB", gb)))
}

fn to_jalali_str(v: &Value) -> Option<Value> {
    let s = v.as_str()?;
    let dt: DateTime<Utc> = s.parse().ok()?;
    let (jy, jm, jd) = gregorian_to_jalali(dt.year(), dt.month() as i32, dt.day() as i32);
    Some(Value::String(format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        jy, jm, jd, dt.hour(), dt.minute(), dt.second()
    )))
}

fn gregorian_to_jalali(gy: i32, gm: i32, gd: i32) -> (i32, i32, i32) {
    let g_d_m = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let gy2 = if gm > 2 { gy + 1 } else { gy };
    let mut days = 355666 + (365 * gy) + ((gy2 + 3) / 4) - ((gy2 + 99) / 100)
        + ((gy2 + 399) / 400) + gd + g_d_m[(gm - 1) as usize];
    let mut jy = -1595 + (33 * (days / 12053));
    days %= 12053;
    jy += 4 * (days / 1461);
    days %= 1461;
    if days > 365 { jy += (days - 1) / 365; days = (days - 1) % 365; }
    let jm = if days < 186 { 1 + days / 31 } else { 7 + (days - 186) / 30 };
    let jd = 1 + if days < 186 { days % 31 } else { (days - 186) % 30 };
    (jy, jm, jd)
}

fn csv_response(items: &[Value]) -> Response {
    let mut w = csv::WriterBuilder::new().from_writer(Vec::<u8>::new());
    let _ = w.write_record(HEADERS);
    for it in items {
        let row = project_row(it);
        let mut rec = Vec::new();
        for h in HEADERS {
            let v = row.get(*h).cloned().unwrap_or(Value::Null);
            rec.push(match v {
                Value::Null => String::new(),
                Value::String(s) => s,
                other => other.to_string(),
            });
        }
        let _ = w.write_record(&rec);
    }
    let bytes = w.into_inner().unwrap_or_default();
    let mut resp = Response::new(Body::from(bytes));
    resp.headers_mut().insert(header::CONTENT_TYPE, header::HeaderValue::from_static("text/csv; charset=utf-8"));
    resp.headers_mut().insert(header::CONTENT_DISPOSITION, header::HeaderValue::from_static("attachment; filename=\"systems.csv\""));
    resp
}

fn xlsx_response(items: &[Value]) -> Result<Response, anyhow::Error> {
    use rust_xlsxwriter::Workbook;
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    let _ = sheet.set_name("Systems");
    for (col, h) in HEADERS.iter().enumerate() {
        sheet.write_string(0, col as u16, *h)?;
    }
    for (r, it) in items.iter().enumerate() {
        let row = project_row(it);
        for (col, h) in HEADERS.iter().enumerate() {
            let v = row.get(*h).cloned().unwrap_or(Value::Null);
            let s = match v {
                Value::Null => String::new(),
                Value::String(s) => s,
                other => other.to_string(),
            };
            sheet.write_string((r + 1) as u32, col as u16, &s)?;
        }
    }
    let bytes = workbook.save_to_buffer()?;
    let mut resp = Response::new(Body::from(bytes));
    resp.headers_mut().insert(header::CONTENT_TYPE, header::HeaderValue::from_static(
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"));
    resp.headers_mut().insert(header::CONTENT_DISPOSITION, header::HeaderValue::from_static("attachment; filename=\"systems.xlsx\""));
    Ok(resp)
}
