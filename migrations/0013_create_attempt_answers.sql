create table attempt_answers (
    id          bigserial primary key,
    attempt_id  bigint not null references attempts(id) on delete cascade,
    question_id bigint not null references questions(id),
    is_correct  boolean not null,
    answered_at timestamptz not null default now(),
    unique (attempt_id, question_id)
);
