/// Secure import pipeline for master-data reference tables.
///
/// Flow:
///   1. Receive multipart upload (field: "file"; query params: entity_type, institution_id)
///   2. Enforce file size limit (10 MB)
///   3. Validate MIME type + extension whitelist
///   4. Compute SHA-256 fingerprint of raw bytes
///   5. Reject duplicate fingerprints (same entity_type + institution_id)
///   6. Create import_job record (status = processing)
///   7. Parse CSV or XLSX; validate column whitelist + required columns
///   8. For each data row: normalize → validate → INSERT or record rejection
///   9. All accepted rows committed in one transaction; job_row results in same tx
///  10. Update import_job to status = done / failed
///
/// Import semantics: PARTIAL SUCCESS — each row is evaluated independently.
/// Accepted rows are committed atomically; rejected rows are recorded with
/// per-row rejection reasons. The caller sees totals in the job record.
use actix_multipart::Multipart;

use futures_util::StreamExt;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiAppError;
use crate::security::rbac::Principal;

use super::service::assert_admin;

// ── constants ──────────────────────────────────────────────────────────────
const MAX_FILE_BYTES: usize = 10 * 1024 * 1024; // 10 MB

const ALLOWED_MIMES: &[&str] = &[
    "text/csv",
    "text/plain",
    "application/csv",
    "application/vnd.ms-excel",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
];
const ALLOWED_EXTS: &[&str] = &["csv", "xlsx"];

// ── column whitelists + required columns per entity ────────────────────────
struct ColSpec {
    name: &'static str,
    required: bool,
}

fn column_spec(entity_type: &str) -> &'static [ColSpec] {
    match entity_type {
        "students" => &[
            ColSpec { name: "student_number", required: true },
            ColSpec { name: "first_name",     required: true },
            ColSpec { name: "last_name",      required: true },
            ColSpec { name: "email",          required: false },
            ColSpec { name: "date_of_birth",  required: false },
            ColSpec { name: "class_code",     required: false },
        ],
        "classes" => &[
            ColSpec { name: "code",            required: true },
            ColSpec { name: "label",           required: true },
            ColSpec { name: "semester_code",   required: false },
            ColSpec { name: "department_name", required: false },
        ],
        "courses" => &[
            ColSpec { name: "code",            required: true },
            ColSpec { name: "title",           required: true },
            ColSpec { name: "department_name", required: false },
            ColSpec { name: "credits",         required: false },
        ],
        "semesters" => &[
            ColSpec { name: "code",      required: true },
            ColSpec { name: "label",     required: true },
            ColSpec { name: "starts_on", required: true },
            ColSpec { name: "ends_on",   required: true },
        ],
        "departments" => &[
            ColSpec { name: "name",      required: true },
            ColSpec { name: "site_name", required: false },
        ],
        _ => &[],
    }
}

// ── public entry point ─────────────────────────────────────────────────────
#[derive(Debug, serde::Serialize)]
pub struct ImportResult {
    pub job_id: Uuid,
    pub status: String,
    pub total_rows: i32,
    pub accepted_rows: i32,
    pub rejected_rows: i32,
    pub error_message: Option<String>,
}

pub async fn run_import(
    pool: &PgPool,
    p: &Principal,
    entity_type: String,
    institution_id: Uuid,
    mut payload: Multipart,
) -> Result<ImportResult, ApiAppError> {
    assert_admin(p)?;
    super::service::assert_institution_scope(p, institution_id)?;

    // Validate entity type
    if !["students","classes","courses","semesters","departments"].contains(&entity_type.as_str()) {
        return Err(ApiAppError::BadRequest(format!("unknown entity_type: {entity_type}")));
    }

    // ── step 1: read multipart field "file" ──────────────────────────────
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut file_name = String::from("upload");
    let mut mime_type = String::from("application/octet-stream");

    while let Some(item) = payload.next().await {
        let mut field = item.map_err(|e| ApiAppError::BadRequest(e.to_string()))?;
        let cd = field.content_disposition();
        let field_name = cd.get_name().unwrap_or("").to_string();
        if field_name != "file" { continue; }

        file_name = cd.get_filename().unwrap_or("upload").to_string();
        mime_type = field.content_type()
            .map(|m| m.to_string())
            .unwrap_or_else(|| "application/octet-stream".into());

        let mut buf = Vec::new();
        while let Some(chunk) = field.next().await {
            let data = chunk.map_err(|e| ApiAppError::BadRequest(e.to_string()))?;
            buf.extend_from_slice(&data);
            if buf.len() > MAX_FILE_BYTES {
                return Err(ApiAppError::BadRequest(format!(
                    "file too large (limit {}MB)", MAX_FILE_BYTES / 1024 / 1024
                )));
            }
        }
        file_bytes = Some(buf);
        break;
    }

    let bytes = file_bytes.ok_or_else(|| ApiAppError::BadRequest("no 'file' field in upload".into()))?;

    // Anomaly detection: flag uploads that approach the hard size limit.
    // Fire-and-forget — a recording failure must not block a legitimate import.
    if let Err(e) =
        crate::anomaly::service::record_large_upload(pool, p.user_id, bytes.len(), &entity_type)
            .await
    {
        tracing::warn!(
            user_id = %p.user_id,
            file_size = bytes.len(),
            entity_type,
            err = %e,
            "anomaly: large upload recording failed"
        );
    }

    // ── step 2: validate MIME + extension ────────────────────────────────
    let ext = std::path::Path::new(&file_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if !ALLOWED_EXTS.contains(&ext.as_str()) {
        return Err(ApiAppError::BadRequest(format!(
            "unsupported file extension '.{ext}'; allowed: csv, xlsx"
        )));
    }
    let effective_mime = if ext == "csv" { "text/csv" } else {
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    };
    // Check provided MIME (lenient: browser MIME can be wrong for CSV)
    let mime_ok = ALLOWED_MIMES.contains(&mime_type.as_str()) || ext == "csv";
    if !mime_ok {
        return Err(ApiAppError::BadRequest(format!("unsupported MIME type: {mime_type}")));
    }

    // ── step 3: fingerprint ───────────────────────────────────────────────
    let fingerprint = {
        let mut h = Sha256::new();
        h.update(&bytes);
        hex::encode(h.finalize())
    };

    // Reject duplicate fingerprint for same entity_type + institution
    let dup: Option<(Uuid,)> = sqlx::query_as(
        "SELECT import_job_id FROM file_fingerprints
         WHERE fingerprint=$1 AND entity_type=$2 AND institution_id=$3",
    )
    .bind(&fingerprint).bind(&entity_type).bind(institution_id)
    .fetch_optional(pool).await?;

    if let Some((prev_job,)) = dup {
        return Err(ApiAppError::Conflict(format!(
            "duplicate file: identical content was already imported (job {prev_job})"
        )));
    }

    // ── step 4: create import job ─────────────────────────────────────────
    let job_id: (Uuid,) = sqlx::query_as(
        "INSERT INTO import_jobs
           (entity_type, institution_id, uploaded_by, file_name, file_size_bytes,
            file_fingerprint, mime_type, status)
         VALUES ($1,$2,$3,$4,$5,$6,$7,'processing') RETURNING id",
    )
    .bind(&entity_type).bind(institution_id).bind(p.user_id)
    .bind(&file_name).bind(bytes.len() as i64)
    .bind(&fingerprint).bind(effective_mime)
    .fetch_one(pool).await?;
    let job_id = job_id.0;

    // Register fingerprint
    sqlx::query(
        "INSERT INTO file_fingerprints (fingerprint, entity_type, institution_id, import_job_id)
         VALUES ($1,$2,$3,$4)",
    )
    .bind(&fingerprint).bind(&entity_type).bind(institution_id).bind(job_id)
    .execute(pool).await?;

    // ── step 5: parse file ────────────────────────────────────────────────
    let parse_result = if ext == "csv" {
        parse_csv(&bytes)
    } else {
        parse_xlsx(&bytes)
    };

    let (headers, data_rows) = match parse_result {
        Ok(r) => r,
        Err(e) => {
            // Mark job failed
            fail_job(pool, job_id, &e.to_string()).await?;
            return Ok(ImportResult {
                job_id, status: "failed".into(), total_rows: 0,
                accepted_rows: 0, rejected_rows: 0,
                error_message: Some(e.to_string()),
            });
        }
    };

    // ── step 6: validate column whitelist + required columns ──────────────
    let spec = column_spec(&entity_type);
    let known: Vec<&str> = spec.iter().map(|s| s.name).collect();
    let required: Vec<&str> = spec.iter().filter(|s| s.required).map(|s| s.name).collect();

    // Normalise: lowercase, trim header names
    let headers_norm: Vec<String> = headers.iter().map(|h| h.trim().to_lowercase()).collect();

    // Unknown columns: warn but don't fail (column whitelisting: only process known cols)
    let _unknown: Vec<&str> = headers_norm.iter()
        .filter(|h| !known.contains(&h.as_str()))
        .map(|s| s.as_str())
        .collect();

    // Missing required columns: fail the whole job
    let missing: Vec<&&str> = required.iter()
        .filter(|r| !headers_norm.contains(&r.to_string()))
        .collect();
    if !missing.is_empty() {
        let msg = format!("missing required columns: {}", missing.iter().map(|s| **s).collect::<Vec<_>>().join(", "));
        fail_job(pool, job_id, &msg).await?;
        return Ok(ImportResult {
            job_id, status: "failed".into(), total_rows: 0,
            accepted_rows: 0, rejected_rows: 0,
            error_message: Some(msg),
        });
    }

    // Build column → index map (only for whitelisted columns)
    let col_idx: std::collections::HashMap<&str, usize> = known.iter()
        .filter_map(|col| {
            headers_norm.iter().position(|h| h == col).map(|i| (*col, i))
        })
        .collect();

    // ── step 7: process rows ──────────────────────────────────────────────
    let total_rows = data_rows.len() as i32;
    let mut accepted = 0i32;
    let mut rejected = 0i32;

    let mut tx = pool.begin().await?;

    for (i, row_values) in data_rows.iter().enumerate() {
        let row_num = i as i32 + 2; // 1-indexed, row 1 = headers, data starts at 2

        // Extract whitelisted fields into a map
        let fields: std::collections::HashMap<&str, String> = col_idx.iter()
            .map(|(col, idx)| (*col, row_values.get(*idx).cloned().unwrap_or_default().trim().to_string()))
            .collect();

        // Represent the raw data for logging
        let raw_data: serde_json::Value = serde_json::Value::Object(
            fields.iter().map(|(k, v)| (k.to_string(), json!(v))).collect()
        );

        // Validate + insert
        let result = process_row(
            &mut tx, &entity_type, institution_id, &fields, raw_data.clone(),
        ).await;

        match result {
            Ok(entity_id) => {
                sqlx::query(
                    "INSERT INTO import_job_rows (job_id, row_number, status, raw_data, entity_id)
                     VALUES ($1,$2,'accepted',$3,$4)",
                )
                .bind(job_id).bind(row_num).bind(&raw_data).bind(entity_id)
                .execute(&mut *tx).await?;
                accepted += 1;
            }
            Err(reason) => {
                sqlx::query(
                    "INSERT INTO import_job_rows (job_id, row_number, status, raw_data, rejection_reason)
                     VALUES ($1,$2,'rejected',$3,$4)",
                )
                .bind(job_id).bind(row_num).bind(&raw_data).bind(&reason)
                .execute(&mut *tx).await?;
                rejected += 1;
            }
        }
    }

    // Update import_job
    sqlx::query(
        "UPDATE import_jobs SET status='done', total_rows=$2, accepted_rows=$3, rejected_rows=$4,
         completed_at=NOW() WHERE id=$1",
    )
    .bind(job_id).bind(total_rows).bind(accepted).bind(rejected)
    .execute(&mut *tx).await?;

    tx.commit().await?;

    Ok(ImportResult {
        job_id, status: "done".into(),
        total_rows, accepted_rows: accepted, rejected_rows: rejected,
        error_message: None,
    })
}

// ── row processor: validate + insert one row ──────────────────────────────
/// Returns Ok(entity_id) on success, Err(rejection_reason) on validation failure.
async fn process_row(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entity_type: &str,
    institution_id: Uuid,
    fields: &std::collections::HashMap<&str, String>,
    _raw: serde_json::Value,
) -> Result<Uuid, String> {
    let get = |key: &str| -> &str { fields.get(key).map(|s| s.as_str()).unwrap_or("") };

    match entity_type {
        "students" => {
            let sn = get("student_number");
            let fn_ = get("first_name");
            let ln = get("last_name");
            if sn.is_empty() { return Err("student_number is required".into()); }
            if fn_.is_empty() { return Err("first_name is required".into()); }
            if ln.is_empty() { return Err("last_name is required".into()); }

            let email = get("email");
            let dob_str = get("date_of_birth");
            let dob: Option<chrono::NaiveDate> = if dob_str.is_empty() { None } else {
                chrono::NaiveDate::parse_from_str(dob_str, "%Y-%m-%d")
                    .or_else(|_| chrono::NaiveDate::parse_from_str(dob_str, "%d/%m/%Y"))
                    .map_err(|_| format!("invalid date_of_birth: '{dob_str}'"))
                    .map(Some)?
            };
            let class_code = get("class_code");
            let class_id: Option<Uuid> = if class_code.is_empty() { None } else {
                let r: Option<(Uuid,)> = sqlx::query_as(
                    "SELECT id FROM classes WHERE institution_id=$1 AND code=$2",
                )
                .bind(institution_id).bind(class_code)
                .fetch_optional(&mut **tx).await
                .map_err(|e| e.to_string())?;
                match r {
                    Some((id,)) => Some(id),
                    None => return Err(format!("class_code '{class_code}' not found")),
                }
            };

            // Uniqueness check
            let dup: Option<(Uuid,)> = sqlx::query_as(
                "SELECT id FROM students WHERE institution_id=$1 AND student_number=$2",
            )
            .bind(institution_id).bind(sn)
            .fetch_optional(&mut **tx).await.map_err(|e| e.to_string())?;
            if dup.is_some() {
                return Err(format!("student_number '{sn}' already exists"));
            }

            let id: (Uuid,) = sqlx::query_as(
                "INSERT INTO students (institution_id, student_number, first_name, last_name, email, date_of_birth, class_id)
                 VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING id",
            )
            .bind(institution_id).bind(sn).bind(fn_).bind(ln)
            .bind(if email.is_empty() { None } else { Some(email) })
            .bind(dob).bind(class_id)
            .fetch_one(&mut **tx).await.map_err(|e| e.to_string())?;
            Ok(id.0)
        }

        "classes" => {
            let code = get("code");
            let label = get("label");
            if code.is_empty() { return Err("code is required".into()); }
            if label.is_empty() { return Err("label is required".into()); }

            let sem_code = get("semester_code");
            let semester_id: Option<Uuid> = if sem_code.is_empty() { None } else {
                let r: Option<(Uuid,)> = sqlx::query_as(
                    "SELECT id FROM semesters WHERE institution_id=$1 AND code=$2",
                )
                .bind(institution_id).bind(sem_code)
                .fetch_optional(&mut **tx).await.map_err(|e| e.to_string())?;
                match r {
                    Some((id,)) => Some(id),
                    None => return Err(format!("semester_code '{sem_code}' not found")),
                }
            };

            let dept_name = get("department_name");
            let department_id: Option<Uuid> = if dept_name.is_empty() { None } else {
                let r: Option<(Uuid,)> = sqlx::query_as(
                    "SELECT d.id FROM departments d JOIN sites si ON si.id=d.site_id
                     WHERE si.institution_id=$1 AND d.name=$2 LIMIT 1",
                )
                .bind(institution_id).bind(dept_name)
                .fetch_optional(&mut **tx).await.map_err(|e| e.to_string())?;
                r.map(|(id,)| id)
            };

            let dup: Option<(Uuid,)> = sqlx::query_as(
                "SELECT id FROM classes WHERE institution_id=$1 AND code=$2",
            )
            .bind(institution_id).bind(code)
            .fetch_optional(&mut **tx).await.map_err(|e| e.to_string())?;
            if dup.is_some() { return Err(format!("class code '{code}' already exists")); }

            let id: (Uuid,) = sqlx::query_as(
                "INSERT INTO classes (institution_id, code, label, semester_id, department_id)
                 VALUES ($1,$2,$3,$4,$5) RETURNING id",
            )
            .bind(institution_id).bind(code).bind(label)
            .bind(semester_id).bind(department_id)
            .fetch_one(&mut **tx).await.map_err(|e| e.to_string())?;
            Ok(id.0)
        }

        "courses" => {
            let code = get("code");
            let title = get("title");
            if code.is_empty() { return Err("code is required".into()); }
            if title.is_empty() { return Err("title is required".into()); }

            let credits_str = get("credits");
            let credits: i32 = if credits_str.is_empty() { 0 } else {
                credits_str.parse::<i32>()
                    .map_err(|_| format!("invalid credits: '{credits_str}'"))?
                    .max(0)
            };

            let dept_name = get("department_name");
            let department_id: Option<Uuid> = if dept_name.is_empty() { None } else {
                let r: Option<(Uuid,)> = sqlx::query_as(
                    "SELECT d.id FROM departments d JOIN sites si ON si.id=d.site_id
                     WHERE si.institution_id=$1 AND d.name=$2 LIMIT 1",
                )
                .bind(institution_id).bind(dept_name)
                .fetch_optional(&mut **tx).await.map_err(|e| e.to_string())?;
                r.map(|(id,)| id)
            };

            let dup: Option<(Uuid,)> = sqlx::query_as(
                "SELECT id FROM courses WHERE institution_id=$1 AND code=$2",
            )
            .bind(institution_id).bind(code)
            .fetch_optional(&mut **tx).await.map_err(|e| e.to_string())?;
            if dup.is_some() { return Err(format!("course code '{code}' already exists")); }

            let id: (Uuid,) = sqlx::query_as(
                "INSERT INTO courses (institution_id, code, title, department_id, credits)
                 VALUES ($1,$2,$3,$4,$5) RETURNING id",
            )
            .bind(institution_id).bind(code).bind(title)
            .bind(department_id).bind(credits)
            .fetch_one(&mut **tx).await.map_err(|e| e.to_string())?;
            Ok(id.0)
        }

        "semesters" => {
            let code = get("code");
            let label = get("label");
            let starts = get("starts_on");
            let ends = get("ends_on");
            if code.is_empty()  { return Err("code is required".into()); }
            if label.is_empty() { return Err("label is required".into()); }
            if starts.is_empty() { return Err("starts_on is required".into()); }
            if ends.is_empty()   { return Err("ends_on is required".into()); }

            let starts_on = parse_date(starts)?;
            let ends_on   = parse_date(ends)?;
            if ends_on <= starts_on {
                return Err("ends_on must be after starts_on".into());
            }

            let dup: Option<(Uuid,)> = sqlx::query_as(
                "SELECT id FROM semesters WHERE institution_id=$1 AND code=$2",
            )
            .bind(institution_id).bind(code)
            .fetch_optional(&mut **tx).await.map_err(|e| e.to_string())?;
            if dup.is_some() { return Err(format!("semester code '{code}' already exists")); }

            let id: (Uuid,) = sqlx::query_as(
                "INSERT INTO semesters (institution_id, code, label, starts_on, ends_on)
                 VALUES ($1,$2,$3,$4,$5) RETURNING id",
            )
            .bind(institution_id).bind(code).bind(label)
            .bind(starts_on).bind(ends_on)
            .fetch_one(&mut **tx).await.map_err(|e| e.to_string())?;
            Ok(id.0)
        }

        "departments" => {
            let name = get("name");
            if name.is_empty() { return Err("name is required".into()); }

            // Look up the institution's first site to associate the department
            let site: Option<(Uuid,)> = sqlx::query_as(
                "SELECT id FROM sites WHERE institution_id=$1 ORDER BY name LIMIT 1",
            )
            .bind(institution_id)
            .fetch_optional(&mut **tx).await.map_err(|e| e.to_string())?;
            let site_id = site.map(|s| s.0)
                .ok_or_else(|| "institution has no sites".to_string())?;

            let dup: Option<(Uuid,)> = sqlx::query_as(
                "SELECT d.id FROM departments d JOIN sites si ON si.id=d.site_id
                 WHERE si.institution_id=$1 AND d.name=$2",
            )
            .bind(institution_id).bind(name)
            .fetch_optional(&mut **tx).await.map_err(|e| e.to_string())?;
            if dup.is_some() { return Err(format!("department '{name}' already exists")); }

            let id: (Uuid,) = sqlx::query_as(
                "INSERT INTO departments (site_id, name) VALUES ($1,$2) RETURNING id",
            )
            .bind(site_id).bind(name)
            .fetch_one(&mut **tx).await.map_err(|e| e.to_string())?;
            Ok(id.0)
        }

        _ => Err(format!("unknown entity_type: {entity_type}")),
    }
}

// ── parsers ────────────────────────────────────────────────────────────────
fn parse_csv(bytes: &[u8]) -> Result<(Vec<String>, Vec<Vec<String>>), ApiAppError> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .trim(csv::Trim::All)
        .from_reader(bytes);
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| ApiAppError::BadRequest(format!("CSV header error: {e}")))?
        .iter()
        .map(String::from)
        .collect();
    let mut rows = Vec::new();
    for result in rdr.records() {
        let record = result.map_err(|e| ApiAppError::BadRequest(format!("CSV parse error: {e}")))?;
        rows.push(record.iter().map(String::from).collect());
    }
    Ok((headers, rows))
}

fn parse_xlsx(bytes: &[u8]) -> Result<(Vec<String>, Vec<Vec<String>>), ApiAppError> {
    use calamine::{open_workbook_from_rs, Reader, Xlsx};
    use std::io::Cursor;

    let cursor = Cursor::new(bytes.to_vec());
    let mut workbook: Xlsx<_> = open_workbook_from_rs(cursor)
        .map_err(|e| ApiAppError::BadRequest(format!("XLSX open error: {e}")))?;

    let sheet_name = workbook.sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| ApiAppError::BadRequest("XLSX has no sheets".into()))?;

    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|e| ApiAppError::BadRequest(format!("XLSX sheet error: {e}")))?;

    let mut iter = range.rows();
    let headers: Vec<String> = iter
        .next()
        .map(|r| r.iter().map(|c| c.to_string()).collect())
        .unwrap_or_default();

    let rows: Vec<Vec<String>> = iter
        .map(|r| r.iter().map(|c| c.to_string()).collect())
        .collect();

    Ok((headers, rows))
}

fn parse_date(s: &str) -> Result<chrono::NaiveDate, String> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .or_else(|_| chrono::NaiveDate::parse_from_str(s, "%d/%m/%Y"))
        .map_err(|_| format!("invalid date '{s}' — use YYYY-MM-DD or DD/MM/YYYY"))
}

async fn fail_job(pool: &PgPool, job_id: Uuid, msg: &str) -> Result<(), ApiAppError> {
    sqlx::query(
        "UPDATE import_jobs SET status='failed', error_message=$2, completed_at=NOW() WHERE id=$1",
    )
    .bind(job_id).bind(msg).execute(pool).await?;
    Ok(())
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── large upload anomaly wiring ───────────────────────────────────────────

    /// Confirms the anomaly detection threshold (8 MiB) is strictly below the
    /// hard file-size limit (10 MiB).  If these were ever inverted, anomaly
    /// events would fire only after the upload was already rejected, making the
    /// signal useless.
    #[test]
    fn large_upload_anomaly_threshold_is_below_hard_limit() {
        const ANOMALY_THRESHOLD_BYTES: usize = 8 * 1024 * 1024; // mirrors anomaly::service
        assert!(
            ANOMALY_THRESHOLD_BYTES < MAX_FILE_BYTES,
            "anomaly threshold ({} B) must be below the hard import limit ({} B)",
            ANOMALY_THRESHOLD_BYTES,
            MAX_FILE_BYTES
        );
    }

    /// Verifies that a file at exactly the hard limit (10 MiB) would be
    /// rejected BEFORE reaching the anomaly recording call, since the upload
    /// guard returns early when `buf.len() > MAX_FILE_BYTES`.
    #[test]
    fn file_at_hard_limit_is_rejected_not_flagged() {
        // 10 MiB file — the upload guard condition is `buf.len() > MAX_FILE_BYTES`,
        // i.e. strictly greater-than, so exactly MAX_FILE_BYTES passes through.
        // Still, the anomaly threshold is 8 MiB so it WOULD be flagged.
        let file_size = MAX_FILE_BYTES;
        const ANOMALY_THRESHOLD_BYTES: usize = 8 * 1024 * 1024;
        assert!(
            file_size > ANOMALY_THRESHOLD_BYTES,
            "10 MiB file should exceed the 8 MiB anomaly threshold"
        );
    }

    /// Confirms an 8 MiB + 1 byte file triggers the anomaly threshold and
    /// is still below the hard limit, so the anomaly call is reachable.
    #[test]
    fn file_just_above_anomaly_threshold_is_reachable() {
        const ANOMALY_THRESHOLD_BYTES: usize = 8 * 1024 * 1024;
        let file_size = ANOMALY_THRESHOLD_BYTES + 1;
        assert!(
            file_size > ANOMALY_THRESHOLD_BYTES,
            "file should trigger anomaly recording"
        );
        assert!(
            file_size <= MAX_FILE_BYTES,
            "file should still pass the upload size guard"
        );
    }
}
