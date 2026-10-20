# Drill — project reference

Drill is a practice-exam web app for cloud certifications (AWS, Google Cloud,
Kubernetes). Learners pick a track, answer a 20-question set one page at a
time, and get a score with a per-domain breakdown and an answer review.
Admins manage the question bank through `/admin`.

It is a server-rendered Rust app: **Axum 0.7** for HTTP, **Tera** for HTML
templates, **sqlx 0.9** on **PostgreSQL**, with Tailwind (CDN) and a little
vanilla JS on the client. The crate is named `drill`.

> `README.md` is the short version: what the app is and how to run it.
> `db.md` is the schema design doc. This file goes into detail on how the
> code works.

---

## Layout

```
Cargo.toml              crate "drill"
src/
  main.rs               startup, learner routes (home, quiz, results), render helpers
  model.rs              Cert/Question types, scoring (pure, no I/O)
  db.rs                 every SQL query, learner and admin (~1000 lines)
  admin.rs              /admin router, AdminUser auth extractor, admin handlers
  auth.rs               Argon2id hash/verify, sessions, CurrentUser extractor
examples/
  hash_password.rs      `cargo run --example hash_password -- <pw>` → prints a hash
migrations/             0000–0031, embedded at compile time by sqlx::migrate!
templates/
  home.html question.html results.html   learner pages
  login.html            log in + sign up side by side (guests land here)
  partials/{head,foot}.html               doctype, fonts, Tailwind CDN, app.js
  admin/*.html + admin/partials/          admin pages, sidebar, edit drawer
static/
  app.js                quiz elapsed-time clock
  tailwind.config.js    theme tokens (ink, rust, muted, bg-right/bg-wrong …)
Dockerfile, docker-compose.yml, .github/workflows/deploy.yml
db.md                   schema design rationale
```

---

## Running locally

```sh
docker compose up -d db                 # Postgres 16, user/pass/db = drill
cp .env.example .env                    # DATABASE_URL=postgres://drill:drill@localhost:5432/drill
cargo run                               # http://localhost:3000
```

- Migrations run automatically at startup (`sqlx::migrate!("./migrations")`).
- `templates/` and `static/` are read from the **current working directory**
  at runtime, so run from the crate root. Startup panics if the
  `templates/**/*.html` glob matches nothing.
- Templates are parsed once at startup, so restart after editing them.
- To run everything in containers instead: `docker compose up --build`.

**Dev logins** (seeded by migration `0024`):

| email | password | roles |
|---|---|---|
| `admin@drill.test` | `admin12345` | user, admin |
| `demo@drill.test`  | `demo12345`  | user (gets 403 on /admin) |

---

## Routes

### Learner (`src/main.rs`)

| Method | Path | Handler | Notes |
|---|---|---|---|
| GET  | `/?provider=All\|AWS\|GCP\|K8s` | `home` | Published tracks with live-question counts |
| GET  | `/login?next=/quiz/saa` | `login_page` | Log in and Sign up forms side by side. Already logged in → redirect to `next` |
| POST | `/login` | `login_submit` | creates a session, redirects to `next` |
| POST | `/signup` | `signup_submit` | creates the user with the `user` role, logs them in, redirects to `next` |
| POST | `/logout` | `logout` | ends the session, redirects to `/` |
| GET  | `/quiz/:cert_id` | `quiz_start` | **login required**. `:cert_id` is `certifications.slug` (`saa`, `dva`, `ace`, `pca`, `cka`) |
| POST | `/quiz/:cert_id` | `quiz_step` | **login required**. next / prev / skip / finish, or renders results |
| GET  | `/static/*` | `ServeDir` | |

### Admin (`src/admin.rs`, nested under `/admin`)

| Method | Path | Purpose |
|---|---|---|
| GET/POST | `/admin/login` | email+password login, creates a DB session |
| POST | `/admin/logout` | deletes the session row, clears the cookie |
| GET  | `/admin` | dashboard: stats, needs-attention list, weakest domains, tracks table |
| POST | `/admin/tracks/:slug/toggle-published` | show or hide a track for learners |
| GET  | `/admin/questions` | search, filter (track, status), sort and page through questions. Server-side pagination with `per_page` of 10/15/20 and `page`. Default sort is Edited, newest first |
| POST | `/admin/questions/:id/delete` | the Delete button in the edit sidebar |
| POST | `/admin/questions/delete` | bulk delete of the rows ticked in the list (`id` repeated). Both delete routes use `db::delete_questions`: unanswered questions are deleted, answered ones are retired instead to keep learners' history, and the redirect carries `deleted=N&retired=M` for the notice |
| GET/POST | `/admin/questions/new` | create a question (edit drawer) |
| GET/POST | `/admin/questions/:id` | edit a question |
| GET/POST | `/admin/flagged` | learner reports. Actions: `keep`, `retire` |
| GET/POST | `/admin/import` | bulk paste of JSON or CSV into a track, can save as draft |
| GET/POST | `/admin/users` | list and search users, suspend or reactivate (`toggle=<email>`) |

---

## How the quiz works

The quiz keeps no state on the server. Each question is one HTML form. On
every POST the form carries:

- `index`: current question position
- `action`: `next` | `prev` | `skip` | `finish`
- `choice`: repeated once per selected option index (multi-select = checkboxes)
- `answers`: JSON of everything answered so far, `{ "0": [1], "3": [0,2] }`
  (`model::Answers = BTreeMap<usize, Vec<usize>>`)
- `elapsed`: seconds, kept up to date by the `#clock` timer in `app.js`
- `instant` / `revealed`: instant-feedback mode. The first submit shows the
  answer and explanation, the second submit moves on.

`quiz_step` parses the body with `form_urlencoded` and **not** `axum::Form`,
because `choice` repeats. It updates `answers` and then renders the next
question, or `results()` once the set is finished.

Each request calls `db::load_cert(slug)` to fetch the track again with only its
`status = 'live'` questions, ordered by `sort_order`. `Cert::set()` takes the
first `SET_LENGTH` (20) of them. There is no randomisation yet.

**Scoring** (`model::score`) is a pure function. A question counts as correct
only when the set of picked options exactly matches the correct set
(`is_correct` sorts both and compares). Results show the overall %, the
correct/wrong/skipped counts, a per-domain % (bar turns `bg-rust` below
`pass_mark`), and a review table. The verdict text uses ≥85 %, ≥ pass mark,
and below.

**Nothing is saved yet.** `attempts` / `attempt_answers` tables exist (and
seed data feeds the admin dashboard), but `results()` does not write to
them. On the home page, `best_label` is hard-coded to `"no attempt yet"`
and `history` is an empty list.

---

## Data model (Postgres)

`db.md` has the full rationale. In short:

- **Auth/RBAC:** `users` (uuid id, citext email, Argon2id `password_hash`,
  `status` active/suspended, `last_active_at`), `sessions` (uuid id = cookie
  value, 30-day expiry), `roles` (`user`, `admin`, `editor`), `permissions`
  (`admin.access`, `certifications.write`, …), `role_permissions`, `user_roles`.
- **Content:** `providers` (AWS/GCP/K8s) → `certifications` (`slug`, `code`,
  `pass_mark`, `published`) → `domains` → `questions` (`status`
  draft/live/retired, `difficulty` Easy/Medium/Hard, `tags text[]`,
  `sort_order`) → `question_options` (`option_index` 0–5 → letters A–F,
  `is_correct`).
- **Activity:** `attempts` → `attempt_answers` → `attempt_answer_options`.
- **Admin:** `question_flags` (learner reports, open/resolved, resolution
  edited/kept/retired), `imports` (log of each bulk import).

IDs: `bigserial` everywhere except `users`/`sessions` (uuid) and `roles`
(smallserial). URLs use `certifications.slug`, never the numeric id.

Seed content: 5 tracks (`saa`, `dva`, `ace`, `pca`, `cka`) with 60 questions
in total, plus demo users, attempts, flags and imports (`0015`–`0023`, `0031`).

### Adding a migration

Add the next numbered file, e.g. `migrations/0032_<what>.sql`. Never edit a
migration that has already run, because sqlx checksums them and startup will
fail. Migrations are embedded **at compile time**, so rebuild after you add one.

---

## Auth

### Learners

- The home page is open to everyone. Taking a quiz requires an account.
- `auth::CurrentUser` is an axum extractor. Adding it as a handler parameter
  gates the route: a guest (or a suspended user, or an expired session) is
  redirected to `/login?next=<path>`. Use `Option<CurrentUser>` on pages that
  guests can see but that change for logged-in users (the home header).
- On the home page, guests' track cards link straight to
  `/login?next=/quiz/<slug>`. After logging in or signing up, they go on to
  that quiz.
- `auth::safe_next` only accepts same-site paths for `next` (no `//host`), so
  it can't be used as an open redirect.
- Signup (`db::create_user`) trims the name and email and requires a password
  of at least 8 characters (`MIN_PASSWORD_LEN`). It inserts the user and grants
  the `user` role in one transaction. A duplicate email is caught by the unique
  constraint, and the citext column makes it case-insensitive.

### Sessions (shared)

- One cookie, `drill_session` (HttpOnly, SameSite=Lax, path `/`). Its value
  is `sessions.id`, with a 30-day expiry. Learner login and admin login both
  use `auth::create_session` / `auth::end_session`, so logging in on one side
  counts on the other.

### Admin

- `AdminUser` is an axum `FromRequestParts` extractor. Any handler that
  takes it as a parameter is gated. No cookie or an expired session redirects
  to `/admin/login`. A logged-in user without the `admin` role gets a 403
  with `admin/forbidden.html`. That page shows who is logged in and offers
  "Back to practice" or "Log in with a different account", which posts to
  `/admin/logout`.
- Suspended users cannot log in. Login (either side) updates `users.last_active_at`.
- The gate checks the **role name `admin`**, not permissions. The `editor`
  role (`0030`) and the `permissions` table are seeded, but no code checks
  them yet, so editors currently get a 403.

## Import formats (`/admin/import`)

JSON is an array of objects:

```json
[{ "question": "…", "options": ["a","b","c","d"], "answer": [1],
   "domain": "Design Resilient Architectures", "explain": "…", "difficulty": "Medium" }]
```

CSV has a header row, with columns in this order:
`question, options (pipe-separated), answer (indices, | or , separated), domain, explain, difficulty`.

Each row goes through `db::insert_question`. That function rejects an
unknown track, a domain that doesn't exist on the track (it never creates
domains), fewer than 2 or more than 6 options, or no correct option. A bad
row is counted as rejected and the other rows are still imported. Imported
questions are added at the end of `sort_order`.

## Editing questions

`db::update_question` runs in one transaction, so a failed save changes
nothing. **Option rows are never deleted and re-inserted.**
`attempt_answer_options` points at option rows by id, with no cascade, so
those ids have to stay stable:

- The drawer's letters A–F are the options' current `option_index`, so the
  form tells us which existing row each option is (`option_slots`).
- Editing an option's text or correctness updates its row in place.
- Clearing an option deletes its row. If a learner has picked that option,
  the save is refused with a message, since deleting it would rewrite their
  history. The fix there is to retire the question and write a new one.
- The options that remain are renumbered to close the gap (D can become C)
  but keep their ids. While renumbering, rows are temporarily moved to
  `option_index + 100` so the unique `(question_id, option_index)`
  constraint never trips partway through.

---

## Code conventions

- All SQL lives in `db.rs`. It uses runtime `sqlx::query(...)` with `.bind()`
  and `row.try_get(..)`, not the `query!` macros, so building needs no
  database or `SQLX_OFFLINE` data.
- DB errors are handled with `.expect("…")`, which panics inside the handler.
  Validation errors return `Result<_, String>`.
- Handlers build a Tera `Context` and call `render(state, "x.html", &ctx)`.
  Learner pages start from `base_context()` (header totals), admin pages
  from `sidebar_context()` (nav counts and current user).
- View structs (`CertRow`, `OptionView`, …) are `#[derive(Serialize)]`, and
  Tailwind class names are chosen in Rust (`rule_class`, `bar_class`).
- Don't give a Tailwind colour the same name as a `text-*` keyword (`left`,
  `right`, `center`, `justify`, `start`, `end`). A colour named `right` made
  `text-right` also set the text to that colour, which made right-aligned
  cells nearly invisible. The "correct answer" background is called
  `correct` for that reason.
- Comments explain *why*. Section separators look like
  `/* ---------- name ---------- */`.

---

## Build & deploy

- **CI/CD** (`.github/workflows/deploy.yml`) runs on push to `main`:
  `cargo test --release` → `cargo build --release` → scp the binary and a
  `templates/` + `static/` tarball to `/opt/drill` on the server over SSH,
  then swap them in and `systemctl restart drill`. It runs on
  `ubuntu-24.04` so the glibc version matches the server. Secrets:
  `SSH_KEY`, `SSH_HOST`, `SSH_USER`. The server needs `DATABASE_URL` set
  (e.g. via the systemd unit/env file).
- **Docker:** multi-stage build (`rust:1-slim-bookworm` →
  `debian:bookworm-slim`), runs as user `drill`, port 3000. `migrations/`
  is only needed at build time. `templates/` and `static/` are copied into
  the runtime image.
- The app binds `0.0.0.0:$PORT`, and `PORT` defaults to 3000.

---

## Known gaps / TODO

- Attempts are not saved, and the home page "best score" and "recent attempts"
  are stubs.
- No password reset or email verification yet.
- Logging in is required, but `CurrentUser.id` isn't used yet. It is what
  attempts should be saved against.
- If a session expires mid-quiz, the next POST redirects to `/login` and the
  in-progress answers are lost.
- `editor` role and permission keys are not enforced (the gate checks the
  `admin` role only).
- `load_cert` does not check `certifications.published`, so a hidden track
  can still be opened directly at `/quiz/<slug>`.
- `load_cert` makes one query per question for options (N+1) on every quiz
  POST.
- Answers are keyed by **position** in the set. If a question is retired or
  added during a quiz, later answers line up with the wrong questions.
- Question order is fixed (first 20 by `sort_order`). Randomise in
  `Cert::set()` or in SQL.
- Learners report a question from the quiz page ("Report a problem with this
  question", `action=report` in `quiz_step` → `db::flag_question`). One open
  report per learner per question, and the comment is capped at 1000 characters.
- The `flagged` page actions only support `keep` / `retire`. Nothing sets
  the `edited` resolution, and saving the question in the editor doesn't
  close its open flags.
- There are no tests, so `cargo test` in CI only checks that the crate compiles.
- Tailwind is loaded from the CDN. For production, build a CSS file with the
  Tailwind CLI (`content: ['./templates/**/*.html']`) into `static/` and
  replace the CDN scripts in `partials/head.html` and `admin/partials/head.html`.
- Admin forms have no CSRF protection beyond SameSite=Lax cookies.
