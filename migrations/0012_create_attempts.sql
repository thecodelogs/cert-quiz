create table attempts (
    id               uuid primary key default uuid_v7(),
    user_id          uuid not null references users(id) on delete cascade,
    certification_id uuid not null references certifications(id),
    started_at       timestamptz not null default now(),
    finished_at      timestamptz,
    elapsed_seconds  integer not null default 0,
    total_questions  integer not null,
    correct_count    integer not null default 0,
    wrong_count      integer not null default 0,
    skipped_count    integer not null default 0,
    pct              integer
);

create index on attempts (user_id, certification_id, finished_at);
