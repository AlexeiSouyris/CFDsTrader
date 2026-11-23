-- Aprendizaje continuo (fase 8b, spec §15b y §1.2, #327). Tres tablas de SOLO AGREGAR (lo
-- imponen los triggers): los informes (learning_report.v1), las propuestas de cambio que traen
-- (change_proposal.v1, siempre nacen "pendiente_de_voto") y los votos (change_vote.v1, uno por
-- miembro y propuesta). El estado de una propuesta se calcula con sus votos: nunca se
-- actualiza una fila. NINGUNA tabla aplica cambios: un parámetro solo cambia en config/ con el
-- voto registrado acá.

CREATE TABLE learning_reports (
    report_id   TEXT        PRIMARY KEY CHECK (report_id ~ '^[A-Za-z0-9._:-]{1,200}$'),
    period_from TIMESTAMPTZ NOT NULL,
    period_to   TIMESTAMPTZ NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL,
    report      JSONB       NOT NULL CHECK (jsonb_typeof(report) = 'object'),
    inserted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT learning_reports_period CHECK (period_to > period_from),
    CONSTRAINT learning_reports_same_id CHECK (report ->> 'report_id' = report_id)
);

CREATE TABLE change_proposals (
    change_id   TEXT        PRIMARY KEY CHECK (change_id ~ '^[A-Za-z0-9._:-]{1,200}$'),
    report_id   TEXT        NOT NULL REFERENCES learning_reports (report_id),
    created_at  TIMESTAMPTZ NOT NULL,
    proposal    JSONB       NOT NULL CHECK (jsonb_typeof(proposal) = 'object'),
    inserted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT change_proposals_same_id CHECK (proposal ->> 'change_id' = change_id),
    CONSTRAINT change_proposals_same_report CHECK (proposal ->> 'report_id' = report_id),
    CONSTRAINT change_proposals_pending CHECK (proposal ->> 'status' = 'pendiente_de_voto')
);

CREATE TABLE change_votes (
    vote_id     TEXT        PRIMARY KEY CHECK (vote_id ~ '^[A-Za-z0-9._:-]{1,200}$'),
    change_id   TEXT        NOT NULL REFERENCES change_proposals (change_id),
    member_id   TEXT        NOT NULL CHECK (member_id ~ '^[A-Za-z0-9._-]{1,64}$'),
    in_favor    BOOLEAN     NOT NULL,
    voted_at    TIMESTAMPTZ NOT NULL,
    vote        JSONB       NOT NULL CHECK (jsonb_typeof(vote) = 'object'),
    inserted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Un voto por miembro y propuesta (spec §1.2: 1 voto por miembro).
    CONSTRAINT change_votes_one_per_member UNIQUE (change_id, member_id),
    CONSTRAINT change_votes_same_id CHECK (vote ->> 'vote_id' = vote_id)
);

CREATE INDEX change_proposals_report ON change_proposals (report_id);

CREATE FUNCTION learning_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION '% es de solo agregar: no se modifica ni se borra', TG_TABLE_NAME;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER learning_reports_no_update_or_delete
    BEFORE UPDATE OR DELETE ON learning_reports
    FOR EACH ROW EXECUTE FUNCTION learning_append_only();

CREATE TRIGGER change_proposals_no_update_or_delete
    BEFORE UPDATE OR DELETE ON change_proposals
    FOR EACH ROW EXECUTE FUNCTION learning_append_only();

CREATE TRIGGER change_votes_no_update_or_delete
    BEFORE UPDATE OR DELETE ON change_votes
    FOR EACH ROW EXECUTE FUNCTION learning_append_only();
