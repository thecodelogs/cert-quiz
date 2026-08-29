-- A learner report against a question. Several reports can pile up on the
-- same question before an admin acts, so this is one row per report, not
-- one row per question.
create table question_flags (
    id          bigserial primary key,
    question_id bigint not null references questions(id) on delete cascade,
    comment     text not null,
    reported_by uuid references users(id) on delete set null,
    status      text not null default 'open' check (status in ('open', 'resolved')),
    resolution  text check (resolution in ('edited', 'kept', 'retired')),
    created_at  timestamptz not null default now(),
    resolved_at timestamptz
);

create index on question_flags (question_id);
create index on question_flags (status);
