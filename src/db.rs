use crate::model::{Cert, Question};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// "today" / "yesterday" / "N days ago" / "N weeks ago" from a day count.
fn relative_days(days: i64) -> String {
    match days {
        d if d <= 0 => "today".to_string(),
        1 => "yesterday".to_string(),
        d if d < 14 => format!("{d} days ago"),
        d => format!("{} weeks ago", d / 7),
    }
}

/// One certification's row on the learner home page — metadata plus a
/// *live* question count, without pulling in the full question/option
/// content nobody on that page needs. Only published tracks are included.
#[derive(serde::Serialize)]
pub struct CertSummary {
    pub id: String,
    pub provider: String,
    pub provider_label: String,
    pub code: String,
    pub name: String,
    pub difficulty: String,
    pub blurb: String,
    pub question_count: i64,
}

pub async fn cert_summaries(pool: &PgPool) -> Vec<CertSummary> {
    let rows = sqlx::query(
        "select c.id, c.slug, p.name as provider, p.label as provider_label,
                c.code, c.name, c.difficulty, c.blurb,
                count(q.id) filter (where q.status = 'live') as question_count
         from certifications c
         join providers p on p.id = c.provider_id
         left join questions q on q.certification_id = c.id
         where c.published
         group by c.id, p.name, p.label
         order by c.id",
    )
    .fetch_all(pool)
    .await
    .expect("failed to load certification summaries");

    rows.into_iter()
        .map(|row| CertSummary {
            id: row.try_get("slug").expect("certifications.slug"),
            provider: row.try_get("provider").expect("providers.name"),
            provider_label: row.try_get("provider_label").expect("providers.label"),
            code: row.try_get("code").expect("certifications.code"),
            name: row.try_get("name").expect("certifications.name"),
            difficulty: row.try_get("difficulty").expect("certifications.difficulty"),
            blurb: row.try_get("blurb").expect("certifications.blurb"),
            question_count: row.try_get("question_count").expect("question_count"),
        })
        .collect()
}

/// (number of published certifications, number of live questions) — the
/// header stat on every learner-facing page.
pub async fn totals(pool: &PgPool) -> (i64, i64) {
    let row = sqlx::query(
        "select (select count(*) from certifications where published) as tracks,
                (select count(*) from questions where status = 'live') as total_questions",
    )
    .fetch_one(pool)
    .await
    .expect("failed to load totals");

    (
        row.try_get("tracks").expect("tracks"),
        row.try_get("total_questions").expect("total_questions"),
    )
}

/// One certification with all of its *live* questions and options, freshly
/// queried — this is what a quiz attempt runs against, so it has to reflect
/// whatever is in the database right now, not a cached snapshot, and it
/// must never hand a learner a draft or retired question.
pub async fn load_cert(pool: &PgPool, slug: &str) -> Option<Cert> {
    let cert_row = sqlx::query("select id, slug, code, pass_mark from certifications where slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await
        .expect("failed to load certification");

    let cert_row = cert_row?;
    let cert_id: Uuid = cert_row.try_get("id").expect("certifications.id");

    let question_rows = sqlx::query(
        "select q.id, q.body, q.explain, d.name as domain_name
         from questions q
         join domains d on d.id = q.domain_id
         where q.certification_id = $1 and q.status = 'live'
         order by q.sort_order",
    )
    .bind(cert_id)
    .fetch_all(pool)
    .await
    .expect("failed to load questions");

    let mut questions = Vec::with_capacity(question_rows.len());
    for q_row in question_rows {
        let question_id: Uuid = q_row.try_get("id").expect("questions.id");

        let option_rows = sqlx::query(
            "select body, is_correct from question_options where question_id = $1 order by option_index",
        )
        .bind(question_id)
        .fetch_all(pool)
        .await
        .expect("failed to load question options");

        let mut options = Vec::with_capacity(option_rows.len());
        let mut answer = Vec::new();
        for (idx, opt_row) in option_rows.iter().enumerate() {
            options.push(opt_row.try_get::<String, _>("body").expect("question_options.body"));
            if opt_row.try_get::<bool, _>("is_correct").expect("question_options.is_correct") {
                answer.push(idx);
            }
        }

        questions.push(Question {
            id: question_id,
            q: q_row.try_get("body").expect("questions.body"),
            options,
            answer,
            domain: q_row.try_get("domain_name").expect("domains.name"),
            explain: q_row.try_get("explain").expect("questions.explain"),
        });
    }

    Some(Cert {
        id: cert_row.try_get("slug").expect("certifications.slug"),
        code: cert_row.try_get("code").expect("certifications.code"),
        pass_mark: cert_row.try_get::<i32, _>("pass_mark").expect("certifications.pass_mark") as u32,
        questions,
    })
}

/* ================= admin: dashboard ================= */

#[derive(serde::Serialize)]
pub struct DashboardStats {
    pub live_questions: i64,
    pub drafts: i64,
    pub flagged: i64,
    pub published_tracks: i64,
    pub total_tracks: i64,
    pub attempts_30d: i64,
    pub mean_score_30d: i64,
    pub pass_mark: i64,
}

pub async fn dashboard_stats(pool: &PgPool) -> DashboardStats {
    let row = sqlx::query(
        "select
            (select count(*) from questions where status = 'live') as live_questions,
            (select count(*) from questions where status = 'draft') as drafts,
            (select count(distinct question_id) from question_flags where status = 'open') as flagged,
            (select count(*) from certifications where published) as published_tracks,
            (select count(*) from certifications) as total_tracks,
            (select count(*) from attempts where finished_at >= now() - interval '30 days') as attempts_30d,
            (select coalesce(round(avg(pct))::bigint, 0) from attempts where finished_at >= now() - interval '30 days') as mean_score_30d,
            (select coalesce(round(avg(pass_mark))::bigint, 72) from certifications) as pass_mark",
    )
    .fetch_one(pool)
    .await
    .expect("failed to load dashboard stats");

    DashboardStats {
        live_questions: row.try_get("live_questions").expect("live_questions"),
        drafts: row.try_get("drafts").expect("drafts"),
        flagged: row.try_get("flagged").expect("flagged"),
        published_tracks: row.try_get("published_tracks").expect("published_tracks"),
        total_tracks: row.try_get("total_tracks").expect("total_tracks"),
        attempts_30d: row.try_get("attempts_30d").expect("attempts_30d"),
        mean_score_30d: row.try_get("mean_score_30d").expect("mean_score_30d"),
        pass_mark: row.try_get("pass_mark").expect("pass_mark"),
    }
}

#[derive(serde::Serialize)]
pub struct AttentionItem {
    pub kind: &'static str,
    pub cert_code: String,
    pub question: String,
    pub detail: String,
    pub when: String,
    pub action_label: &'static str,
}

/// Flagged, draft, and "answered a lot, scored badly" questions, newest
/// first, capped for the dashboard widget.
pub async fn needs_attention(pool: &PgPool, limit: i64) -> Vec<AttentionItem> {
    let mut items = Vec::new();

    let flagged_rows = sqlx::query(
        "select c.code, q.body,
                count(f.id) as report_count,
                max(f.comment) filter (where f.id = (select id from question_flags where question_id = q.id and status='open' order by created_at desc limit 1)) as latest_comment,
                extract(day from now() - max(f.created_at))::bigint as days_ago
         from question_flags f
         join questions q on q.id = f.question_id
         join certifications c on c.id = q.certification_id
         where f.status = 'open'
         group by c.code, q.id, q.body
         order by max(f.created_at) desc
         limit $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .expect("failed to load flagged attention items");

    for row in flagged_rows {
        let report_count: i64 = row.try_get("report_count").expect("report_count");
        let comment: Option<String> = row.try_get("latest_comment").ok();
        let days_ago: i64 = row.try_get("days_ago").expect("days_ago");
        items.push(AttentionItem {
            kind: "Flagged",
            cert_code: row.try_get("code").expect("code"),
            question: row.try_get("body").expect("body"),
            detail: format!(
                "{} report{}{}",
                report_count,
                if report_count == 1 { "" } else { "s" },
                comment.map(|c| format!(": {c}")).unwrap_or_default()
            ),
            when: relative_days(days_ago),
            action_label: "Review",
        });
    }

    let draft_rows = sqlx::query(
        "select c.code, q.body, extract(day from now() - q.updated_at)::bigint as days_ago
         from questions q
         join certifications c on c.id = q.certification_id
         where q.status = 'draft'
         order by q.updated_at desc
         limit $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .expect("failed to load draft attention items");

    for row in draft_rows {
        let days_ago: i64 = row.try_get("days_ago").expect("days_ago");
        items.push(AttentionItem {
            kind: "Draft",
            cert_code: row.try_get("code").expect("code"),
            question: row.try_get("body").expect("body"),
            detail: "Draft · not visible to learners".to_string(),
            when: relative_days(days_ago),
            action_label: "Finish",
        });
    }

    let low_score_rows = sqlx::query(
        "select c.code, q.body,
                count(aa.id) as answered,
                round(100.0 * count(aa.id) filter (where aa.is_correct) / nullif(count(aa.id), 0))::bigint as pct,
                extract(day from now() - max(aa.answered_at))::bigint as days_ago
         from attempt_answers aa
         join questions q on q.id = aa.question_id
         join certifications c on c.id = q.certification_id
         where q.status = 'live'
         group by c.code, q.id, q.body
         having count(aa.id) >= 3 and round(100.0 * count(aa.id) filter (where aa.is_correct) / nullif(count(aa.id), 0)) < 50
         order by pct asc
         limit $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .expect("failed to load low-score attention items");

    for row in low_score_rows {
        let answered: i64 = row.try_get("answered").expect("answered");
        let pct: i64 = row.try_get("pct").expect("pct");
        let days_ago: i64 = row.try_get("days_ago").expect("days_ago");
        items.push(AttentionItem {
            kind: "Low score",
            cert_code: row.try_get("code").expect("code"),
            question: row.try_get("body").expect("body"),
            detail: format!("Answered {answered} times · {pct}% correct, below average"),
            when: relative_days(days_ago),
            action_label: "Inspect",
        });
    }

    items.truncate(limit as usize);
    items
}

#[derive(serde::Serialize)]
pub struct WeakDomain {
    pub name: String,
    pub pct: i64,
}

/// Domains with the lowest learner accuracy over the last 30 days.
pub async fn weakest_domains(pool: &PgPool, limit: i64) -> Vec<WeakDomain> {
    let rows = sqlx::query(
        "select d.name,
                round(100.0 * count(aa.id) filter (where aa.is_correct) / nullif(count(aa.id), 0))::bigint as pct
         from attempt_answers aa
         join questions q on q.id = aa.question_id
         join domains d on d.id = q.domain_id
         join attempts a on a.id = aa.attempt_id
         where a.finished_at >= now() - interval '30 days'
         group by d.name
         having count(aa.id) > 0
         order by pct asc
         limit $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .expect("failed to load weakest domains");

    rows.into_iter()
        .map(|row| WeakDomain {
            name: row.try_get("name").expect("name"),
            pct: row.try_get("pct").expect("pct"),
        })
        .collect()
}

#[derive(serde::Serialize)]
pub struct LastImport {
    pub summary: String,
    pub when: String,
    pub outcome: String,
}

pub async fn last_import(pool: &PgPool) -> Option<LastImport> {
    let row = sqlx::query(
        "select c.slug, i.rows_accepted, i.rows_rejected, i.rejected_reason,
                extract(day from now() - i.created_at)::bigint as days_ago
         from imports i
         join certifications c on c.id = i.certification_id
         order by i.created_at desc
         limit 1",
    )
    .fetch_optional(pool)
    .await
    .expect("failed to load last import");

    row.map(|row| {
        let slug: String = row.try_get("slug").expect("slug");
        let accepted: i32 = row.try_get("rows_accepted").expect("rows_accepted");
        let rejected: i32 = row.try_get("rows_rejected").expect("rows_rejected");
        let reason: Option<String> = row.try_get("rejected_reason").ok();
        let days_ago: i64 = row.try_get("days_ago").expect("days_ago");
        LastImport {
            summary: format!("{accepted} questions added to {}", slug.to_uppercase()),
            when: relative_days(days_ago),
            outcome: if rejected > 0 {
                format!("{rejected} rows rejected{}", reason.map(|r| format!(": {r}")).unwrap_or_default())
            } else {
                "All rows accepted".to_string()
            },
        }
    })
}

#[derive(serde::Serialize)]
pub struct AdminTrackRow {
    pub slug: String,
    pub code: String,
    pub name: String,
    pub provider_label: String,
    pub live: i64,
    pub drafts: i64,
    pub flagged: i64,
    pub mean_score: i64,
    pub published: bool,
    pub edited: String,
}

/// Every certification with live/draft/flagged counts and mean score,
/// regardless of publish state — the admin dashboard's tracks table.
pub async fn admin_tracks(pool: &PgPool) -> Vec<AdminTrackRow> {
    let rows = sqlx::query(
        "select c.slug, c.code, c.name, p.label as provider_label, c.published,
                count(q.id) filter (where q.status = 'live') as live,
                count(q.id) filter (where q.status = 'draft') as drafts,
                count(distinct f.question_id) filter (where f.status = 'open') as flagged,
                coalesce((select round(avg(a.pct))::bigint from attempts a where a.certification_id = c.id), 0) as mean_score,
                extract(day from now() - max(q.updated_at))::bigint as days_ago
         from certifications c
         join providers p on p.id = c.provider_id
         left join questions q on q.certification_id = c.id
         left join question_flags f on f.question_id = q.id
         group by c.id, p.label
         order by c.id",
    )
    .fetch_all(pool)
    .await
    .expect("failed to load admin tracks");

    rows.into_iter()
        .map(|row| {
            let days_ago: Option<i64> = row.try_get("days_ago").ok();
            AdminTrackRow {
                slug: row.try_get("slug").expect("slug"),
                code: row.try_get("code").expect("code"),
                name: row.try_get("name").expect("name"),
                provider_label: row.try_get("provider_label").expect("provider_label"),
                live: row.try_get("live").expect("live"),
                drafts: row.try_get("drafts").expect("drafts"),
                flagged: row.try_get("flagged").expect("flagged"),
                mean_score: row.try_get("mean_score").expect("mean_score"),
                published: row.try_get("published").expect("published"),
                edited: days_ago.map(relative_days).unwrap_or_else(|| "—".to_string()),
            }
        })
        .collect()
}

pub async fn toggle_track_published(pool: &PgPool, slug: &str) {
    sqlx::query("update certifications set published = not published where slug = $1")
        .bind(slug)
        .execute(pool)
        .await
        .expect("failed to toggle track published state");
}

/* ================= admin: questions ================= */

#[derive(Default, serde::Deserialize)]
/// Query-string filters for the admin question list. An empty value (the
/// "All tracks" option, the All status) means "don't filter", same as absent.
pub struct QuestionFilter {
    pub q: Option<String>,
    pub track: Option<String>,
    pub status: Option<String>,
    pub sort: Option<String>,
    pub dir: Option<String>,
    pub page: Option<String>,
    pub per_page: Option<String>,
}

pub const PER_PAGE_OPTIONS: [i64; 3] = [10, 15, 20];

impl QuestionFilter {
    /// Newest activity first unless a column header picked something else, so a
    /// question that was just created or edited is at the top of page 1.
    pub fn sort_key(&self) -> &str {
        self.sort.as_deref().filter(|s| !s.is_empty()).unwrap_or("edited")
    }

    pub fn dir(&self) -> &str {
        match self.dir.as_deref() {
            Some("asc") => "asc",
            Some("desc") => "desc",
            _ if self.sort_key() == "edited" => "desc",
            _ => "asc",
        }
    }

    /// Only the offered sizes are accepted; anything else falls back to the first.
    pub fn per_page(&self) -> i64 {
        self.per_page
            .as_deref()
            .and_then(|p| p.parse().ok())
            .filter(|p| PER_PAGE_OPTIONS.contains(p))
            .unwrap_or(PER_PAGE_OPTIONS[0])
    }

    /// 1-based page as requested; `questions_list` clamps it to the last page.
    pub fn page(&self) -> i64 {
        self.page.as_deref().and_then(|p| p.parse().ok()).filter(|&p| p >= 1).unwrap_or(1)
    }
}

/// One page of the filtered list, plus what the pager needs.
pub struct QuestionPage {
    pub rows: Vec<QuestionRow>,
    /// Questions matching the filters, across all pages.
    pub total: i64,
    /// The page actually shown (after clamping), 1-based.
    pub page: i64,
    pub pages: i64,
}

#[derive(serde::Serialize)]
pub struct QuestionRow {
    pub id: Uuid,
    pub body: String,
    pub cert_code: String,
    pub domain: String,
    pub kind: &'static str,
    pub status: String,
    pub answered: i64,
    pub correct_pct: Option<i64>,
    pub edited: String,
}

/// Shared by the count and the page query so they always agree.
const QUESTION_FILTER_SQL: &str = "(nullif($1::text, '') is null or q.body ilike '%' || $1 || '%' or c.code ilike '%' || $1 || '%')
           and (nullif($2::text, '') is null or c.slug = $2)
           and (nullif($3::text, '') is null or q.status = $3)";

pub async fn questions_list(pool: &PgPool, filter: &QuestionFilter) -> QuestionPage {
    let sort_col = match filter.sort_key() {
        "question" => "q.body",
        "track" => "c.code",
        "domain" => "d.name",
        "status" => "q.status",
        "answered" => "answered",
        "correct" => "correct_pct",
        _ => "q.updated_at",
    };
    let dir = filter.dir();

    let count_sql = format!(
        "select count(*) as n
         from questions q
         join certifications c on c.id = q.certification_id
         where {QUESTION_FILTER_SQL}"
    );
    let total: i64 = sqlx::query(sqlx::AssertSqlSafe(count_sql))
        .bind(&filter.q)
        .bind(&filter.track)
        .bind(&filter.status)
        .fetch_one(pool)
        .await
        .expect("failed to count questions")
        .try_get("n")
        .expect("n");

    let per_page = filter.per_page();
    let pages = ((total + per_page - 1) / per_page).max(1);
    let page = filter.page().min(pages);

    let sql = format!(
        "select q.id, q.body, c.code as cert_code, d.name as domain, q.status,
                extract(day from now() - q.updated_at)::bigint as days_ago,
                count(distinct qo.id) filter (where qo.is_correct) as correct_options,
                count(distinct aa.id) as answered,
                round(100.0 * count(aa.id) filter (where aa.is_correct) / nullif(count(aa.id), 0))::bigint as correct_pct
         from questions q
         join certifications c on c.id = q.certification_id
         join domains d on d.id = q.domain_id
         left join question_options qo on qo.question_id = q.id
         left join attempt_answers aa on aa.question_id = q.id
         where {QUESTION_FILTER_SQL}
         group by q.id, c.code, d.name
         order by {sort_col} {dir} nulls last, q.id desc
         limit $4 offset $5"
    );

    // sort_col/dir are chosen from fixed match arms above, never from raw
    // user input, so string-interpolating them into the query is safe.
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(&filter.q)
        .bind(&filter.track)
        .bind(&filter.status)
        .bind(per_page)
        .bind((page - 1) * per_page)
        .fetch_all(pool)
        .await
        .expect("failed to load questions list");

    let rows = rows
        .into_iter()
        .map(|row| {
            let correct_options: i64 = row.try_get("correct_options").expect("correct_options");
            let days_ago: i64 = row.try_get("days_ago").expect("days_ago");
            QuestionRow {
                id: row.try_get("id").expect("id"),
                body: row.try_get("body").expect("body"),
                cert_code: row.try_get("cert_code").expect("cert_code"),
                domain: row.try_get("domain").expect("domain"),
                kind: if correct_options > 1 { "Multi" } else { "Single" },
                status: row.try_get("status").expect("status"),
                answered: row.try_get("answered").expect("answered"),
                correct_pct: row.try_get("correct_pct").ok(),
                edited: relative_days(days_ago),
            }
        })
        .collect();

    QuestionPage { rows, total, page, pages }
}

#[derive(serde::Serialize)]
pub struct OptionDetail {
    pub letter: char,
    pub body: String,
    pub is_correct: bool,
}

#[derive(serde::Serialize)]
pub struct QuestionDetail {
    pub id: Uuid,
    pub cert_slug: String,
    pub cert_code: String,
    pub body: String,
    pub explain: String,
    pub domain: String,
    pub difficulty: String,
    pub status: String,
    pub tags: String,
    pub options: Vec<OptionDetail>,
}

pub async fn question_detail(pool: &PgPool, id: Uuid) -> Option<QuestionDetail> {
    let row = sqlx::query(
        "select q.id, c.slug as cert_slug, c.code as cert_code, q.body, q.explain,
                d.name as domain, q.difficulty, q.status, q.tags
         from questions q
         join certifications c on c.id = q.certification_id
         join domains d on d.id = q.domain_id
         where q.id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .expect("failed to load question");
    let row = row?;

    let option_rows = sqlx::query("select body, is_correct from question_options where question_id = $1 order by option_index")
        .bind(id)
        .fetch_all(pool)
        .await
        .expect("failed to load question options");

    let tags: Vec<String> = row.try_get("tags").unwrap_or_default();

    Some(QuestionDetail {
        id: row.try_get("id").expect("id"),
        cert_slug: row.try_get("cert_slug").expect("cert_slug"),
        cert_code: row.try_get("cert_code").expect("cert_code"),
        body: row.try_get("body").expect("body"),
        explain: row.try_get("explain").expect("explain"),
        domain: row.try_get("domain").expect("domain"),
        difficulty: row.try_get("difficulty").expect("difficulty"),
        status: row.try_get("status").expect("status"),
        tags: tags.join(", "),
        options: option_rows
            .into_iter()
            .enumerate()
            .map(|(idx, r)| OptionDetail {
                letter: crate::model::LETTERS[idx],
                body: r.try_get("body").expect("body"),
                is_correct: r.try_get("is_correct").expect("is_correct"),
            })
            .collect(),
    })
}

#[derive(serde::Serialize)]
pub struct DomainOption {
    pub name: String,
    pub selected: bool,
}

pub async fn domains_for_cert(pool: &PgPool, cert_slug: &str, selected: Option<&str>) -> Vec<DomainOption> {
    let rows = sqlx::query(
        "select d.name from domains d
         join certifications c on c.id = d.certification_id
         where c.slug = $1
         order by d.sort_order",
    )
    .bind(cert_slug)
    .fetch_all(pool)
    .await
    .expect("failed to load domains for cert");

    rows.into_iter()
        .map(|row| {
            let name: String = row.try_get("name").expect("name");
            let is_selected = selected == Some(name.as_str());
            DomainOption { name, selected: is_selected }
        })
        .collect()
}

pub struct NewQuestionInput<'a> {
    pub cert_slug: &'a str,
    pub domain: &'a str,
    pub body: &'a str,
    pub explain: &'a str,
    pub difficulty: &'a str,
    pub status: &'a str,
    pub tags: Vec<String>,
    pub options: Vec<(String, bool)>,
}

/// Inserts a question and its options, returning the new question's id.
/// Shared by the editor's "New question" save and the CSV/JSON importer.
pub async fn insert_question(pool: &PgPool, input: &NewQuestionInput<'_>) -> Result<Uuid, String> {
    let cert_row = sqlx::query("select id from certifications where slug = $1")
        .bind(input.cert_slug)
        .fetch_optional(pool)
        .await
        .expect("failed to look up certification");
    let Some(cert_row) = cert_row else {
        return Err(format!("unknown track '{}'", input.cert_slug));
    };
    let cert_id: Uuid = cert_row.try_get("id").expect("id");

    let domain_row = sqlx::query("select id from domains where certification_id = $1 and name = $2")
        .bind(cert_id)
        .bind(input.domain)
        .fetch_optional(pool)
        .await
        .expect("failed to look up domain");
    let Some(domain_row) = domain_row else {
        return Err(format!("domain '{}' does not exist on this track", input.domain));
    };
    let domain_id: Uuid = domain_row.try_get("id").expect("id");

    if input.options.len() < 2 || input.options.len() > 6 {
        return Err("a question needs between 2 and 6 options".to_string());
    }
    if !input.options.iter().any(|(_, correct)| *correct) {
        return Err("at least one option must be marked correct".to_string());
    }

    let next_sort_row = sqlx::query("select coalesce(max(sort_order), -1) + 1 as next from questions where certification_id = $1")
        .bind(cert_id)
        .fetch_one(pool)
        .await
        .expect("failed to compute sort order");
    let sort_order: i32 = next_sort_row.try_get("next").expect("next");

    let inserted = sqlx::query(
        "insert into questions (certification_id, domain_id, body, explain, sort_order, status, difficulty, tags)
         values ($1, $2, $3, $4, $5, $6, $7, $8)
         returning id",
    )
    .bind(cert_id)
    .bind(domain_id)
    .bind(input.body)
    .bind(input.explain)
    .bind(sort_order)
    .bind(input.status)
    .bind(input.difficulty)
    .bind(&input.tags)
    .fetch_one(pool)
    .await
    .expect("failed to insert question");
    let question_id: Uuid = inserted.try_get("id").expect("id");

    for (idx, (body, is_correct)) in input.options.iter().enumerate() {
        sqlx::query(
            "insert into question_options (question_id, option_index, body, is_correct) values ($1, $2, $3, $4)",
        )
        .bind(question_id)
        .bind(idx as i16)
        .bind(body)
        .bind(is_correct)
        .execute(pool)
        .await
        .expect("failed to insert question option");
    }

    Ok(question_id)
}

/// `slots[i]` is the `option_index` that `input.options[i]` had before the
/// edit (see `QuestionFormData::option_slots`); a slot with no existing row is
/// a newly added option.
pub async fn update_question(
    pool: &PgPool,
    id: Uuid,
    input: &NewQuestionInput<'_>,
    slots: &[usize],
) -> Result<(), String> {
    let domain_row = sqlx::query(
        "select d.id from domains d
         join questions q on q.certification_id = d.certification_id
         where q.id = $1 and d.name = $2",
    )
    .bind(id)
    .bind(input.domain)
    .fetch_optional(pool)
    .await
    .expect("failed to look up domain");
    let Some(domain_row) = domain_row else {
        return Err(format!("domain '{}' does not exist on this track", input.domain));
    };
    let domain_id: Uuid = domain_row.try_get("id").expect("id");

    if input.options.len() < 2 || input.options.len() > 6 {
        return Err("a question needs between 2 and 6 options".to_string());
    }
    if !input.options.iter().any(|(_, correct)| *correct) {
        return Err("at least one option must be marked correct".to_string());
    }

    // One transaction: either the question and all of its options are saved,
    // or nothing is.
    let mut tx = pool.begin().await.expect("failed to begin transaction");

    sqlx::query(
        "update questions
         set body = $1, explain = $2, domain_id = $3, difficulty = $4, status = $5, tags = $6, updated_at = now()
         where id = $7",
    )
    .bind(input.body)
    .bind(input.explain)
    .bind(domain_id)
    .bind(input.difficulty)
    .bind(input.status)
    .bind(&input.tags)
    .bind(id)
    .execute(&mut *tx)
    .await
    .expect("failed to update question");

    // Options keep their rows across an edit rather than being deleted and
    // re-inserted: past attempts reference option rows by id
    // (attempt_answer_options), so those rows have to survive, and still
    // mean the same option, when the text changes or letters shift.
    let slots_i16: Vec<i16> = slots.iter().map(|&s| s as i16).collect();

    // Options cleared in the editor. One a learner has picked can't be
    // deleted without rewriting their history.
    let picked_removed = sqlx::query(
        "select o.option_index from question_options o
         where o.question_id = $1 and not (o.option_index = any($2))
           and exists (select 1 from attempt_answer_options aao where aao.question_option_id = o.id)
         order by o.option_index",
    )
    .bind(id)
    .bind(&slots_i16)
    .fetch_all(&mut *tx)
    .await
    .expect("failed to check removed options");
    if !picked_removed.is_empty() {
        let letters: Vec<String> = picked_removed
            .iter()
            .map(|row| crate::model::LETTERS[row.try_get::<i16, _>("option_index").expect("option_index") as usize].to_string())
            .collect();
        // Returning drops tx, which rolls back the question update above too.
        return Err(format!(
            "option {} can't be removed because learners have picked it in past attempts. Edit its text instead, or retire this question and create a new one.",
            letters.join(", ")
        ));
    }

    sqlx::query("delete from question_options where question_id = $1 and not (option_index = any($2))")
        .bind(id)
        .bind(&slots_i16)
        .execute(&mut *tx)
        .await
        .expect("failed to remove old options");

    // Park the surviving rows out of the 0–5 range so renumbering them can't
    // trip unique (question_id, option_index) halfway through.
    sqlx::query("update question_options set option_index = option_index + 100 where question_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .expect("failed to renumber options");

    for (idx, ((body, is_correct), slot)) in input.options.iter().zip(&slots_i16).enumerate() {
        let updated = sqlx::query(
            "update question_options set option_index = $1, body = $2, is_correct = $3
             where question_id = $4 and option_index = $5 + 100",
        )
        .bind(idx as i16)
        .bind(body)
        .bind(is_correct)
        .bind(id)
        .bind(slot)
        .execute(&mut *tx)
        .await
        .expect("failed to update question option");

        if updated.rows_affected() == 0 {
            sqlx::query(
                "insert into question_options (question_id, option_index, body, is_correct) values ($1, $2, $3, $4)",
            )
            .bind(id)
            .bind(idx as i16)
            .bind(body)
            .bind(is_correct)
            .execute(&mut *tx)
            .await
            .expect("failed to insert question option");
        }
    }

    tx.commit().await.expect("failed to commit question update");
    Ok(())
}

/// How many of the requested questions were deleted and how many retired.
pub struct DeleteCounts {
    pub deleted: i64,
    pub retired: i64,
}

/// Hard-deletes questions nobody has answered (options and flags cascade).
/// Ones a learner has answered are retired instead, since `attempt_answers`
/// must keep pointing at them. All in one transaction.
pub async fn delete_questions(pool: &PgPool, ids: &[Uuid]) -> DeleteCounts {
    let mut tx = pool.begin().await.expect("failed to begin transaction");

    let retired = sqlx::query(
        "update questions set status = 'retired', updated_at = now()
         where id = any($1) and exists (select 1 from attempt_answers a where a.question_id = questions.id)",
    )
    .bind(ids)
    .execute(&mut *tx)
    .await
    .expect("failed to retire questions")
    .rows_affected() as i64;

    let deleted = sqlx::query(
        "delete from questions
         where id = any($1) and not exists (select 1 from attempt_answers a where a.question_id = questions.id)",
    )
    .bind(ids)
    .execute(&mut *tx)
    .await
    .expect("failed to delete questions")
    .rows_affected() as i64;

    tx.commit().await.expect("failed to commit question delete");
    DeleteCounts { deleted, retired }
}

/* ================= admin: flagged ================= */

#[derive(serde::Serialize)]
pub struct FlaggedComment {
    pub text: String,
    /// Who reported it, so they can be told when the question is fixed. Both
    /// are `None` if the account was deleted (`reported_by` is set null).
    pub reporter_name: Option<String>,
    pub reporter_email: Option<String>,
    pub when: String,
}

#[derive(serde::Serialize)]
pub struct FlaggedGroup {
    pub question_id: Uuid,
    pub cert_code: String,
    pub domain: String,
    pub body: String,
    pub options: Vec<OptionDetail>,
    pub report_count: i64,
    pub comments: Vec<FlaggedComment>,
    pub when: String,
}

pub async fn flagged_list(pool: &PgPool) -> Vec<FlaggedGroup> {
    let question_rows = sqlx::query(
        "select q.id, c.code as cert_code, d.name as domain, q.body,
                count(f.id) as report_count,
                extract(day from now() - max(f.created_at))::bigint as days_ago
         from question_flags f
         join questions q on q.id = f.question_id
         join certifications c on c.id = q.certification_id
         join domains d on d.id = q.domain_id
         where f.status = 'open'
         group by q.id, c.code, d.name
         order by max(f.created_at) desc",
    )
    .fetch_all(pool)
    .await
    .expect("failed to load flagged questions");

    let mut groups = Vec::with_capacity(question_rows.len());
    for row in question_rows {
        let question_id: Uuid = row.try_get("id").expect("id");

        let option_rows = sqlx::query("select body, is_correct from question_options where question_id = $1 order by option_index")
            .bind(question_id)
            .fetch_all(pool)
            .await
            .expect("failed to load flagged question options");

        let comment_rows = sqlx::query(
            "select f.comment, u.name, u.email::text as email,
                    extract(day from now() - f.created_at)::bigint as days_ago
             from question_flags f
             left join users u on u.id = f.reported_by
             where f.question_id = $1 and f.status = 'open'
             order by f.created_at",
        )
        .bind(question_id)
        .fetch_all(pool)
        .await
        .expect("failed to load flag comments");

        let days_ago: i64 = row.try_get("days_ago").expect("days_ago");
        groups.push(FlaggedGroup {
            question_id,
            cert_code: row.try_get("cert_code").expect("cert_code"),
            domain: row.try_get("domain").expect("domain"),
            body: row.try_get("body").expect("body"),
            options: option_rows
                .into_iter()
                .enumerate()
                .map(|(idx, r)| OptionDetail {
                    letter: crate::model::LETTERS[idx],
                    body: r.try_get("body").expect("body"),
                    is_correct: r.try_get("is_correct").expect("is_correct"),
                })
                .collect(),
            report_count: row.try_get("report_count").expect("report_count"),
            comments: comment_rows
                .into_iter()
                .map(|r| FlaggedComment {
                    text: r.try_get("comment").expect("comment"),
                    reporter_name: r.try_get("name").expect("users.name"),
                    reporter_email: r.try_get("email").expect("users.email"),
                    when: relative_days(r.try_get("days_ago").expect("days_ago")),
                })
                .collect(),
            when: relative_days(days_ago),
        });
    }
    groups
}

/// `resolution` is "kept" or "retired" — "edit" is handled by redirecting to
/// the editor, not by a DB write here.
pub async fn resolve_flags(pool: &PgPool, question_id: Uuid, resolution: &str) {
    sqlx::query(
        "update question_flags set status = 'resolved', resolution = $1, resolved_at = now()
         where question_id = $2 and status = 'open'",
    )
    .bind(resolution)
    .bind(question_id)
    .execute(pool)
    .await
    .expect("failed to resolve flags");

    if resolution == "retired" {
        sqlx::query("update questions set status = 'retired', updated_at = now() where id = $1")
            .bind(question_id)
            .execute(pool)
            .await
            .expect("failed to retire question");
    }
}

/// A learner's report against a question, shown on the admin Flagged page.
/// A second report from the same person while their first is still open is
/// ignored, so resubmitting doesn't pile up duplicates.
pub async fn flag_question(pool: &PgPool, question_id: Uuid, comment: &str, user_id: Uuid) {
    sqlx::query(
        "insert into question_flags (question_id, comment, reported_by)
         select $1, $2, $3
         where not exists (
             select 1 from question_flags
             where question_id = $1 and reported_by = $3 and status = 'open'
         )",
    )
    .bind(question_id)
    .bind(comment)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("failed to flag question");
}

/* ================= admin: import ================= */

pub async fn log_import(
    pool: &PgPool,
    cert_slug: &str,
    format: &str,
    accepted: i32,
    rejected: i32,
    reason: Option<&str>,
    imported_by: Uuid,
) {
    sqlx::query(
        "insert into imports (certification_id, format, rows_accepted, rows_rejected, rejected_reason, imported_by)
         select id, $2, $3, $4, $5, $6 from certifications where slug = $1",
    )
    .bind(cert_slug)
    .bind(format)
    .bind(accepted)
    .bind(rejected)
    .bind(reason)
    .bind(imported_by)
    .execute(pool)
    .await
    .expect("failed to log import");
}

#[derive(serde::Serialize)]
pub struct ImportRow {
    pub summary: String,
    pub when: String,
    pub outcome: String,
    pub rejected: bool,
}

pub async fn recent_imports(pool: &PgPool, limit: i64) -> Vec<ImportRow> {
    let rows = sqlx::query(
        "select c.code, i.rows_accepted, i.rows_rejected, i.rejected_reason,
                to_char(i.created_at, 'DD Mon') as when_label
         from imports i
         join certifications c on c.id = i.certification_id
         order by i.created_at desc
         limit $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .expect("failed to load recent imports");

    rows.into_iter()
        .map(|row| {
            let accepted: i32 = row.try_get("rows_accepted").expect("rows_accepted");
            let rejected: i32 = row.try_get("rows_rejected").expect("rows_rejected");
            let reason: Option<String> = row.try_get("rejected_reason").ok();
            let code: String = row.try_get("code").expect("code");
            ImportRow {
                summary: format!("{accepted} questions → {code}"),
                when: row.try_get("when_label").expect("when_label"),
                outcome: if rejected > 0 {
                    format!("{rejected} rows rejected{}", reason.map(|r| format!(": {r}")).unwrap_or_default())
                } else {
                    "All rows accepted".to_string()
                },
                rejected: rejected > 0,
            }
        })
        .collect()
}

/* ================= admin: users ================= */

#[derive(serde::Serialize)]
pub struct AdminUserRow {
    pub id: String,
    pub name: String,
    pub email: String,
    pub role: String,
    pub attempts: i64,
    pub mean_score: Option<i64>,
    pub last_active: String,
    pub status: String,
}

/// Every user with their highest-precedence role, attempt count, mean
/// score, and last-active readout, for the admin users page.
pub async fn admin_users(pool: &PgPool) -> Vec<AdminUserRow> {
    let rows = sqlx::query(
        "select u.id, u.name, u.email::text as email, u.status,
                (select r.name from user_roles ur join roles r on r.id = ur.role_id
                 where ur.user_id = u.id
                 order by case r.name when 'admin' then 0 when 'editor' then 1 else 2 end
                 limit 1) as role,
                (select count(*) from attempts a where a.user_id = u.id and a.finished_at is not null) as attempts,
                (select round(avg(a.pct))::bigint from attempts a where a.user_id = u.id and a.finished_at is not null) as mean_score,
                extract(day from now() - u.last_active_at)::bigint as days_ago
         from users u
         order by u.created_at",
    )
    .fetch_all(pool)
    .await
    .expect("failed to load admin users");

    rows.into_iter()
        .map(|row| {
            let days_ago: Option<i64> = row.try_get("days_ago").ok();
            AdminUserRow {
                id: row.try_get::<Uuid, _>("id").expect("id").to_string(),
                name: row.try_get("name").expect("name"),
                email: row.try_get("email").expect("email"),
                role: row.try_get::<Option<String>, _>("role").ok().flatten().unwrap_or_else(|| "Learner".to_string()),
                attempts: row.try_get("attempts").expect("attempts"),
                mean_score: row.try_get("mean_score").ok(),
                last_active: days_ago.map(relative_days).unwrap_or_else(|| "never".to_string()),
                status: row.try_get("status").expect("status"),
            }
        })
        .collect()
}

pub async fn toggle_user_status(pool: &PgPool, email: &str) {
    sqlx::query(
        "update users set status = case status when 'active' then 'suspended' else 'active' end where email = $1",
    )
    .bind(email)
    .execute(pool)
    .await
    .expect("failed to toggle user status");
}

/// (name, email) for the admin's edit-user panel.
pub async fn user_for_edit(pool: &PgPool, id: Uuid) -> Option<(String, String)> {
    let row = sqlx::query("select name, email::text as email from users where id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .expect("failed to load user")?;
    Some((row.try_get("name").expect("name"), row.try_get("email").expect("email")))
}

/// Saves an admin's edits to a user. With a new password, the user's other
/// sessions are ended so an old login can't outlive the change; `keep_session`
/// is the editing admin's own session, kept when they change their own password.
/// `Err` carries a message fit to show in the form.
pub async fn update_user(
    pool: &PgPool,
    id: Uuid,
    name: &str,
    email: &str,
    password_hash: Option<&str>,
    keep_session: Option<Uuid>,
) -> Result<(), String> {
    let mut tx = pool.begin().await.expect("failed to begin transaction");

    let updated = sqlx::query(
        "update users
         set name = $2, email = $3, password_hash = coalesce($4, password_hash), updated_at = now()
         where id = $1",
    )
    .bind(id)
    .bind(name)
    .bind(email)
    .bind(password_hash)
    .execute(&mut *tx)
    .await;
    match updated {
        Ok(r) if r.rows_affected() == 0 => return Err("That user no longer exists.".to_string()),
        Ok(_) => {}
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            return Err("Another account already uses that email.".to_string());
        }
        Err(e) => panic!("failed to update user: {e}"),
    }

    if password_hash.is_some() {
        sqlx::query("delete from sessions where user_id = $1 and id is distinct from $2")
            .bind(id)
            .bind(keep_session)
            .execute(&mut *tx)
            .await
            .expect("failed to end user sessions");
    }

    tx.commit().await.expect("failed to commit user update");
    Ok(())
}

pub async fn touch_last_active(pool: &PgPool, user_id: Uuid) {
    sqlx::query("update users set last_active_at = now() where id = $1")
        .bind(user_id)
        .execute(pool)
        .await
        .ok();
}

/* ================= learner accounts ================= */

/// (id, password_hash, status) for a login attempt; email matching is
/// case-insensitive because the column is citext.
pub async fn user_for_login(pool: &PgPool, email: &str) -> Option<(Uuid, String, String)> {
    let row = sqlx::query("select id, password_hash, status from users where email = $1")
        .bind(email)
        .fetch_optional(pool)
        .await
        .expect("user lookup failed")?;
    Some((
        row.try_get("id").expect("users.id"),
        row.try_get("password_hash").expect("users.password_hash"),
        row.try_get("status").expect("users.status"),
    ))
}

/// Creates a learner account holding the `user` role. `Err` carries a
/// message fit to show on the signup form.
pub async fn create_user(pool: &PgPool, name: &str, email: &str, password_hash: &str) -> Result<Uuid, String> {
    let mut tx = pool.begin().await.expect("failed to begin transaction");

    let inserted = sqlx::query("insert into users (name, email, password_hash) values ($1, $2, $3) returning id")
        .bind(name)
        .bind(email)
        .bind(password_hash)
        .fetch_one(&mut *tx)
        .await;
    let user_id: Uuid = match inserted {
        Ok(row) => row.try_get("id").expect("users.id"),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            return Err("An account with that email already exists.".to_string());
        }
        Err(e) => panic!("failed to create user: {e}"),
    };

    sqlx::query("insert into user_roles (user_id, role_id) select $1, id from roles where name = 'user'")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .expect("failed to grant user role");

    tx.commit().await.expect("failed to commit signup");
    Ok(user_id)
}

/* ================= blog ================= */

/// A published post as it appears on the public /blog list.
#[derive(serde::Serialize)]
pub struct PostListItem {
    pub slug: String,
    pub title: String,
    pub summary: String,
    pub date: String,
    pub minutes: i64,
}

/// "October 20, 2026"
const POST_DATE_SQL: &str = "to_char(p.published_at, 'FMMonth FMDD, YYYY')";
/// Rough reading time at ~200 words a minute, never less than 1.
const READING_MINUTES_SQL: &str =
    "greatest(1, ceil(coalesce(array_length(regexp_split_to_array(trim(p.body), '\\s+'), 1), 0) / 200.0))::bigint";

pub async fn published_posts(pool: &PgPool) -> Vec<PostListItem> {
    let sql = format!(
        "select p.slug, p.title, p.summary, {POST_DATE_SQL} as date, {READING_MINUTES_SQL} as minutes
         from posts p
         where p.status = 'published'
         order by p.published_at desc, p.id desc"
    );
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .fetch_all(pool)
        .await
        .expect("failed to load posts")
        .into_iter()
        .map(|row| PostListItem {
            slug: row.try_get("slug").expect("slug"),
            title: row.try_get("title").expect("title"),
            summary: row.try_get("summary").expect("summary"),
            date: row.try_get("date").expect("date"),
            minutes: row.try_get("minutes").expect("minutes"),
        })
        .collect()
}

pub struct Post {
    pub title: String,
    pub summary: String,
    /// Markdown source.
    pub body: String,
    pub date: String,
    pub minutes: i64,
    pub author: Option<String>,
}

/// One published post by slug; drafts are treated as not found.
pub async fn published_post(pool: &PgPool, slug: &str) -> Option<Post> {
    let sql = format!(
        "select p.title, p.summary, p.body, {POST_DATE_SQL} as date, {READING_MINUTES_SQL} as minutes, u.name as author
         from posts p
         left join users u on u.id = p.author_id
         where p.slug = $1 and p.status = 'published'"
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(slug)
        .fetch_optional(pool)
        .await
        .expect("failed to load post")?;
    Some(Post {
        title: row.try_get("title").expect("title"),
        summary: row.try_get("summary").expect("summary"),
        body: row.try_get("body").expect("body"),
        date: row.try_get("date").expect("date"),
        minutes: row.try_get("minutes").expect("minutes"),
        author: row.try_get("author").expect("author"),
    })
}

/* ================= admin: blog ================= */

#[derive(serde::Serialize)]
pub struct AdminPostRow {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub status: String,
    pub published: String,
    pub edited: String,
}

pub async fn admin_posts(pool: &PgPool) -> Vec<AdminPostRow> {
    let sql = format!(
        "select p.id, p.slug, p.title, p.status, {POST_DATE_SQL} as published,
                extract(day from now() - p.updated_at)::bigint as days_ago
         from posts p
         order by p.updated_at desc, p.id desc"
    );
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .fetch_all(pool)
        .await
        .expect("failed to load admin posts")
        .into_iter()
        .map(|row| AdminPostRow {
            id: row.try_get("id").expect("id"),
            slug: row.try_get("slug").expect("slug"),
            title: row.try_get("title").expect("title"),
            status: row.try_get("status").expect("status"),
            published: row.try_get::<Option<String>, _>("published").expect("published").unwrap_or_else(|| "—".to_string()),
            edited: relative_days(row.try_get("days_ago").expect("days_ago")),
        })
        .collect()
}

/// What the post editor shows and submits.
#[derive(Default)]
pub struct PostInput {
    pub title: String,
    pub slug: String,
    pub summary: String,
    pub body: String,
    pub status: String,
}

pub async fn post_for_edit(pool: &PgPool, id: Uuid) -> Option<PostInput> {
    let row = sqlx::query("select title, slug, summary, body, status from posts where id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .expect("failed to load post")?;
    Some(PostInput {
        title: row.try_get("title").expect("title"),
        slug: row.try_get("slug").expect("slug"),
        summary: row.try_get("summary").expect("summary"),
        body: row.try_get("body").expect("body"),
        status: row.try_get("status").expect("status"),
    })
}

/// Creates (`id` None) or updates a post and returns its id. `published_at`
/// is stamped the first time the post is published and kept afterwards.
/// `Err` carries a message fit to show in the editor.
pub async fn save_post(pool: &PgPool, id: Option<Uuid>, input: &PostInput, author: Uuid) -> Result<Uuid, String> {
    let result = match id {
        None => {
            sqlx::query(
                "insert into posts (slug, title, summary, body, status, published_at, author_id)
                 values ($1, $2, $3, $4, $5, case when $5 = 'published' then now() end, $6)
                 returning id",
            )
            .bind(&input.slug)
            .bind(&input.title)
            .bind(&input.summary)
            .bind(&input.body)
            .bind(&input.status)
            .bind(author)
            .fetch_optional(pool)
            .await
        }
        Some(id) => {
            sqlx::query(
                "update posts
                 set slug = $2, title = $3, summary = $4, body = $5, status = $6,
                     published_at = case when $6 = 'published' then coalesce(published_at, now()) else published_at end,
                     updated_at = now()
                 where id = $1
                 returning id",
            )
            .bind(id)
            .bind(&input.slug)
            .bind(&input.title)
            .bind(&input.summary)
            .bind(&input.body)
            .bind(&input.status)
            .fetch_optional(pool)
            .await
        }
    };
    match result {
        Ok(Some(row)) => Ok(row.try_get("id").expect("id")),
        Ok(None) => Err("That post no longer exists.".to_string()),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            Err(format!("Another post already uses the URL /blog/{}. Choose a different slug.", input.slug))
        }
        Err(e) => panic!("failed to save post: {e}"),
    }
}

pub async fn delete_post(pool: &PgPool, id: Uuid) {
    sqlx::query("delete from posts where id = $1").bind(id).execute(pool).await.expect("failed to delete post");
}

/* ================= admin: sidebar ================= */

/// (live questions, open flags, users) — the sidebar's nav badges.
pub async fn nav_counts(pool: &PgPool) -> (i64, i64, i64, i64) {
    let row = sqlx::query(
        "select (select count(*) from questions where status = 'live') as questions,
                (select count(distinct question_id) from question_flags where status = 'open') as flagged,
                (select count(*) from users) as users,
                (select count(*) from posts) as posts",
    )
    .fetch_one(pool)
    .await
    .expect("failed to load nav counts");

    (
        row.try_get("questions").expect("questions"),
        row.try_get("flagged").expect("flagged"),
        row.try_get("users").expect("users"),
        row.try_get("posts").expect("posts"),
    )
}

#[derive(serde::Serialize)]
pub struct TrackOption {
    pub slug: String,
    pub code: String,
    pub name: String,
}

pub async fn track_options(pool: &PgPool) -> Vec<TrackOption> {
    let rows = sqlx::query("select slug, code, name from certifications order by id")
        .fetch_all(pool)
        .await
        .expect("failed to load track options");

    rows.into_iter()
        .map(|row| TrackOption {
            slug: row.try_get("slug").expect("slug"),
            code: row.try_get("code").expect("code"),
            name: row.try_get("name").expect("name"),
        })
        .collect()
}

/// Total questions of any status, across every track — the questions
/// page's unfiltered header count.
pub async fn questions_grand_total(pool: &PgPool) -> i64 {
    sqlx::query("select count(*) as n from questions")
        .fetch_one(pool)
        .await
        .expect("failed to count questions")
        .try_get("n")
        .expect("n")
}

pub async fn today_label(pool: &PgPool) -> String {
    sqlx::query("select to_char(now(), 'DD Month YYYY') as today")
        .fetch_one(pool)
        .await
        .expect("failed to load today's date")
        .try_get("today")
        .expect("today")
}
