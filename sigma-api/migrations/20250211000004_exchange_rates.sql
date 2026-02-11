CREATE TABLE exchange_rates (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    from_currency TEXT NOT NULL,
    to_currency   TEXT NOT NULL,
    rate          NUMERIC(12,6) NOT NULL,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(from_currency, to_currency)
);
