CREATE TABLE passport_account (
    id serial PRIMARY KEY,
    local_user_id int NOT NULL REFERENCES local_user (id) ON UPDATE CASCADE ON DELETE CASCADE,
    identifier text NOT NULL UNIQUE
);
CREATE INDEX idx_passport_account_local_user ON passport_account (local_user_id);
CREATE TABLE passport_nonce (
    nonce text PRIMARY KEY,
    published_at timestamptz NOT NULL DEFAULT now()
);