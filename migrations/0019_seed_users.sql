-- Fake accounts for local/dev testing. password_hash is a clearly-labeled
-- placeholder, not a real Argon2id hash -- there is no login-verification
-- code yet for it to matter to.
insert into users (name, email, password_hash) values
    ('Demo User', 'demo@drill.test', '$argon2id$dummy$not-a-real-hash-seed-data-only-replace-once-auth-is-wired'),
    ('Admin User', 'admin@drill.test', '$argon2id$dummy$not-a-real-hash-seed-data-only-replace-once-auth-is-wired');
