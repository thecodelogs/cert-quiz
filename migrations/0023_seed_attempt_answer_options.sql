insert into attempt_answer_options (attempt_answer_id, question_option_id)
select aa.id, qo.id
from (values
    ('demo@drill.test', 'saa', 0, 2),
    ('demo@drill.test', 'saa', 1, 1),
    ('demo@drill.test', 'saa', 2, 0),
    ('demo@drill.test', 'saa', 2, 2),
    ('demo@drill.test', 'saa', 3, 1),
    ('demo@drill.test', 'saa', 4, 1),
    ('demo@drill.test', 'saa', 5, 1),
    ('demo@drill.test', 'saa', 6, 1),
    ('demo@drill.test', 'saa', 7, 0),
    ('demo@drill.test', 'saa', 8, 0),
    ('demo@drill.test', 'saa', 8, 1),
    ('demo@drill.test', 'saa', 9, 1),
    ('demo@drill.test', 'saa', 10, 0),
    ('demo@drill.test', 'cka', 0, 1),
    ('demo@drill.test', 'cka', 1, 0),
    ('demo@drill.test', 'cka', 1, 2),
    ('demo@drill.test', 'cka', 2, 0),
    ('demo@drill.test', 'cka', 3, 1),
    ('demo@drill.test', 'cka', 4, 1),
    ('demo@drill.test', 'cka', 5, 1),
    ('demo@drill.test', 'cka', 6, 1),
    ('demo@drill.test', 'cka', 7, 0),
    ('demo@drill.test', 'cka', 7, 1),
    ('demo@drill.test', 'cka', 8, 0),
    ('demo@drill.test', 'cka', 9, 1),
    ('demo@drill.test', 'cka', 10, 0)
) as v(email, cert_slug, q_sort_order, option_index)
join users u on u.email = v.email
join certifications c on c.slug = v.cert_slug
join attempts a on a.user_id = u.id and a.certification_id = c.id
join questions q on q.certification_id = c.id and q.sort_order = v.q_sort_order
join attempt_answers aa on aa.attempt_id = a.id and aa.question_id = q.id
join question_options qo on qo.question_id = q.id and qo.option_index = v.option_index;
