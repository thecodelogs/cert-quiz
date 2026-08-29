insert into attempt_answers (attempt_id, question_id, is_correct)
select a.id, q.id, v.is_correct
from (values
    ('demo@drill.test', 'saa', 0, true),
    ('demo@drill.test', 'saa', 1, true),
    ('demo@drill.test', 'saa', 2, true),
    ('demo@drill.test', 'saa', 3, true),
    ('demo@drill.test', 'saa', 4, true),
    ('demo@drill.test', 'saa', 5, true),
    ('demo@drill.test', 'saa', 6, true),
    ('demo@drill.test', 'saa', 7, true),
    ('demo@drill.test', 'saa', 8, true),
    ('demo@drill.test', 'saa', 9, true),
    ('demo@drill.test', 'saa', 10, false),
    ('demo@drill.test', 'cka', 0, true),
    ('demo@drill.test', 'cka', 1, true),
    ('demo@drill.test', 'cka', 2, true),
    ('demo@drill.test', 'cka', 3, true),
    ('demo@drill.test', 'cka', 4, true),
    ('demo@drill.test', 'cka', 5, true),
    ('demo@drill.test', 'cka', 6, true),
    ('demo@drill.test', 'cka', 7, true),
    ('demo@drill.test', 'cka', 8, false),
    ('demo@drill.test', 'cka', 9, false),
    ('demo@drill.test', 'cka', 10, false)
) as v(email, cert_slug, q_sort_order, is_correct)
join users u on u.email = v.email
join certifications c on c.slug = v.cert_slug
join attempts a on a.user_id = u.id and a.certification_id = c.id
join questions q on q.certification_id = c.id and q.sort_order = v.q_sort_order;
