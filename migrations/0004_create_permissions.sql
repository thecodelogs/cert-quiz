create table permissions (
    id          bigserial primary key,
    key         text not null unique,
    description text
);

insert into permissions (key, description) values
    ('admin.access', 'Can open the admin panel at all.'),
    ('certifications.write', 'Create/edit certifications, domains, questions, and options.'),
    ('certifications.delete', 'Delete certifications, domains, questions, and options.'),
    ('users.manage', 'View users and change their roles.'),
    ('attempts.view_all', 'See every user''s attempts, not just their own.'),
    ('roles.manage', 'Edit roles and permissions themselves.');
