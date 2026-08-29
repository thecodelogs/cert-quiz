-- Replaces the placeholder password_hash values from 0019 (which were never
-- valid Argon2id hashes) with real ones, so these two seeded accounts can
-- actually log in through the admin panel now that auth exists.
--
-- Dev-only credentials, not secrets:
--   demo@drill.test  / demo12345   (role: user  -- not an admin)
--   admin@drill.test / admin12345  (roles: user, admin)
update users
set password_hash = '$argon2id$v=19$m=19456,t=2,p=1$co5RW59SMTA53mRWlC2aFw$3rAAPShGBTvoPGerQI40lRQVz/QIl5aNZpLi5KBSWPM'
where email = 'demo@drill.test';

update users
set password_hash = '$argon2id$v=19$m=19456,t=2,p=1$Zir4tPiIpEpXJZy4fjj9Ng$UBj2MRc3jFCZBaHcho1LqGd+7MvcjU+fIHe/sEViY+c'
where email = 'admin@drill.test';
