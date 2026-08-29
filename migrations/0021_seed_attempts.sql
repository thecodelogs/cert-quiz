insert into attempts
  (user_id, certification_id, started_at, finished_at, elapsed_seconds,
   total_questions, correct_count, wrong_count, skipped_count, pct)
select u.id, c.id,
       now() - v.started_ago::interval, now() - v.finished_ago::interval,
       v.elapsed_seconds, v.total, v.correct_count, v.wrong_count, v.skipped_count, v.pct
from (values
    ('demo@drill.test', 'saa', '9 days', '8 days', 912, 12, 10, 1, 1, 83),
    ('demo@drill.test', 'cka', '2 days', '1 day', 1384, 12, 8, 3, 1, 67)
) as v(email, cert_slug, started_ago, finished_ago, elapsed_seconds, total, correct_count, wrong_count, skipped_count, pct)
join users u on u.email = v.email
join certifications c on c.slug = v.cert_slug;
