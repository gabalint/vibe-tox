-- Whether this installation is a laboratory or the demo laboratory (proposed REQ-OPS-016;
-- starter data brief, decision 1a). Fixed when the row is written at the first start; the
-- row stays append-only, so a demo installation can never become a laboratory one. Rows
-- written before this migration belong to laboratories.

ALTER TABLE ops_installation
    ADD COLUMN kind TEXT NOT NULL DEFAULT 'laboratory' CHECK (kind IN ('laboratory', 'demo'));
