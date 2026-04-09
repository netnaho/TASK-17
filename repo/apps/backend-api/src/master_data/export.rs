/// Export master-data reference tables to CSV or XLSX.
///
/// Both formats include the same column set as the import whitelist, making
/// exported files valid templates for reimporting (after editing).
/// Export events are logged to `export_logs`.
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiAppError;
use crate::security::rbac::Principal;

use super::service::{assert_admin, assert_institution_scope};

#[derive(Debug)]
pub struct ExportOutput {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
    pub file_name: String,
    pub row_count: i32,
}

pub async fn export_entity(
    pool: &PgPool,
    p: &Principal,
    entity_type: &str,
    institution_id: Uuid,
    format: &str,
) -> Result<ExportOutput, ApiAppError> {
    assert_admin(p)?;
    assert_institution_scope(p, institution_id)?;

    if !["students","classes","courses","semesters","departments"].contains(&entity_type) {
        return Err(ApiAppError::BadRequest(format!("unknown entity_type: {entity_type}")));
    }
    if !["csv","xlsx"].contains(&format) {
        return Err(ApiAppError::BadRequest("format must be 'csv' or 'xlsx'".into()));
    }

    let (headers, rows) = fetch_rows(pool, entity_type, institution_id).await?;
    let row_count = rows.len() as i32;

    let (bytes, content_type, ext) = match format {
        "xlsx" => {
            let b = build_xlsx(&headers, &rows)?;
            (b, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", "xlsx")
        }
        _ => {
            let b = build_csv(&headers, &rows)?;
            (b, "text/csv; charset=utf-8", "csv")
        }
    };

    let timestamp = Utc::now().format("%Y%m%d%H%M%S");
    let file_name = format!("{entity_type}_{timestamp}.{ext}");

    // Log the export
    sqlx::query(
        "INSERT INTO export_logs (entity_type, institution_id, exported_by, format, row_count, file_name)
         VALUES ($1,$2,$3,$4,$5,$6)",
    )
    .bind(entity_type).bind(institution_id).bind(p.user_id)
    .bind(format).bind(row_count).bind(&file_name)
    .execute(pool).await?;

    Ok(ExportOutput { bytes, content_type, file_name, row_count })
}

// ── data fetcher ──────────────────────────────────────────────────────────
async fn fetch_rows(
    pool: &PgPool,
    entity_type: &str,
    institution_id: Uuid,
) -> Result<(Vec<String>, Vec<Vec<String>>), ApiAppError> {
    match entity_type {
        "students" => {
            let rows: Vec<(String, String, String, Option<String>, Option<chrono::NaiveDate>, Option<String>)> =
                sqlx::query_as(
                    "SELECT s.student_number, s.first_name, s.last_name, s.email,
                             s.date_of_birth, cl.code
                     FROM students s LEFT JOIN classes cl ON cl.id = s.class_id
                     WHERE s.institution_id=$1 ORDER BY s.student_number",
                ).bind(institution_id).fetch_all(pool).await?;
            let headers = vec!["student_number","first_name","last_name","email","date_of_birth","class_code"]
                .into_iter().map(String::from).collect();
            let data = rows.into_iter().map(|r| vec![
                r.0, r.1, r.2,
                r.3.unwrap_or_default(),
                r.4.map(|d| d.to_string()).unwrap_or_default(),
                r.5.unwrap_or_default(),
            ]).collect();
            Ok((headers, data))
        }
        "classes" => {
            let rows: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
                "SELECT cl.code, cl.label, s.code, d.name
                 FROM classes cl
                 LEFT JOIN semesters s ON s.id = cl.semester_id
                 LEFT JOIN departments d ON d.id = cl.department_id
                 WHERE cl.institution_id=$1 ORDER BY cl.code",
            ).bind(institution_id).fetch_all(pool).await?;
            let headers = vec!["code","label","semester_code","department_name"]
                .into_iter().map(String::from).collect();
            let data = rows.into_iter().map(|r| vec![
                r.0, r.1,
                r.2.unwrap_or_default(),
                r.3.unwrap_or_default(),
            ]).collect();
            Ok((headers, data))
        }
        "courses" => {
            let rows: Vec<(String, String, Option<String>, i32)> = sqlx::query_as(
                "SELECT c.code, c.title, d.name, c.credits
                 FROM courses c LEFT JOIN departments d ON d.id = c.department_id
                 WHERE c.institution_id=$1 ORDER BY c.code",
            ).bind(institution_id).fetch_all(pool).await?;
            let headers = vec!["code","title","department_name","credits"]
                .into_iter().map(String::from).collect();
            let data = rows.into_iter().map(|r| vec![
                r.0, r.1,
                r.2.unwrap_or_default(),
                r.3.to_string(),
            ]).collect();
            Ok((headers, data))
        }
        "semesters" => {
            let rows: Vec<(String, String, chrono::NaiveDate, chrono::NaiveDate)> = sqlx::query_as(
                "SELECT code, label, starts_on, ends_on FROM semesters
                 WHERE institution_id=$1 ORDER BY starts_on",
            ).bind(institution_id).fetch_all(pool).await?;
            let headers = vec!["code","label","starts_on","ends_on"]
                .into_iter().map(String::from).collect();
            let data = rows.into_iter().map(|r| vec![
                r.0, r.1, r.2.to_string(), r.3.to_string(),
            ]).collect();
            Ok((headers, data))
        }
        "departments" => {
            let rows: Vec<(String,)> = sqlx::query_as(
                "SELECT d.name FROM departments d JOIN sites si ON si.id=d.site_id
                 WHERE si.institution_id=$1 ORDER BY d.name",
            ).bind(institution_id).fetch_all(pool).await?;
            let headers = vec!["name".to_string()];
            let data = rows.into_iter().map(|r| vec![r.0]).collect();
            Ok((headers, data))
        }
        _ => Err(ApiAppError::BadRequest("unknown entity_type".into())),
    }
}

// ── CSV builder ────────────────────────────────────────────────────────────
fn build_csv(headers: &[String], rows: &[Vec<String>]) -> Result<Vec<u8>, ApiAppError> {
    let mut wtr = csv::WriterBuilder::new()
        .has_headers(true)
        .from_writer(Vec::new());
    wtr.write_record(headers)
        .map_err(|e| ApiAppError::Internal(e.to_string()))?;
    for row in rows {
        wtr.write_record(row)
            .map_err(|e| ApiAppError::Internal(e.to_string()))?;
    }
    wtr.flush().map_err(|e| ApiAppError::Internal(e.to_string()))?;
    wtr.into_inner().map_err(|e| ApiAppError::Internal(e.to_string()))
}

// ── XLSX builder ───────────────────────────────────────────────────────────
fn build_xlsx(headers: &[String], rows: &[Vec<String>]) -> Result<Vec<u8>, ApiAppError> {
    use rust_xlsxwriter::Workbook;
    use std::io::Read;

    // rust_xlsxwriter 0.7.0 writes to a named file; use a temp path.
    let tmp = tempfile::Builder::new()
        .suffix(".xlsx")
        .tempfile()
        .map_err(|e| ApiAppError::Internal(e.to_string()))?;
    let tmp_path = tmp.path().to_str()
        .ok_or_else(|| ApiAppError::Internal("temp path not valid UTF-8".into()))?
        .to_string();
    // Close the NamedTempFile handle so rust_xlsxwriter can write to the same path.
    drop(tmp);

    let mut wb = Workbook::new(&tmp_path);
    let ws = wb.add_worksheet();

    for (col, h) in headers.iter().enumerate() {
        ws.write_string_only(0, col as u16, h.as_str())
            .map_err(|e: rust_xlsxwriter::XlsxError| ApiAppError::Internal(e.to_string()))?;
    }
    for (row_idx, row) in rows.iter().enumerate() {
        for (col, val) in row.iter().enumerate() {
            ws.write_string_only(row_idx as u32 + 1, col as u16, val.as_str())
                .map_err(|e: rust_xlsxwriter::XlsxError| ApiAppError::Internal(e.to_string()))?;
        }
    }
    wb.close()
        .map_err(|e: rust_xlsxwriter::XlsxError| ApiAppError::Internal(e.to_string()))?;

    let mut buf = Vec::new();
    std::fs::File::open(&tmp_path)
        .map_err(|e| ApiAppError::Internal(e.to_string()))?
        .read_to_end(&mut buf)
        .map_err(|e| ApiAppError::Internal(e.to_string()))?;
    // Clean up temp file
    let _ = std::fs::remove_file(&tmp_path);
    Ok(buf)
}
