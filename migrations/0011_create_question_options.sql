create table question_options (
    id           bigserial primary key,
    question_id  bigint not null references questions(id) on delete cascade,
    option_index smallint not null,
    body         text not null,
    is_correct   boolean not null default false,
    unique (question_id, option_index)
);
