create table question_options (
    id           uuid primary key default uuid_v7(),
    question_id  uuid not null references questions(id) on delete cascade,
    option_index smallint not null,
    body         text not null,
    is_correct   boolean not null default false,
    unique (question_id, option_index)
);
