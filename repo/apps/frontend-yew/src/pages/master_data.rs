/// Admin master-data management UI.
///
/// Tabs: Students | Classes | Courses | Semesters | Departments
/// Per-tab: paginated table, create form, file import (CSV/XLSX), export.
use serde::Deserialize;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{File, FormData, HtmlInputElement};
use yew::prelude::*;

use crate::auth::{use_auth, ApiClient};
use crate::components::card::Card;

// ── DTOs ──────────────────────────────────────────────────────────────────
#[derive(Clone, PartialEq, Deserialize, Debug)]
struct Institution {
    id: String,
    name: String,
    code: String,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct SemesterRow {
    id: String,
    code: String,
    label: String,
    starts_on: String,
    ends_on: String,
    is_active: bool,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct ClassRow {
    id: String,
    code: String,
    label: String,
    semester_code: Option<String>,
    department_name: Option<String>,
    is_active: bool,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct CourseRow {
    id: String,
    code: String,
    title: String,
    department_name: Option<String>,
    credits: i32,
    is_active: bool,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct StudentRow {
    id: String,
    student_number: String,
    first_name: String,
    last_name: String,
    email: Option<String>,
    class_code: Option<String>,
    is_active: bool,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct DepartmentRow {
    id: String,
    name: String,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct Page<T: Clone + PartialEq + 'static> {
    items: Vec<T>,
    total: i64,
    page: i64,
    per_page: i64,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct ImportResult {
    job_id: String,
    status: String,
    total_rows: i32,
    accepted_rows: i32,
    rejected_rows: i32,
    error_message: Option<String>,
}

// ── entity tab ─────────────────────────────────────────────────────────────
#[derive(Clone, PartialEq, Debug, Copy)]
enum EntityTab {
    Students,
    Classes,
    Courses,
    Semesters,
    Departments,
}

impl EntityTab {
    fn label(self) -> &'static str {
        match self {
            EntityTab::Students => "Students",
            EntityTab::Classes => "Classes",
            EntityTab::Courses => "Courses",
            EntityTab::Semesters => "Semesters",
            EntityTab::Departments => "Departments",
        }
    }
    fn api_key(self) -> &'static str {
        match self {
            EntityTab::Students => "students",
            EntityTab::Classes => "classes",
            EntityTab::Courses => "courses",
            EntityTab::Semesters => "semesters",
            EntityTab::Departments => "departments",
        }
    }
    fn all() -> [EntityTab; 5] {
        [
            EntityTab::Students,
            EntityTab::Classes,
            EntityTab::Courses,
            EntityTab::Semesters,
            EntityTab::Departments,
        ]
    }
}

// ── table data union ───────────────────────────────────────────────────────
#[derive(Clone, PartialEq, Debug)]
enum EntityData {
    Students(Vec<StudentRow>),
    Classes(Vec<ClassRow>),
    Courses(Vec<CourseRow>),
    Semesters(Vec<SemesterRow>),
    Departments(Vec<DepartmentRow>),
}

// ── component ──────────────────────────────────────────────────────────────
#[function_component(MasterDataPage)]
pub fn master_data() -> Html {
    let auth = use_auth();

    // UI state
    let institutions = use_state(|| Vec::<Institution>::new());
    let selected_iid = use_state(|| String::new());
    let tab = use_state(|| EntityTab::Students);
    let page_num = use_state(|| 1i64);
    let filter = use_state(|| String::new());
    let refresh_token = use_state(|| 0u32);

    // Data state
    let data = use_state(|| Option::<Result<EntityData, String>>::None);
    let total = use_state(|| 0i64);

    // Form state
    let show_form = use_state(|| false);
    let form_fields = use_state(|| std::collections::HashMap::<String, String>::new());
    let form_err = use_state(|| Option::<String>::None);
    let form_saving = use_state(|| false);

    // Import state
    let import_result = use_state(|| Option::<Result<ImportResult, String>>::None);
    let importing = use_state(|| false);

    // ── load institutions on mount ─────────────────────────────────────
    {
        let institutions = institutions.clone();
        let selected_iid = selected_iid.clone();
        let auth = auth.clone();
        use_effect_with((), move |_| {
            let institutions = institutions.clone();
            let selected_iid = selected_iid.clone();
            let token = auth.api_token.clone();
            let key = auth.signing_key.clone();
            spawn_local(async move {
                if let (Some(token), Some(key)) = (token, key) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    if let Ok(r) = client.get("/api/v1/master-data/institutions").await {
                        if let Ok(v) = r.json::<Vec<Institution>>().await {
                            if let Some(first) = v.first() {
                                selected_iid.set(first.id.clone());
                            }
                            institutions.set(v);
                        }
                    }
                }
            });
            || ()
        });
    }

    // ── load entity data when iid / tab / page / filter changes ────────
    {
        let data = data.clone();
        let total = total.clone();
        let auth = auth.clone();
        let iid = (*selected_iid).clone();
        let current_tab = *tab;
        let current_page = *page_num;
        let current_filter = (*filter).clone();
        let refresh_token_val = *refresh_token;

        use_effect_with(
            (iid.clone(), current_tab, current_page, current_filter.clone(), refresh_token_val),
            move |_| {
                if !iid.is_empty() {
                    let data = data.clone();
                    let total = total.clone();
                    let token = auth.api_token.clone();
                    let key = auth.signing_key.clone();
                    data.set(None);
                    spawn_local(async move {
                        if let (Some(token), Some(key)) = (token, key) {
                            let client = ApiClient { api_token: token, signing_key: key };
                            let encoded_filter = current_filter.replace(' ', "+");
                            let url = format!(
                                "/api/v1/master-data/institutions/{}/{}?page={}&per_page=20&filter={}",
                                iid, current_tab.api_key(), current_page, encoded_filter
                            );
                            match client.get(&url).await {
                                Ok(r) if r.ok() => {
                                    let entity_data = match current_tab {
                                        EntityTab::Students => r.json::<Page<StudentRow>>().await
                                            .map(|p| { total.set(p.total); EntityData::Students(p.items) })
                                            .map_err(|e| e.to_string()),
                                        EntityTab::Classes => r.json::<Page<ClassRow>>().await
                                            .map(|p| { total.set(p.total); EntityData::Classes(p.items) })
                                            .map_err(|e| e.to_string()),
                                        EntityTab::Courses => r.json::<Page<CourseRow>>().await
                                            .map(|p| { total.set(p.total); EntityData::Courses(p.items) })
                                            .map_err(|e| e.to_string()),
                                        EntityTab::Semesters => r.json::<Page<SemesterRow>>().await
                                            .map(|p| { total.set(p.total); EntityData::Semesters(p.items) })
                                            .map_err(|e| e.to_string()),
                                        EntityTab::Departments => r.json::<Page<DepartmentRow>>().await
                                            .map(|p| { total.set(p.total); EntityData::Departments(p.items) })
                                            .map_err(|e| e.to_string()),
                                    };
                                    data.set(Some(entity_data));
                                }
                                Ok(r) => data.set(Some(Err(format!("HTTP {}", r.status())))),
                                Err(e) => data.set(Some(Err(e.to_string()))),
                            }
                        }
                    });
                }
                || ()
            },
        );
    }

    // ── helpers: bump refresh_token to force a data re-fetch ──────────
    let refresh = {
        let refresh_token = refresh_token.clone();
        Callback::from(move |_: ()| refresh_token.set(*refresh_token + 1))
    };

    // ── institution selector ───────────────────────────────────────────
    let on_iid_change = {
        let selected_iid = selected_iid.clone();
        let page_num = page_num.clone();
        Callback::from(move |e: Event| {
            let el: HtmlInputElement = e.target_unchecked_into();
            selected_iid.set(el.value());
            page_num.set(1);
        })
    };

    // ── tab switch ─────────────────────────────────────────────────────
    let on_tab = {
        let tab = tab.clone();
        let page_num = page_num.clone();
        let show_form = show_form.clone();
        let import_result = import_result.clone();
        Callback::from(move |t: EntityTab| {
            tab.set(t);
            page_num.set(1);
            show_form.set(false);
            import_result.set(None);
        })
    };

    // ── filter ─────────────────────────────────────────────────────────
    let on_filter = {
        let filter = filter.clone();
        let page_num = page_num.clone();
        Callback::from(move |e: InputEvent| {
            let el: HtmlInputElement = e.target_unchecked_into();
            filter.set(el.value());
            page_num.set(1);
        })
    };

    // ── delete ─────────────────────────────────────────────────────────
    let make_delete = {
        let auth = auth.clone();
        let tab = tab.clone();
        let refresh = refresh.clone();
        let data = data.clone();
        move |entity_id: String| {
            let auth = auth.clone();
            let tab = *tab;
            let refresh = refresh.clone();
            let data = data.clone();
            Callback::from(move |_: MouseEvent| {
                if !gloo::dialogs::confirm(&format!("Delete this {}?", tab.label())) {
                    return;
                }
                let auth = auth.clone();
                let entity_id = entity_id.clone();
                let tab = tab;
                let refresh = refresh.clone();
                let data = data.clone();
                spawn_local(async move {
                    if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                        let client = ApiClient { api_token: token, signing_key: key };
                        let url = format!("/api/v1/master-data/{}/{}/delete", tab.api_key(), entity_id);
                        match client.post_json(&url, &serde_json::json!({})).await {
                            Ok(_) => { data.set(None); refresh.emit(()); }
                            Err(e) => {
                                gloo::dialogs::alert(&format!("Delete failed: {e}"));
                            }
                        }
                    }
                });
            })
        }
    };

    // ── create form submit ─────────────────────────────────────────────
    let on_create = {
        let auth = auth.clone();
        let tab = tab.clone();
        let selected_iid = selected_iid.clone();
        let form_fields = form_fields.clone();
        let form_err = form_err.clone();
        let form_saving = form_saving.clone();
        let show_form = show_form.clone();
        let refresh = refresh.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let auth = auth.clone();
            let current_tab = *tab;
            let iid = (*selected_iid).clone();
            let fields = (*form_fields).clone();
            let form_err = form_err.clone();
            let form_saving = form_saving.clone();
            let show_form = show_form.clone();
            let refresh = refresh.clone();
            form_saving.set(true);
            form_err.set(None);
            spawn_local(async move {
                if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    let body = build_create_body(current_tab, &fields);
                    let url = format!("/api/v1/master-data/institutions/{}/{}", iid, current_tab.api_key());
                    match client.post_json(&url, &body).await {
                        Ok(r) if r.ok() => {
                            show_form.set(false);
                            refresh.emit(());
                        }
                        Ok(r) => {
                            let msg = r.text().await.unwrap_or_default();
                            form_err.set(Some(msg));
                        }
                        Err(e) => form_err.set(Some(e)),
                    }
                }
                form_saving.set(false);
            });
        })
    };

    // ── form field update ──────────────────────────────────────────────
    let on_field = {
        let form_fields = form_fields.clone();
        move |key: &'static str| {
            let form_fields = form_fields.clone();
            Callback::from(move |e: InputEvent| {
                let el: HtmlInputElement = e.target_unchecked_into();
                let mut m = (*form_fields).clone();
                m.insert(key.to_string(), el.value());
                form_fields.set(m);
            })
        }
    };

    // ── import file chosen ─────────────────────────────────────────────
    let on_import_file = {
        let auth = auth.clone();
        let tab = tab.clone();
        let selected_iid = selected_iid.clone();
        let importing = importing.clone();
        let import_result = import_result.clone();
        Callback::from(move |e: Event| {
            let el: HtmlInputElement = e.target_unchecked_into();
            let files = el.files();
            let file: Option<File> = files.and_then(|fl| fl.get(0));
            let Some(file) = file else { return; };
            let auth = auth.clone();
            let current_tab = *tab;
            let iid = (*selected_iid).clone();
            let importing = importing.clone();
            let import_result = import_result.clone();
            importing.set(true);
            import_result.set(None);
            spawn_local(async move {
                if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    let form = FormData::new().unwrap();
                    form.append_with_blob("file", &file).unwrap();
                    let url = format!(
                        "/api/v1/master-data/institutions/{}/import/{}",
                        iid, current_tab.api_key()
                    );
                    match client.post_multipart(&url, form).await {
                        Ok(r) => match r.json::<ImportResult>().await {
                            Ok(v) => import_result.set(Some(Ok(v))),
                            Err(e) => import_result.set(Some(Err(e.to_string()))),
                        },
                        Err(e) => import_result.set(Some(Err(e))),
                    }
                }
                importing.set(false);
            });
        })
    };

    // ── export ─────────────────────────────────────────────────────────
    let on_export = {
        let auth = auth.clone();
        let tab = tab.clone();
        let selected_iid = selected_iid.clone();
        move |fmt: &'static str| {
            let auth = auth.clone();
            let current_tab = *tab;
            let iid = (*selected_iid).clone();
            Callback::from(move |_: MouseEvent| {
                let auth = auth.clone();
                let iid = iid.clone();
                spawn_local(async move {
                    if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                        let client = ApiClient { api_token: token, signing_key: key };
                        let url = format!(
                            "/api/v1/master-data/institutions/{}/export/{}?format={}",
                            iid, current_tab.api_key(), fmt
                        );
                        if let Ok(r) = client.get(&url).await {
                            if let Ok(bytes) = r.binary().await {
                                trigger_download(&bytes, &format!("{}.{}", current_tab.api_key(), fmt), fmt);
                            }
                        }
                    }
                });
            })
        }
    };

    // ── render ─────────────────────────────────────────────────────────
    let iid_val = (*selected_iid).clone();

    html! {
        <>
            <div class="page-header">
                <h1>{"Master Data"}</h1>
                <p>{"CRUD, bulk import (CSV/XLSX) and export for reference tables."}</p>
            </div>

            // Institution selector
            <Card>
                <div class="form-row" style="display:flex;align-items:center;gap:1rem;">
                    <label><strong>{"Institution:"}</strong></label>
                    <select class="input" onchange={on_iid_change}>
                        { for (*institutions).iter().map(|inst| {
                            let selected = inst.id == iid_val;
                            html! {
                                <option value={inst.id.clone()} selected={selected}>
                                    {format!("{} ({})", inst.name, inst.code)}
                                </option>
                            }
                        })}
                    </select>
                </div>
            </Card>

            // Entity tabs
            if !iid_val.is_empty() {
                <Card>
                    // Tab bar
                    <div class="tabs" style="display:flex;gap:0.5rem;margin-bottom:1rem;border-bottom:1px solid #ddd;padding-bottom:0.5rem;">
                        { for EntityTab::all().into_iter().map(|t| {
                            let active = *tab == t;
                            let on_tab = on_tab.clone();
                            html! {
                                <button
                                    class={if active { "button" } else { "button button--ghost" }}
                                    onclick={Callback::from(move |_| on_tab.emit(t))}
                                >
                                    {t.label()}
                                </button>
                            }
                        })}
                    </div>

                    // Filter + actions bar
                    <div style="display:flex;gap:1rem;align-items:center;margin-bottom:1rem;flex-wrap:wrap;">
                        <input
                            class="input"
                            placeholder="Filter…"
                            value={(*filter).clone()}
                            oninput={on_filter}
                            style="flex:1;min-width:200px;"
                        />
                        <button
                            class="button button--secondary"
                            onclick={Callback::from({
                                let show_form = show_form.clone();
                                let form_fields = form_fields.clone();
                                let form_err = form_err.clone();
                                move |_| {
                                    form_fields.set(Default::default());
                                    form_err.set(None);
                                    show_form.set(!*show_form);
                                }
                            })}
                        >
                            {if *show_form { "Cancel" } else { "+ New" }}
                        </button>
                        // Import
                        <label class="button button--secondary" style="cursor:pointer;">
                            {if *importing { "Importing…" } else { "Import CSV/XLSX" }}
                            <input
                                type="file"
                                accept=".csv,.xlsx"
                                style="display:none;"
                                onchange={on_import_file}
                                disabled={*importing}
                            />
                        </label>
                        <button class="button button--ghost" onclick={on_export("csv")}>{"Export CSV"}</button>
                        <button class="button button--ghost" onclick={on_export("xlsx")}>{"Export XLSX"}</button>
                    </div>

                    // Import result banner
                    if let Some(ref ir) = *import_result {
                        <div class={if ir.is_ok() { "banner banner--success" } else { "banner banner--error" }}
                             style="margin-bottom:1rem;padding:0.75rem;border-radius:4px;background:#f0fff0;">
                        {match ir {
                            Ok(r) => html! {
                                <>
                                    <strong>{format!("Import {}: ", r.status)}</strong>
                                    {format!("{} total / {} accepted / {} rejected",
                                        r.total_rows, r.accepted_rows, r.rejected_rows)}
                                    if let Some(ref msg) = r.error_message {
                                        <div style="color:red;">{msg}</div>
                                    }
                                </>
                            },
                            Err(e) => html! { <span style="color:red;">{e}</span> },
                        }}
                        </div>
                    }

                    // Create form
                    if *show_form {
                        <div style="background:#f8f8f8;border:1px solid #ddd;border-radius:4px;padding:1rem;margin-bottom:1rem;">
                            <h3 style="margin-top:0;">{format!("New {}", (*tab).label())}</h3>
                            <form onsubmit={on_create}>
                                { render_create_form(*tab, &on_field, &form_fields) }
                                if let Some(ref e) = *form_err {
                                    <div style="color:red;margin:0.5rem 0;">{e}</div>
                                }
                                <button class="button" type="submit" disabled={*form_saving}>
                                    {if *form_saving { "Saving…" } else { "Create" }}
                                </button>
                            </form>
                        </div>
                    }

                    // Pagination info
                    <div style="margin-bottom:0.5rem;font-size:0.85rem;color:#666;">
                        {format!("Total: {} record(s)", *total)}
                        {if *total > 20 {
                            format!(" — page {} of {}", *page_num, (*total as f64 / 20.0).ceil() as i64)
                        } else { String::new() }}
                    </div>

                    // Data table
                    { render_table(&data, &make_delete) }

                    // Pagination controls
                    if *total > 20 {
                        <div style="display:flex;gap:0.5rem;margin-top:1rem;">
                            <button class="button button--ghost"
                                disabled={*page_num <= 1}
                                onclick={Callback::from({
                                    let page_num = page_num.clone();
                                    move |_| { if *page_num > 1 { page_num.set(*page_num - 1); } }
                                })}>
                                {"Prev"}
                            </button>
                            <button class="button button--ghost"
                                disabled={*page_num * 20 >= *total}
                                onclick={Callback::from({
                                    let page_num = page_num.clone();
                                    move |_| { page_num.set(*page_num + 1); }
                                })}>
                                {"Next"}
                            </button>
                        </div>
                    }
                </Card>
            }
        </>
    }
}

// ── form field renderer ────────────────────────────────────────────────────
fn render_create_form(
    tab: EntityTab,
    on_field: &dyn Fn(&'static str) -> Callback<InputEvent>,
    fields: &std::collections::HashMap<String, String>,
) -> Html {
    let v = |k: &str| fields.get(k).cloned().unwrap_or_default();
    let row = |label: &str, key: &'static str, typ: &'static str, required: bool, on_field: &dyn Fn(&'static str) -> Callback<InputEvent>| {
        html! {
            <div style="margin-bottom:0.5rem;display:flex;gap:0.5rem;align-items:center;">
                <label style="min-width:150px;font-size:0.9rem;">{label}</label>
                <input
                    class="input"
                    type={typ}
                    required={required}
                    value={v(key)}
                    oninput={on_field(key)}
                    style="flex:1;"
                />
            </div>
        }
    };

    match tab {
        EntityTab::Students => html! {
            <>
                { row("Student number *", "student_number", "text", true, on_field) }
                { row("First name *", "first_name", "text", true, on_field) }
                { row("Last name *", "last_name", "text", true, on_field) }
                { row("Email", "email", "email", false, on_field) }
                { row("Date of birth", "date_of_birth", "date", false, on_field) }
                { row("Class ID (UUID)", "class_id", "text", false, on_field) }
            </>
        },
        EntityTab::Classes => html! {
            <>
                { row("Code *", "code", "text", true, on_field) }
                { row("Label *", "label", "text", true, on_field) }
                { row("Semester ID (UUID)", "semester_id", "text", false, on_field) }
                { row("Department ID (UUID)", "department_id", "text", false, on_field) }
            </>
        },
        EntityTab::Courses => html! {
            <>
                { row("Code *", "code", "text", true, on_field) }
                { row("Title *", "title", "text", true, on_field) }
                { row("Department ID (UUID)", "department_id", "text", false, on_field) }
                { row("Credits", "credits", "number", false, on_field) }
            </>
        },
        EntityTab::Semesters => html! {
            <>
                { row("Code *", "code", "text", true, on_field) }
                { row("Label *", "label", "text", true, on_field) }
                { row("Starts on *", "starts_on", "date", true, on_field) }
                { row("Ends on *", "ends_on", "date", true, on_field) }
            </>
        },
        EntityTab::Departments => html! {
            <>
                { row("Name *", "name", "text", true, on_field) }
                { row("Site ID (UUID) *", "site_id", "text", true, on_field) }
            </>
        },
    }
}

// ── JSON body builder ──────────────────────────────────────────────────────
fn build_create_body(
    tab: EntityTab,
    fields: &std::collections::HashMap<String, String>,
) -> serde_json::Value {
    let s = |k: &str| fields.get(k).cloned().unwrap_or_default();
    let opt_s = |k: &str| -> serde_json::Value {
        match fields.get(k).filter(|v| !v.is_empty()) {
            Some(v) => serde_json::Value::String(v.clone()),
            None => serde_json::Value::Null,
        }
    };

    match tab {
        EntityTab::Students => serde_json::json!({
            "student_number": s("student_number"),
            "first_name": s("first_name"),
            "last_name": s("last_name"),
            "email": opt_s("email"),
            "date_of_birth": opt_s("date_of_birth"),
            "class_id": opt_s("class_id"),
            "is_active": true,
        }),
        EntityTab::Classes => serde_json::json!({
            "code": s("code"),
            "label": s("label"),
            "semester_id": opt_s("semester_id"),
            "department_id": opt_s("department_id"),
            "is_active": true,
        }),
        EntityTab::Courses => serde_json::json!({
            "code": s("code"),
            "title": s("title"),
            "department_id": opt_s("department_id"),
            "credits": s("credits").parse::<i32>().unwrap_or(0),
            "is_active": true,
        }),
        EntityTab::Semesters => serde_json::json!({
            "code": s("code"),
            "label": s("label"),
            "starts_on": s("starts_on"),
            "ends_on": s("ends_on"),
            "is_active": true,
        }),
        EntityTab::Departments => serde_json::json!({
            "name": s("name"),
            "site_id": s("site_id"),
        }),
    }
}

// ── table renderer ─────────────────────────────────────────────────────────
fn render_table(
    data: &UseStateHandle<Option<Result<EntityData, String>>>,
    make_delete: &dyn Fn(String) -> Callback<MouseEvent>,
) -> Html {
    match &**data {
        None => html! { <div style="padding:2rem;text-align:center;color:#888;">{"Loading…"}</div> },
        Some(Err(e)) => html! { <div style="color:red;padding:1rem;">{e}</div> },
        Some(Ok(EntityData::Students(rows))) => html! {
            <div style="overflow-x:auto;">
            <table class="table" style="width:100%;">
                <thead><tr>
                    <th>{"Number"}</th><th>{"Name"}</th><th>{"Email"}</th>
                    <th>{"Class"}</th><th>{"Active"}</th><th></th>
                </tr></thead>
                <tbody>
                { for rows.iter().map(|r| {
                    let del = make_delete(r.id.clone());
                    html! { <tr>
                        <td>{ &r.student_number }</td>
                        <td>{ format!("{} {}", r.first_name, r.last_name) }</td>
                        <td>{ r.email.as_deref().unwrap_or("—") }</td>
                        <td>{ r.class_code.as_deref().unwrap_or("—") }</td>
                        <td>{ if r.is_active { "Yes" } else { "No" } }</td>
                        <td><button class="button button--danger button--sm" onclick={del}>{"Delete"}</button></td>
                    </tr> }
                })}
                </tbody>
            </table>
            </div>
        },
        Some(Ok(EntityData::Classes(rows))) => html! {
            <div style="overflow-x:auto;">
            <table class="table" style="width:100%;">
                <thead><tr>
                    <th>{"Code"}</th><th>{"Label"}</th><th>{"Semester"}</th>
                    <th>{"Department"}</th><th>{"Active"}</th><th></th>
                </tr></thead>
                <tbody>
                { for rows.iter().map(|r| {
                    let del = make_delete(r.id.clone());
                    html! { <tr>
                        <td>{ &r.code }</td>
                        <td>{ &r.label }</td>
                        <td>{ r.semester_code.as_deref().unwrap_or("—") }</td>
                        <td>{ r.department_name.as_deref().unwrap_or("—") }</td>
                        <td>{ if r.is_active { "Yes" } else { "No" } }</td>
                        <td><button class="button button--danger button--sm" onclick={del}>{"Delete"}</button></td>
                    </tr> }
                })}
                </tbody>
            </table>
            </div>
        },
        Some(Ok(EntityData::Courses(rows))) => html! {
            <div style="overflow-x:auto;">
            <table class="table" style="width:100%;">
                <thead><tr>
                    <th>{"Code"}</th><th>{"Title"}</th><th>{"Department"}</th>
                    <th>{"Credits"}</th><th>{"Active"}</th><th></th>
                </tr></thead>
                <tbody>
                { for rows.iter().map(|r| {
                    let del = make_delete(r.id.clone());
                    html! { <tr>
                        <td>{ &r.code }</td>
                        <td>{ &r.title }</td>
                        <td>{ r.department_name.as_deref().unwrap_or("—") }</td>
                        <td>{ r.credits }</td>
                        <td>{ if r.is_active { "Yes" } else { "No" } }</td>
                        <td><button class="button button--danger button--sm" onclick={del}>{"Delete"}</button></td>
                    </tr> }
                })}
                </tbody>
            </table>
            </div>
        },
        Some(Ok(EntityData::Semesters(rows))) => html! {
            <div style="overflow-x:auto;">
            <table class="table" style="width:100%;">
                <thead><tr>
                    <th>{"Code"}</th><th>{"Label"}</th>
                    <th>{"Starts"}</th><th>{"Ends"}</th><th>{"Active"}</th><th></th>
                </tr></thead>
                <tbody>
                { for rows.iter().map(|r| {
                    let del = make_delete(r.id.clone());
                    html! { <tr>
                        <td>{ &r.code }</td>
                        <td>{ &r.label }</td>
                        <td>{ &r.starts_on }</td>
                        <td>{ &r.ends_on }</td>
                        <td>{ if r.is_active { "Yes" } else { "No" } }</td>
                        <td><button class="button button--danger button--sm" onclick={del}>{"Delete"}</button></td>
                    </tr> }
                })}
                </tbody>
            </table>
            </div>
        },
        Some(Ok(EntityData::Departments(rows))) => html! {
            <div style="overflow-x:auto;">
            <table class="table" style="width:100%;">
                <thead><tr>
                    <th>{"Name"}</th><th></th>
                </tr></thead>
                <tbody>
                { for rows.iter().map(|r| {
                    let del = make_delete(r.id.clone());
                    html! { <tr>
                        <td>{ &r.name }</td>
                        <td><button class="button button--danger button--sm" onclick={del}>{"Delete"}</button></td>
                    </tr> }
                })}
                </tbody>
            </table>
            </div>
        },
    }
}

// ── download helper ────────────────────────────────────────────────────────
fn trigger_download(bytes: &[u8], filename: &str, fmt: &str) {
    let content_type = if fmt == "xlsx" {
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    } else {
        "text/csv"
    };
    // Build a Uint8Array and wrap it in a JS Array for Blob construction.
    let uint8 = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
    uint8.copy_from(bytes);
    let parts = js_sys::Array::of1(&uint8);
    let mut opts = web_sys::BlobPropertyBag::new();
    opts.type_(content_type);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).unwrap();
    let url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();
    let doc = gloo::utils::document();
    let a = doc.create_element("a").unwrap();
    a.set_attribute("href", &url).unwrap();
    a.set_attribute("download", filename).unwrap();
    doc.body().unwrap().append_child(&a).unwrap();
    a.unchecked_ref::<web_sys::HtmlElement>().click();
    doc.body().unwrap().remove_child(&a).unwrap();
    web_sys::Url::revoke_object_url(&url).unwrap();
}
