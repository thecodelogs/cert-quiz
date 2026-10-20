use crate::{auth, db, render, AppState};
use axum::{
    body::Bytes,
    extract::{FromRequestParts, Path, Query, State},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Router,
};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use sqlx::Row;
use std::sync::Arc;
use tera::Context;
use uuid::Uuid;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/login", get(login_page).post(login_submit))
        .route("/logout", post(logout))
        .route("/", get(dashboard))
        .route("/questions", get(questions_list_page))
        .route("/questions/new", get(question_new_page).post(question_create))
        .route("/questions/:id", get(question_edit_page).post(question_update))
        .route("/questions/delete", post(questions_delete))
        .route("/questions/:id/delete", post(question_delete))
        .route("/flagged", get(flagged_page).post(flagged_action))
        .route("/import", get(import_page).post(import_submit))
        .route("/users", get(users_page).post(users_toggle))
        .route("/tracks/:slug/toggle-published", post(track_toggle_published))
}

/* ---------- auth gate ---------- */

/// A logged-in user who holds the `admin` role. Any handler that takes this
/// as a parameter is gated behind it — axum runs extraction before the
/// handler body, so an unauthenticated or non-admin request never reaches
/// the handler at all.
pub struct AdminUser {
    pub id: Uuid,
    pub name: String,
}

pub enum AdminGate {
    NotLoggedIn,
    /// Logged in, but without the `admin` role — carries the rendered 403 page.
    Forbidden(Response),
}

impl IntoResponse for AdminGate {
    fn into_response(self) -> Response {
        match self {
            AdminGate::NotLoggedIn => Redirect::to("/admin/login").into_response(),
            AdminGate::Forbidden(page) => page,
        }
    }
}

#[axum::async_trait]
impl FromRequestParts<Arc<AppState>> for AdminUser {
    type Rejection = AdminGate;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let session_id = jar
            .get(auth::SESSION_COOKIE)
            .and_then(|c| Uuid::parse_str(c.value()).ok())
            .ok_or(AdminGate::NotLoggedIn)?;

        let row = sqlx::query(
            "select u.id, u.name, u.email::text as email,
                    exists (
                        select 1 from user_roles ur
                        join roles r on r.id = ur.role_id
                        where ur.user_id = u.id and r.name = 'admin'
                    ) as is_admin
             from sessions s
             join users u on u.id = s.user_id
             where s.id = $1 and s.expires_at > now()",
        )
        .bind(session_id)
        .fetch_optional(&state.db)
        .await
        .expect("session lookup failed");

        let row = row.ok_or(AdminGate::NotLoggedIn)?;
        if !row.try_get::<bool, _>("is_admin").unwrap_or(false) {
            let mut ctx = Context::new();
            ctx.insert("name", &row.try_get::<String, _>("name").expect("users.name"));
            ctx.insert("email", &row.try_get::<String, _>("email").expect("users.email"));
            let page = render(state, "admin/forbidden.html", &ctx);
            return Err(AdminGate::Forbidden((StatusCode::FORBIDDEN, page).into_response()));
        }

        Ok(AdminUser {
            id: row.try_get("id").expect("users.id"),
            name: row.try_get("name").expect("users.name"),
        })
    }
}

/* ---------- login / logout ---------- */

#[derive(Deserialize)]
struct LoginForm {
    email: String,
    password: String,
}

async fn login_page(State(state): State<Arc<AppState>>) -> Response {
    render(&state, "admin/login.html", &Context::new())
}

fn login_error(state: &AppState, message: &str) -> Response {
    let mut ctx = Context::new();
    ctx.insert("error", message);
    render(state, "admin/login.html", &ctx)
}

async fn login_submit(State(state): State<Arc<AppState>>, axum::Form(form): axum::Form<LoginForm>) -> Response {
    let row = sqlx::query("select id, password_hash, status from users where email = $1")
        .bind(&form.email)
        .fetch_optional(&state.db)
        .await
        .expect("user lookup failed");

    let Some(row) = row else {
        return login_error(&state, "Incorrect email or password.");
    };

    let user_id: Uuid = row.try_get("id").expect("users.id");
    let password_hash: String = row.try_get("password_hash").expect("users.password_hash");
    let status: String = row.try_get("status").expect("users.status");

    if !auth::verify_password(&form.password, &password_hash) {
        return login_error(&state, "Incorrect email or password.");
    }
    if status == "suspended" {
        return login_error(&state, "This account has been suspended.");
    }

    let jar = CookieJar::new().add(auth::create_session(&state.db, user_id).await);

    (jar, Redirect::to("/admin")).into_response()
}

async fn logout(State(state): State<Arc<AppState>>, jar: CookieJar) -> Response {
    let jar = auth::end_session(&state.db, jar).await;
    (jar, Redirect::to("/admin/login")).into_response()
}

/* ---------- shared: sidebar context ---------- */

async fn sidebar_context(state: &AppState, active: &str, admin: &AdminUser) -> Context {
    let (nav_questions, nav_flagged, nav_users) = db::nav_counts(&state.db).await;
    let mut ctx = Context::new();
    ctx.insert("active", active);
    ctx.insert("nav_questions", &nav_questions);
    ctx.insert("nav_flagged", &nav_flagged);
    ctx.insert("nav_users", &nav_users);
    ctx.insert("admin_name", &admin.name);
    ctx
}

/* ---------- dashboard ---------- */

async fn dashboard(State(state): State<Arc<AppState>>, admin: AdminUser) -> Response {
    let stats = db::dashboard_stats(&state.db).await;
    let attention = db::needs_attention(&state.db, 6).await;
    let weak_domains = db::weakest_domains(&state.db, 5).await;
    let last_import = db::last_import(&state.db).await;
    let tracks = db::admin_tracks(&state.db).await;
    let today = db::today_label(&state.db).await;

    let mut ctx = sidebar_context(&state, "dashboard", &admin).await;
    ctx.insert("today", &today);
    ctx.insert("stats", &stats);
    ctx.insert("attention", &attention);
    ctx.insert("weak_domains", &weak_domains);
    ctx.insert("last_import", &last_import);
    ctx.insert("tracks", &tracks);
    render(&state, "admin/dashboard.html", &ctx)
}

async fn track_toggle_published(State(state): State<Arc<AppState>>, _admin: AdminUser, Path(slug): Path<String>) -> Response {
    db::toggle_track_published(&state.db, &slug).await;
    Redirect::to("/admin").into_response()
}

/* ---------- questions: list + editor drawer ---------- */

/// `/admin/questions?…` with the current filters, sort and page size, and
/// `set` overriding individual params (an empty value drops one). `page` is
/// only carried when it's in `set`, so any change of filter, sort or page size
/// lands back on page 1.
fn list_href(filter: &db::QuestionFilter, set: &[(&str, String)]) -> String {
    let current = [
        ("q", filter.q.clone()),
        ("track", filter.track.clone()),
        ("status", filter.status.clone()),
        ("sort", filter.sort.clone()),
        ("dir", filter.dir.clone()),
        ("per_page", filter.per_page.clone()),
        ("page", None),
    ];
    let mut s = form_urlencoded::Serializer::new(String::new());
    for (key, value) in current {
        let value = set.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone()).or(value);
        if let Some(v) = value.filter(|v| !v.is_empty()) {
            s.append_pair(key, &v);
        }
    }
    let qs = s.finish();
    if qs.is_empty() {
        "/admin/questions".to_string()
    } else {
        format!("/admin/questions?{qs}")
    }
}

#[derive(serde::Serialize)]
struct StatusTab {
    label: &'static str,
    href: String,
    active: bool,
}

/// The All / Live / Draft filter. These are links rather than submit buttons
/// so each one carries the current search, track, sort and page size.
fn status_tabs(filter: &db::QuestionFilter) -> Vec<StatusTab> {
    let current = filter.status.as_deref().unwrap_or("");
    [("All", ""), ("Live", "live"), ("Draft", "draft")]
        .into_iter()
        .map(|(label, value)| StatusTab {
            label,
            href: list_href(filter, &[("status", value.to_string())]),
            active: current == value,
        })
        .collect()
}

#[derive(serde::Serialize)]
struct SortCol {
    key: &'static str,
    label: &'static str,
}

fn sort_cols() -> Vec<SortCol> {
    vec![
        SortCol { key: "question", label: "Question" },
        SortCol { key: "track", label: "Track" },
        SortCol { key: "domain", label: "Domain" },
        SortCol { key: "status", label: "Status" },
        SortCol { key: "answered", label: "Answered" },
        SortCol { key: "correct", label: "Correct" },
        SortCol { key: "edited", label: "Edited" },
    ]
}

#[derive(serde::Serialize)]
struct SortHeader {
    label: &'static str,
    href: String,
    /// "↓" / "↑" on the active column, empty otherwise.
    arrow: &'static str,
    active: bool,
    numeric: bool,
}

fn sort_headers(filter: &db::QuestionFilter) -> Vec<SortHeader> {
    let (sort, dir) = (filter.sort_key(), filter.dir());
    sort_cols()
        .into_iter()
        .map(|col| {
            let active = col.key == sort;
            // First click on a column sorts the natural way for it; clicking
            // the active column flips it.
            let natural = if matches!(col.key, "edited" | "answered" | "correct") { "desc" } else { "asc" };
            let next_dir = if active { if dir == "asc" { "desc" } else { "asc" } } else { natural };
            SortHeader {
                label: col.label,
                href: list_href(filter, &[("sort", col.key.to_string()), ("dir", next_dir.to_string())]),
                arrow: if !active { "" } else if dir == "asc" { "↑" } else { "↓" },
                active,
                numeric: matches!(col.key, "answered" | "correct" | "edited"),
            }
        })
        .collect()
}

#[derive(serde::Serialize)]
struct PageLink {
    /// Page number, or "…" for a gap (no href).
    label: String,
    href: String,
    current: bool,
}

/// Every page when there are a few, otherwise first, last and the pages
/// around the current one, with "…" for the gaps.
fn page_links(filter: &db::QuestionFilter, page: i64, pages: i64) -> Vec<PageLink> {
    let mut out = Vec::new();
    let mut last_shown = 0;
    for n in 1..=pages {
        if pages > 7 && n != 1 && n != pages && (n - page).abs() > 1 {
            continue;
        }
        if n - last_shown > 1 {
            out.push(PageLink { label: "…".to_string(), href: String::new(), current: false });
        }
        out.push(PageLink {
            label: n.to_string(),
            href: list_href(filter, &[("page", n.to_string())]),
            current: n == page,
        });
        last_shown = n;
    }
    out
}

#[derive(serde::Serialize)]
struct EditableOption {
    letter: char,
    body: String,
    correct: bool,
}

fn pad_options(existing: &[(String, bool)]) -> Vec<EditableOption> {
    let mut out = Vec::with_capacity(6);
    for (i, letter) in crate::model::LETTERS.iter().enumerate() {
        if let Some((body, correct)) = existing.get(i) {
            out.push(EditableOption { letter: *letter, body: body.clone(), correct: *correct });
        } else {
            out.push(EditableOption { letter: *letter, body: String::new(), correct: false });
        }
    }
    out
}

async fn build_questions_list_ctx(state: &AppState, admin: &AdminUser, filter: &db::QuestionFilter) -> Context {
    let list = db::questions_list(&state.db, filter).await;
    let track_options = db::track_options(&state.db).await;
    let grand_total = db::questions_grand_total(&state.db).await;
    let per_page = filter.per_page();
    let first = if list.total == 0 { 0 } else { (list.page - 1) * per_page + 1 };

    let mut ctx = sidebar_context(state, "questions", admin).await;
    ctx.insert("rows", &list.rows);
    ctx.insert("total", &grand_total);
    ctx.insert("track_count", &track_options.len());
    ctx.insert("track_options", &track_options);
    ctx.insert("filter_q", &filter.q.clone().unwrap_or_default());
    ctx.insert("filter_track", &filter.track.clone().unwrap_or_default());
    ctx.insert("filter_status", &filter.status.clone().unwrap_or_default());
    ctx.insert("filter_sort", &filter.sort.clone().unwrap_or_default());
    ctx.insert("filter_dir", &filter.dir.clone().unwrap_or_default());
    ctx.insert("sort_headers", &sort_headers(filter));
    ctx.insert("status_tabs", &status_tabs(filter));

    ctx.insert("matched", &list.total);
    ctx.insert("first", &first);
    ctx.insert("last", &(first + list.rows.len() as i64 - if list.total == 0 { 0 } else { 1 }));
    ctx.insert("per_page", &per_page);
    ctx.insert("per_page_options", &db::PER_PAGE_OPTIONS);
    ctx.insert("page", &list.page);
    ctx.insert("pages", &list.pages);
    ctx.insert("page_links", &page_links(filter, list.page, list.pages));
    let prev = (list.page > 1).then(|| list_href(filter, &[("page", (list.page - 1).to_string())]));
    let next = (list.page < list.pages).then(|| list_href(filter, &[("page", (list.page + 1).to_string())]));
    ctx.insert("prev_href", &prev);
    ctx.insert("next_href", &next);
    // Where a delete sends you back to: this exact page of this exact list.
    ctx.insert("back_href", &list_href(filter, &[("page", list.page.to_string())]));
    ctx.insert("drawer_open", &false);
    ctx
}

#[allow(clippy::too_many_arguments)]
async fn insert_drawer_ctx(
    state: &AppState,
    ctx: &mut Context,
    mode: &str,
    id: Option<i64>,
    cert_slug: &str,
    cert_code: &str,
    domain: &str,
    body: &str,
    explain: &str,
    difficulty: &str,
    status: &str,
    tags: &str,
    options: &[(String, bool)],
    error: Option<&str>,
) {
    let domains = db::domains_for_cert(&state.db, cert_slug, Some(domain)).await;
    ctx.insert("drawer_open", &true);
    ctx.insert("drawer_mode", mode);
    if let Some(id) = id {
        ctx.insert("q_id", &id);
    }
    ctx.insert("cert_slug", cert_slug);
    ctx.insert("cert_code", cert_code);
    ctx.insert("domains", &domains);
    ctx.insert("body", body);
    ctx.insert("explain", explain);
    ctx.insert("difficulty", difficulty);
    ctx.insert("status", status);
    ctx.insert("tags", tags);
    ctx.insert("options", &pad_options(options));
    if let Some(e) = error {
        ctx.insert("drawer_error", e);
    }
}

#[derive(Deserialize)]
struct NoticeQuery {
    deleted: Option<i64>,
    retired: Option<i64>,
}

fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The message after a delete, e.g. "2 questions deleted. 1 question was
/// retired instead …".
fn delete_notice(nq: &NoticeQuery) -> Option<String> {
    let (deleted, retired) = (nq.deleted.unwrap_or(0), nq.retired.unwrap_or(0));
    let mut parts = Vec::new();
    if deleted > 0 {
        parts.push(format!("{} deleted.", plural(deleted, "question", "questions")));
    }
    if retired > 0 {
        parts.push(format!(
            "{} retired instead of deleted, because learners have already answered {}. Retired questions no longer appear in quizzes, and learners' past results are kept.",
            plural(retired, "question was", "questions were"),
            if retired == 1 { "it" } else { "them" }
        ));
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

async fn questions_list_page(
    State(state): State<Arc<AppState>>,
    admin: AdminUser,
    Query(filter): Query<db::QuestionFilter>,
    Query(nq): Query<NoticeQuery>,
) -> Response {
    let mut ctx = build_questions_list_ctx(&state, &admin, &filter).await;
    ctx.insert("notice", &delete_notice(&nq));
    render(&state, "admin/questions.html", &ctx)
}

/// Runs the delete and returns to `back` (only ever a question-list URL) with
/// the counts for the notice.
async fn delete_and_return(state: &AppState, ids: &[i64], back: Option<&str>) -> Response {
    let back = back
        .filter(|b| *b == "/admin/questions" || b.starts_with("/admin/questions?"))
        .unwrap_or("/admin/questions");
    let counts = db::delete_questions(&state.db, ids).await;
    let sep = if back.contains('?') { '&' } else { '?' };
    Redirect::to(&format!("{back}{sep}deleted={}&retired={}", counts.deleted, counts.retired)).into_response()
}

/// The Delete button in the edit sidebar.
async fn question_delete(State(state): State<Arc<AppState>>, _admin: AdminUser, Path(id): Path<i64>, body: Bytes) -> Response {
    let back = form_urlencoded::parse(&body).find(|(k, _)| k == "back").map(|(_, v)| v.into_owned());
    delete_and_return(&state, &[id], back.as_deref()).await
}

/// "Delete selected" on the list: one `id` per ticked row.
async fn questions_delete(State(state): State<Arc<AppState>>, _admin: AdminUser, body: Bytes) -> Response {
    let mut ids = Vec::new();
    let mut back = None;
    for (key, value) in form_urlencoded::parse(&body) {
        match key.as_ref() {
            "id" => ids.extend(value.parse::<i64>().ok()),
            "back" => back = Some(value.into_owned()),
            _ => {}
        }
    }
    delete_and_return(&state, &ids, back.as_deref()).await
}

#[derive(Deserialize)]
struct NewQuery {
    track: Option<String>,
}

async fn question_new_page(
    State(state): State<Arc<AppState>>,
    admin: AdminUser,
    Query(nq): Query<NewQuery>,
) -> Response {
    let track_options = db::track_options(&state.db).await;
    let cert = nq
        .track
        .as_deref()
        .and_then(|t| track_options.iter().find(|o| o.slug == t))
        .or_else(|| track_options.first());
    let (cert_slug, cert_code) = cert.map(|c| (c.slug.clone(), c.code.clone())).unwrap_or_default();

    let filter = db::QuestionFilter::default();
    let mut ctx = build_questions_list_ctx(&state, &admin, &filter).await;
    insert_drawer_ctx(&state, &mut ctx, "new", None, &cert_slug, &cert_code, "", "", "", "Medium", "live", "", &[], None).await;
    render(&state, "admin/questions.html", &ctx)
}

async fn question_edit_page(State(state): State<Arc<AppState>>, admin: AdminUser, Path(id): Path<i64>) -> Response {
    let filter = db::QuestionFilter::default();
    let mut ctx = build_questions_list_ctx(&state, &admin, &filter).await;
    if let Some(detail) = db::question_detail(&state.db, id).await {
        let options: Vec<(String, bool)> = detail.options.iter().map(|o| (o.body.clone(), o.is_correct)).collect();
        insert_drawer_ctx(
            &state,
            &mut ctx,
            "edit",
            Some(detail.id),
            &detail.cert_slug,
            &detail.cert_code,
            &detail.domain,
            &detail.body,
            &detail.explain,
            &detail.difficulty,
            &detail.status,
            &detail.tags,
            &options,
            None,
        )
        .await;
    }
    render(&state, "admin/questions.html", &ctx)
}

struct QuestionFormData {
    cert_slug: String,
    domain: String,
    body: String,
    explain: String,
    difficulty: String,
    status: String,
    tags: Vec<String>,
    options: Vec<(String, bool)>,
    /// For each entry in `options`, the letter slot it came from (A = 0). The
    /// drawer lists a question's options in `option_index` order, so this is
    /// the option's index before the edit — `update_question` uses it to keep
    /// each option's row (and so learners' past picks) attached to it.
    option_slots: Vec<usize>,
}

fn parse_question_form(body: &Bytes) -> QuestionFormData {
    use std::collections::{BTreeMap, HashSet};

    let mut cert_slug = String::new();
    let mut domain = String::new();
    let mut q_body = String::new();
    let mut explain = String::new();
    let mut difficulty = "Medium".to_string();
    let mut status = "live".to_string();
    let mut tags_raw = String::new();
    let mut option_map: BTreeMap<char, String> = BTreeMap::new();
    let mut correct: HashSet<char> = HashSet::new();

    for (key, value) in form_urlencoded::parse(body) {
        match key.as_ref() {
            "cert_slug" => cert_slug = value.to_string(),
            "domain" => domain = value.to_string(),
            "body" => q_body = value.to_string(),
            "explain" => explain = value.to_string(),
            "difficulty" => difficulty = value.to_string(),
            "status" => status = value.to_string(),
            "tags" => tags_raw = value.to_string(),
            "correct" => {
                if let Some(c) = value.chars().next() {
                    correct.insert(c);
                }
            }
            k if k.starts_with("option_") => {
                if let Some(letter) = k.chars().last() {
                    option_map.insert(letter, value.trim().to_string());
                }
            }
            _ => {}
        }
    }

    let filled: Vec<(char, String)> = option_map.into_iter().filter(|(_, body)| !body.is_empty()).collect();
    let option_slots: Vec<usize> = filled
        .iter()
        .map(|(letter, _)| crate::model::LETTERS.iter().position(|l| l == letter).unwrap_or(usize::MAX))
        .collect();
    let options: Vec<(String, bool)> =
        filled.into_iter().map(|(letter, body)| (body, correct.contains(&letter))).collect();

    let tags: Vec<String> = tags_raw.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();

    QuestionFormData { cert_slug, domain, body: q_body, explain, difficulty, status, tags, options, option_slots }
}

async fn question_create(State(state): State<Arc<AppState>>, admin: AdminUser, body: Bytes) -> Response {
    let parsed = parse_question_form(&body);
    let input = db::NewQuestionInput {
        cert_slug: &parsed.cert_slug,
        domain: &parsed.domain,
        body: &parsed.body,
        explain: &parsed.explain,
        difficulty: &parsed.difficulty,
        status: &parsed.status,
        tags: parsed.tags.clone(),
        options: parsed.options.clone(),
    };
    match db::insert_question(&state.db, &input).await {
        Ok(_) => Redirect::to("/admin/questions").into_response(),
        Err(err) => {
            let filter = db::QuestionFilter::default();
            let mut ctx = build_questions_list_ctx(&state, &admin, &filter).await;
            let track_options = db::track_options(&state.db).await;
            let cert_code = track_options.iter().find(|t| t.slug == parsed.cert_slug).map(|t| t.code.clone()).unwrap_or_default();
            insert_drawer_ctx(
                &state,
                &mut ctx,
                "new",
                None,
                &parsed.cert_slug,
                &cert_code,
                &parsed.domain,
                &parsed.body,
                &parsed.explain,
                &parsed.difficulty,
                &parsed.status,
                &parsed.tags.join(", "),
                &parsed.options,
                Some(&err),
            )
            .await;
            render(&state, "admin/questions.html", &ctx)
        }
    }
}

async fn question_update(State(state): State<Arc<AppState>>, admin: AdminUser, Path(id): Path<i64>, body: Bytes) -> Response {
    let parsed = parse_question_form(&body);
    let input = db::NewQuestionInput {
        cert_slug: &parsed.cert_slug,
        domain: &parsed.domain,
        body: &parsed.body,
        explain: &parsed.explain,
        difficulty: &parsed.difficulty,
        status: &parsed.status,
        tags: parsed.tags.clone(),
        options: parsed.options.clone(),
    };
    match db::update_question(&state.db, id, &input, &parsed.option_slots).await {
        Ok(()) => Redirect::to("/admin/questions").into_response(),
        Err(err) => {
            let filter = db::QuestionFilter::default();
            let mut ctx = build_questions_list_ctx(&state, &admin, &filter).await;
            let track_options = db::track_options(&state.db).await;
            let cert_code = track_options.iter().find(|t| t.slug == parsed.cert_slug).map(|t| t.code.clone()).unwrap_or_default();
            insert_drawer_ctx(
                &state,
                &mut ctx,
                "edit",
                Some(id),
                &parsed.cert_slug,
                &cert_code,
                &parsed.domain,
                &parsed.body,
                &parsed.explain,
                &parsed.difficulty,
                &parsed.status,
                &parsed.tags.join(", "),
                &parsed.options,
                Some(&err),
            )
            .await;
            render(&state, "admin/questions.html", &ctx)
        }
    }
}

/* ---------- flagged ---------- */

async fn flagged_page(State(state): State<Arc<AppState>>, admin: AdminUser) -> Response {
    let groups = db::flagged_list(&state.db).await;
    let mut ctx = sidebar_context(&state, "flagged", &admin).await;
    ctx.insert("groups", &groups);
    render(&state, "admin/flagged.html", &ctx)
}

async fn flagged_action(State(state): State<Arc<AppState>>, _admin: AdminUser, body: Bytes) -> Response {
    let mut question_id: Option<i64> = None;
    let mut action = String::new();
    for (key, value) in form_urlencoded::parse(&body) {
        match key.as_ref() {
            "question_id" => question_id = value.parse().ok(),
            "action" => action = value.to_string(),
            _ => {}
        }
    }
    if let Some(id) = question_id {
        match action.as_str() {
            "keep" => db::resolve_flags(&state.db, id, "kept").await,
            "retire" => db::resolve_flags(&state.db, id, "retired").await,
            _ => {}
        }
    }
    Redirect::to("/admin/flagged").into_response()
}

/* ---------- import ---------- */

struct ParsedImportRow {
    question: String,
    options: Vec<String>,
    answer: Vec<usize>,
    domain: String,
    explain: String,
    difficulty: String,
}

#[derive(Deserialize)]
struct ImportRowJson {
    question: String,
    options: Vec<String>,
    answer: Vec<usize>,
    domain: String,
    #[serde(default)]
    explain: String,
    #[serde(default = "default_difficulty")]
    difficulty: String,
}
fn default_difficulty() -> String {
    "Medium".to_string()
}

fn parse_json_rows(payload: &str) -> Result<Vec<ParsedImportRow>, String> {
    let rows: Vec<ImportRowJson> = serde_json::from_str(payload).map_err(|e| format!("invalid JSON: {e}"))?;
    Ok(rows
        .into_iter()
        .map(|r| ParsedImportRow {
            question: r.question,
            options: r.options,
            answer: r.answer,
            domain: r.domain,
            explain: r.explain,
            difficulty: r.difficulty,
        })
        .collect())
}

fn parse_csv_rows(payload: &str) -> Result<Vec<ParsedImportRow>, String> {
    let mut reader = csv::ReaderBuilder::new().has_headers(true).from_reader(payload.as_bytes());
    let mut rows = Vec::new();
    for result in reader.records() {
        let record = result.map_err(|e| format!("invalid CSV: {e}"))?;
        let get = |i: usize| record.get(i).unwrap_or("").trim().to_string();
        let options: Vec<String> = get(1).split('|').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        let answer: Vec<usize> = get(2)
            .split(|c: char| c == '|' || c == ',')
            .filter_map(|s| s.trim().parse::<usize>().ok())
            .collect();
        let difficulty = {
            let d = get(5);
            if d.is_empty() {
                default_difficulty()
            } else {
                d
            }
        };
        rows.push(ParsedImportRow { question: get(0), options, answer, domain: get(3), explain: get(4), difficulty });
    }
    Ok(rows)
}

async fn render_import_page(
    state: &AppState,
    admin: &AdminUser,
    format: &str,
    track: &str,
    payload: &str,
    result: Option<(i32, i32, Option<String>)>,
) -> Response {
    let track_options = db::track_options(&state.db).await;
    let recent = db::recent_imports(&state.db, 6).await;

    let mut ctx = sidebar_context(state, "import", admin).await;
    let track_value = if track.is_empty() {
        track_options.first().map(|t| t.slug.clone()).unwrap_or_default()
    } else {
        track.to_string()
    };
    ctx.insert("track_options", &track_options);
    ctx.insert("format", format);
    ctx.insert("track", &track_value);
    ctx.insert("payload", payload);
    ctx.insert("recent", &recent);
    if let Some((accepted, rejected, reason)) = result {
        ctx.insert(
            "result",
            &serde_json::json!({ "accepted": accepted, "rejected": rejected, "reason": reason.unwrap_or_default() }),
        );
    }
    render(state, "admin/import.html", &ctx)
}

async fn import_page(State(state): State<Arc<AppState>>, admin: AdminUser) -> Response {
    render_import_page(&state, &admin, "json", "", "", None).await
}

async fn import_submit(State(state): State<Arc<AppState>>, admin: AdminUser, body: Bytes) -> Response {
    let mut format = "json".to_string();
    let mut track = String::new();
    let mut payload = String::new();
    let mut as_draft = false;

    for (key, value) in form_urlencoded::parse(&body) {
        match key.as_ref() {
            "format" => format = value.to_string(),
            "track" => track = value.to_string(),
            "payload" => payload = value.to_string(),
            "as_draft" => as_draft = true,
            _ => {}
        }
    }

    let status = if as_draft { "draft" } else { "live" };

    let parsed_rows = if format == "csv" { parse_csv_rows(&payload) } else { parse_json_rows(&payload) };

    let (accepted, rejected, reason) = match parsed_rows {
        Err(top_error) => (0, 0, Some(top_error)),
        Ok(rows) => {
            let mut accepted = 0i32;
            let mut rejected = 0i32;
            let mut first_reason: Option<String> = None;
            for row in &rows {
                let options: Vec<(String, bool)> =
                    row.options.iter().enumerate().map(|(i, o)| (o.clone(), row.answer.contains(&i))).collect();
                let input = db::NewQuestionInput {
                    cert_slug: &track,
                    domain: &row.domain,
                    body: &row.question,
                    explain: &row.explain,
                    difficulty: &row.difficulty,
                    status,
                    tags: Vec::new(),
                    options,
                };
                match db::insert_question(&state.db, &input).await {
                    Ok(_) => accepted += 1,
                    Err(e) => {
                        rejected += 1;
                        if first_reason.is_none() {
                            first_reason = Some(e);
                        }
                    }
                }
            }
            (accepted, rejected, first_reason)
        }
    };

    if accepted > 0 || rejected > 0 {
        db::log_import(&state.db, &track, &format, accepted, rejected, reason.as_deref(), admin.id).await;
    }

    render_import_page(&state, &admin, &format, &track, "", Some((accepted, rejected, reason))).await
}

/* ---------- users ---------- */

#[derive(Deserialize)]
struct UsersQuery {
    q: Option<String>,
}

async fn users_page(State(state): State<Arc<AppState>>, admin: AdminUser, Query(uq): Query<UsersQuery>) -> Response {
    let mut rows = db::admin_users(&state.db).await;
    if let Some(q) = &uq.q {
        let needle = q.to_lowercase();
        if !needle.is_empty() {
            rows.retain(|r| r.name.to_lowercase().contains(&needle) || r.email.to_lowercase().contains(&needle));
        }
    }
    let mut ctx = sidebar_context(&state, "users", &admin).await;
    ctx.insert("rows", &rows);
    ctx.insert("filter_q", &uq.q.unwrap_or_default());
    render(&state, "admin/users.html", &ctx)
}

async fn users_toggle(State(state): State<Arc<AppState>>, _admin: AdminUser, body: Bytes) -> Response {
    for (key, value) in form_urlencoded::parse(&body) {
        if key == "toggle" {
            db::toggle_user_status(&state.db, &value).await;
        }
    }
    Redirect::to("/admin/users").into_response()
}
