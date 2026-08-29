-- Demo/dev data so the Flagged and Import pages have something real to
-- show, the same way 0021-0023 seeded demo attempts. Not learner-generated;
-- clearly a seed for local development.

insert into question_flags (question_id, comment, reported_by, created_at)
select q.id, v.comment, u.id, now() - v.age::interval
from (values
    ('saa', 10, 'DAX is not needed to relieve a hot write key. Sharding alone should be the answer.', '1 day'),
    ('cka', 5, 'The wording implies exactly one cause, but a readiness failure also empties endpoints.', '4 days')
) as v(cert_slug, q_sort_order, comment, age)
join certifications c on c.slug = v.cert_slug
join questions q on q.certification_id = c.id and q.sort_order = v.q_sort_order
join users u on u.email = 'demo@drill.test';

insert into imports (certification_id, format, rows_accepted, rows_rejected, rejected_reason, imported_by, created_at)
select c.id, v.format, v.accepted, v.rejected, v.reason, u.id, now() - v.age::interval
from (values
    ('cka', 'json', 12, 0, null, '10 days'),
    ('saa', 'csv', 12, 0, null, '18 days')
) as v(cert_slug, format, accepted, rejected, reason, age)
join certifications c on c.slug = v.cert_slug
join users u on u.email = 'admin@drill.test';
