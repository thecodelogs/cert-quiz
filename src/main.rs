mod admin;
mod auth;
mod db;
mod model;

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Router,
};
use model::{fmt_clock, is_correct, score, Answers, Cert, Question, LETTERS};
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use std::{collections::BTreeMap, sync::Arc};
use tera::{Context, Tera};
use tower_http::services::ServeDir;

struct AppState {
    tera: Tera,
    db: sqlx::PgPool,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("failed to connect to database");

    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("failed to run migrations");

    let tera = Tera::new("templates/**/*.html").expect("template parse error");
    // An unmatched glob yields an empty Tera rather than an error, which only
    // surfaces later as TemplateNotFound — fail at startup instead.
    if tera.get_template_names().next().is_none() {
        panic!(
            "no templates found under {}/templates",
            std::env::current_dir().unwrap_or_default().display()
        );
    }
    let state = Arc::new(AppState { tera, db });

    let app = Router::new()
        .route("/", get(home))
        .route("/quiz/:cert_id", get(quiz_start).post(quiz_step))
        .nest("/admin", admin::router())
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Drill running on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

/* ---------- rendering ---------- */

fn render(state: &AppState, template: &str, ctx: &Context) -> Response {
    match state.tera.render(template, ctx) {
        Ok(html) => Html(html).into_response(),
        Err(err) => {
            eprintln!("render error: {err:?}");
            (StatusCode::INTERNAL_SERVER_ERROR, "template error").into_response()
        }
    }
}

async fn base_context(state: &AppState) -> Context {
    let (tracks, total_questions) = db::totals(&state.db).await;
    let mut ctx = Context::new();
    ctx.insert("total_questions", &total_questions);
    ctx.insert("tracks", &tracks);
    ctx
}

/* ---------- home ---------- */

#[derive(Deserialize)]
struct HomeQuery {
    provider: Option<String>,
}

#[derive(serde::Serialize)]
struct Tab {
    key: String,
    label: String,
    active: bool,
}

#[derive(serde::Serialize)]
struct CertRow {
    id: String,
    code: String,
    name: String,
    blurb: String,
    provider_label: String,
    difficulty: String,
    count: usize,
    best_label: String,
}

async fn home(State(state): State<Arc<AppState>>, Query(q): Query<HomeQuery>) -> Response {
    let filter = q.provider.unwrap_or_else(|| "All".to_string());

    let tabs: Vec<Tab> = [("All", "All"), ("AWS", "AWS"), ("GCP", "Google Cloud"), ("K8s", "Kubernetes")]
        .iter()
        .map(|(key, label)| Tab {
            key: key.to_string(),
            label: label.to_string(),
            active: filter == *key,
        })
        .collect();

    let summaries = db::cert_summaries(&state.db).await;

    let rows: Vec<CertRow> = summaries
        .iter()
        .filter(|c| filter == "All" || c.provider == filter)
        .map(|c| CertRow {
            id: c.id.clone(),
            code: c.code.clone(),
            name: c.name.clone(),
            blurb: c.blurb.clone(),
            provider_label: c.provider_label.clone(),
            difficulty: c.difficulty.clone(),
            count: c.question_count as usize,
            // static for now — read the signed-in user's attempts here
            best_label: "no attempt yet".to_string(),
        })
        .collect();

    let mut ctx = base_context(&state).await;
    ctx.insert("count_label", &format!("{} {}", rows.len(), if rows.len() == 1 { "track" } else { "tracks" }));
    ctx.insert("certs", &rows);
    ctx.insert("tabs", &tabs);
    ctx.insert("first_cert", &summaries.first().map(|c| c.id.clone()).unwrap_or_default());
    ctx.insert("history", &Vec::<String>::new());
    render(&state, "home.html", &ctx)
}

/* ---------- quiz ---------- */

#[derive(serde::Serialize)]
struct OptionView {
    value: usize,
    letter: String,
    text: String,
    checked: bool,
    rule_class: &'static str,
    bg_class: &'static str,
    letter_class: &'static str,
}

async fn question_context(
    state: &AppState,
    cert: &Cert,
    index: usize,
    picked: &[usize],
    answers: &Answers,
    elapsed: u64,
    instant: bool,
    revealed: bool,
) -> Context {
    let qs = cert.set();
    let q: &Question = &qs[index];
    let multi = q.answer.len() > 1;

    let options: Vec<OptionView> = q
        .options
        .iter()
        .enumerate()
        .map(|(oi, text)| {
            let is_picked = picked.contains(&oi);
            let is_answer = q.answer.contains(&oi);
            let (rule_class, bg_class, letter_class) = if revealed && is_answer {
                ("border-l-ink", "bg-right", "text-ink")
            } else if revealed && is_picked {
                ("border-l-rust", "bg-wrong", "text-rustd")
            } else {
                ("border-l-transparent", "", "text-[#8a8073]")
            };
            OptionView {
                value: oi,
                letter: LETTERS[oi].to_string(),
                text: text.clone(),
                checked: is_picked,
                rule_class,
                bg_class,
                letter_class,
            }
        })
        .collect();

    let mut ctx = base_context(state).await;
    ctx.insert("cert_id", &cert.id);
    ctx.insert("cert_code", &cert.code);
    ctx.insert("index", &index);
    ctx.insert("q_number", &(index + 1));
    ctx.insert("total", &qs.len());
    ctx.insert("progress", &(((index + 1) as f32 / qs.len() as f32 * 100.0).round() as u32));
    ctx.insert("question", &q.q);
    ctx.insert("domain", &q.domain);
    ctx.insert("explain", &q.explain);
    ctx.insert("select_hint", &if multi { format!("Select {}", q.answer.len()) } else { "Select one".to_string() });
    ctx.insert("input_type", &if multi { "checkbox" } else { "radio" });
    ctx.insert("options", &options);
    ctx.insert("elapsed", &elapsed);
    ctx.insert("instant", &instant);
    ctx.insert("instant_next", &if instant { "0" } else { "1" });
    ctx.insert("revealed", &revealed);
    ctx.insert("correct", &is_correct(q, picked));
    ctx.insert("is_last", &(index + 1 == qs.len()));
    ctx.insert("answers_json", &serde_json::to_string(answers).unwrap_or_else(|_| "{}".into()));
    ctx
}

async fn quiz_start(State(state): State<Arc<AppState>>, Path(cert_id): Path<String>) -> Response {
    let Some(cert) = db::load_cert(&state.db, &cert_id).await else {
        return Redirect::to("/").into_response();
    };
    let answers: Answers = BTreeMap::new();
    let ctx = question_context(&state, &cert, 0, &[], &answers, 0, false, false).await;
    render(&state, "question.html", &ctx)
}

/// One POST per question. `answers` travels as JSON in a hidden field, so the
/// server stays stateless — move it into a session or an `attempts` table later.
async fn quiz_step(
    State(state): State<Arc<AppState>>,
    Path(cert_id): Path<String>,
    body: Bytes,
) -> Response {
    let Some(cert) = db::load_cert(&state.db, &cert_id).await else {
        return Redirect::to("/").into_response();
    };
    let qs = cert.set();

    let mut index = 0usize;
    let mut action = String::from("next");
    let mut elapsed = 0u64;
    let mut instant = false;
    let mut was_revealed = false;
    let mut answers: Answers = BTreeMap::new();
    let mut picked: Vec<usize> = Vec::new();

    for (key, value) in form_urlencoded::parse(&body) {
        match key.as_ref() {
            "index" => index = value.parse().unwrap_or(0),
            "action" => action = value.to_string(),
            "elapsed" => elapsed = value.parse().unwrap_or(0),
            "instant" => instant = value == "1",
            "revealed" => was_revealed = value == "1",
            "answers" => answers = serde_json::from_str(&value).unwrap_or_default(),
            "choice" => {
                if let Ok(v) = value.parse::<usize>() {
                    picked.push(v);
                }
            }
            _ => {}
        }
    }

    index = index.min(qs.len().saturating_sub(1));
    if action == "skip" {
        picked.clear();
    }
    if picked.is_empty() {
        answers.remove(&index);
    } else {
        answers.insert(index, picked.clone());
    }

    // instant feedback: the first submit reveals the answer, the second advances
    if instant && action == "next" && !picked.is_empty() && !was_revealed {
        let ctx = question_context(&state, &cert, index, &picked, &answers, elapsed, instant, true).await;
        return render(&state, "question.html", &ctx);
    }

    let next_index = match action.as_str() {
        "prev" => index.saturating_sub(1),
        "next" | "skip" => index + 1,
        _ => index,
    };

    if action == "finish" || next_index >= qs.len() {
        return results(&state, &cert, &answers, elapsed).await;
    }

    let next_picked = answers.get(&next_index).cloned().unwrap_or_default();
    let ctx = question_context(&state, &cert, next_index, &next_picked, &answers, elapsed, instant, false).await;
    render(&state, "question.html", &ctx)
}

/* ---------- results ---------- */

#[derive(serde::Serialize)]
struct DomainView {
    name: String,
    label: String,
    pct: u32,
    bar_class: &'static str,
}

#[derive(serde::Serialize)]
struct ReviewView {
    num: String,
    question: String,
    status: String,
    ok: bool,
    yours: String,
    correct: String,
}

async fn results(state: &AppState, cert: &Cert, answers: &Answers, elapsed: u64) -> Response {
    let pass = cert.pass_mark;
    let s = score(cert, answers);

    let domains: Vec<DomainView> = s
        .domains
        .iter()
        .map(|d| DomainView {
            name: d.name.clone(),
            label: format!("{} / {}", d.ok, d.n),
            pct: d.pct,
            bar_class: if d.pct >= pass { "bg-ink" } else { "bg-rust" },
        })
        .collect();

    let review: Vec<ReviewView> = s
        .review
        .iter()
        .map(|r| ReviewView {
            num: r.num.clone(),
            question: r.question.clone(),
            status: r.status.to_string(),
            ok: r.ok,
            yours: r.yours.clone(),
            correct: r.correct.clone(),
        })
        .collect();

    let verdict = if s.pct >= 85 {
        "Comfortably above a typical pass mark. Move to the next track, or retake this one for speed."
    } else if s.pct >= pass {
        "Around the pass mark. Repeat the weak domains below before you book the exam."
    } else {
        "Below a typical pass mark. Work the weak domains, then retake the set."
    };

    let mut ctx = base_context(state).await;
    ctx.insert("cert_id", &cert.id);
    ctx.insert("cert_code", &cert.code);
    ctx.insert("pct", &s.pct);
    ctx.insert("correct_count", &s.correct);
    ctx.insert("wrong_count", &s.wrong);
    ctx.insert("skipped_count", &s.skipped);
    ctx.insert("elapsed_label", &fmt_clock(elapsed));
    ctx.insert("verdict", &verdict);
    ctx.insert("domains", &domains);
    ctx.insert("review", &review);
    render(state, "results.html", &ctx)
}
