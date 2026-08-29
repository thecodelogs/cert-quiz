insert into user_roles (user_id, role_id)
select u.id, r.id
from (values
    ('demo@drill.test', 'user'),
    ('admin@drill.test', 'user'),
    ('admin@drill.test', 'admin')
) as v(email, role_name)
join users u on u.email = v.email
join roles r on r.name = v.role_name;
