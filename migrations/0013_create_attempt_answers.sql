create table attempt_answers (
    id          uuid primary key default uuid_v7(),
    attempt_id  uuid not null references attempts(id) on delete cascade,
    question_id uuid not null references questions(id),
    is_correct  boolean not null,
    answered_at timestamptz not null default now(),
    unique (attempt_id, question_id)
);
