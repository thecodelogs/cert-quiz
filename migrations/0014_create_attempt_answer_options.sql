create table attempt_answer_options (
    attempt_answer_id  bigint not null references attempt_answers(id) on delete cascade,
    question_option_id bigint not null references question_options(id),
    primary key (attempt_answer_id, question_option_id)
);
