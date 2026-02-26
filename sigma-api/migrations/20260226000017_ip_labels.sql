-- IP address label types (dynamic, DB-driven)
CREATE TABLE ip_labels (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name       TEXT NOT NULL UNIQUE,
    label      TEXT NOT NULL,
    short      TEXT NOT NULL DEFAULT '',
    color      TEXT NOT NULL DEFAULT 'gray',
    sort_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER trg_ip_labels_updated
    BEFORE UPDATE ON ip_labels
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

INSERT INTO ip_labels (name, label, short, color, sort_order) VALUES
    ('china-telecom', '电信',     'CT',  'red',    1),
    ('china-unicom',  '联通',     'CU',  'orange', 2),
    ('china-mobile',  '移动',     'CM',  'green',  3),
    ('china-cernet',  '教育网',   'EDU', 'purple', 4),
    ('overseas',      '海外',     'OS',  'blue',   5),
    ('internal',      '内网',     'LAN', 'gray',   6),
    ('anycast',       'Anycast', 'AC',  'cyan',   7)
ON CONFLICT (name) DO NOTHING;
