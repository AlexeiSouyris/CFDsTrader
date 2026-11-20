-- Diario de propuestas y resultados (spec §15b, #13). Columnas = contracts/proposal.v1 y
-- contracts/outcome.v1. Como en 0001, las CHECK repiten lo esencial del contrato como última
-- barrera: la base no acepta una propuesta imposible aunque alguien se saltee el motor.
-- Solo el motor (Rust) escribe; Python lee.

CREATE TABLE proposals (
    id            TEXT        PRIMARY KEY CHECK (id ~ '^[A-Za-z0-9._:-]{1,200}$'),
    source        TEXT        NOT NULL CHECK (source IN ('backtest', 'paper', 'live')),
    run_id        TEXT        NOT NULL CHECK (run_id ~ '^[A-Za-z0-9._-]{1,100}$'),
    instrument_id TEXT        NOT NULL REFERENCES instruments (id),
    side          TEXT        NOT NULL CHECK (side IN ('long', 'short')),
    signal_at     TIMESTAMPTZ NOT NULL,
    reference     NUMERIC     NOT NULL CHECK (reference > 0),
    stop          NUMERIC     NOT NULL CHECK (stop > 0),
    tp1           NUMERIC     NOT NULL CHECK (tp1 > 0),
    size          NUMERIC     NOT NULL CHECK (size > 0),
    leverage      NUMERIC     NOT NULL CHECK (leverage > 0),
    score         INTEGER     NOT NULL,
    breakdown     JSONB       NOT NULL CHECK (jsonb_typeof(breakdown) = 'array'),
    triggers      JSONB       NOT NULL CHECK (jsonb_typeof(triggers) = 'array'),
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- El stop y el TP1 quedan del lado correcto de la entrada.
    CONSTRAINT proposals_levels_consistent CHECK (
        (side = 'long' AND stop < reference AND tp1 > reference)
        OR (side = 'short' AND stop > reference AND tp1 < reference)
    )
);

CREATE INDEX proposals_run_idx ON proposals (run_id);
CREATE INDEX proposals_instrument_signal_idx ON proposals (instrument_id, signal_at);

CREATE TABLE outcomes (
    proposal_id TEXT        NOT NULL REFERENCES proposals (id),
    kind        TEXT        NOT NULL CHECK (kind IN ('simulated', 'real')),
    entry_at    TIMESTAMPTZ NOT NULL,
    entry_price NUMERIC     NOT NULL CHECK (entry_price > 0),
    exits       JSONB       NOT NULL CHECK (jsonb_typeof(exits) = 'array'),
    commission  NUMERIC     NOT NULL CHECK (commission >= 0),
    funding     NUMERIC     NOT NULL,
    gross_pnl   NUMERIC     NOT NULL,
    net_pnl     NUMERIC     NOT NULL,
    hit_tp1     BOOLEAN     NOT NULL,
    hit_stop    BOOLEAN     NOT NULL,
    exit_reason TEXT        NOT NULL CHECK (
        exit_reason IN ('stop', 'tp1', 'break_even', 'trailing', 'time', 'forced', 'end_of_data')
    ),
    inserted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (proposal_id, kind)
);
