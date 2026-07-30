-- Session model: DB-backed opaque tokens (the row's own id is the cookie value), not JWTs —
-- revocation (logout, account deletion) is then just "delete the row."
create table sessions (
    id uuid primary key default gen_random_uuid(),
    user_id uuid not null references users(id) on delete cascade,
    expires_at timestamptz not null,
    created_at timestamptz not null default now()
);

create index sessions_user_id_idx on sessions (user_id);
create index sessions_expires_at_idx on sessions (expires_at);
