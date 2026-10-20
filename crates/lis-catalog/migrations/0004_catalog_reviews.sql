-- Catalog Preview reports and All Data Reviewed (REQ-OPS-015, proposed;
-- REQ-ANA-009). A preview records the drafts its report lists, each at the
-- revision printed. A review approves exactly those drafts, once per preview,
-- and records each blank cutoff the reviewer confirmed as not applicable.
-- The approvals themselves are the ordinary version approvals with their own
-- audit events. All four tables are append-only.

CREATE TABLE cat_catalog_previews (
    id              TEXT NOT NULL PRIMARY KEY,
    generated_by    TEXT NOT NULL,
    generated_at    TEXT NOT NULL,
    -- SHA-256 of the canonical preview content, excluding time and user.
    content_digest  TEXT NOT NULL CHECK (length(content_digest) = 64),
    -- SHA-256 of the stored report artifact.
    artifact_digest TEXT NOT NULL CHECK (length(artifact_digest) = 64)
) STRICT;

CREATE TABLE cat_catalog_preview_drafts (
    preview_id  TEXT    NOT NULL REFERENCES cat_catalog_previews (id),
    record_type TEXT    NOT NULL CHECK (record_type IN ('method_version', 'orderable_version',
        'validity_policy_version')),
    version_id  TEXT    NOT NULL,
    revision    INTEGER NOT NULL CHECK (revision >= 1),
    PRIMARY KEY (preview_id, version_id)
) STRICT;

CREATE TABLE cat_catalog_reviews (
    id          TEXT NOT NULL PRIMARY KEY,
    preview_id  TEXT NOT NULL UNIQUE REFERENCES cat_catalog_previews (id),
    reviewed_by TEXT NOT NULL,
    reviewed_at TEXT NOT NULL
) STRICT;

-- REQ-ANA-009: a blank cutoff in an approved method is marked not applicable.
CREATE TABLE cat_catalog_review_confirmations (
    review_id         TEXT NOT NULL REFERENCES cat_catalog_reviews (id),
    method_version_id TEXT NOT NULL,
    analyte_id        TEXT NOT NULL,
    PRIMARY KEY (review_id, method_version_id, analyte_id),
    FOREIGN KEY (method_version_id, analyte_id)
        REFERENCES cat_method_version_analytes (method_version_id, analyte_id)
) STRICT;

CREATE TRIGGER cat_catalog_previews_no_update BEFORE UPDATE ON cat_catalog_previews
BEGIN
    SELECT RAISE(ABORT, 'catalog previews are append-only');
END;

CREATE TRIGGER cat_catalog_previews_no_delete BEFORE DELETE ON cat_catalog_previews
BEGIN
    SELECT RAISE(ABORT, 'catalog previews are append-only');
END;

CREATE TRIGGER cat_catalog_preview_drafts_no_update BEFORE UPDATE ON cat_catalog_preview_drafts
BEGIN
    SELECT RAISE(ABORT, 'catalog previews are append-only');
END;

CREATE TRIGGER cat_catalog_preview_drafts_no_delete BEFORE DELETE ON cat_catalog_preview_drafts
BEGIN
    SELECT RAISE(ABORT, 'catalog previews are append-only');
END;

CREATE TRIGGER cat_catalog_reviews_no_update BEFORE UPDATE ON cat_catalog_reviews
BEGIN
    SELECT RAISE(ABORT, 'catalog reviews are append-only');
END;

CREATE TRIGGER cat_catalog_reviews_no_delete BEFORE DELETE ON cat_catalog_reviews
BEGIN
    SELECT RAISE(ABORT, 'catalog reviews are append-only');
END;

CREATE TRIGGER cat_catalog_review_confirmations_no_update
BEFORE UPDATE ON cat_catalog_review_confirmations
BEGIN
    SELECT RAISE(ABORT, 'catalog reviews are append-only');
END;

CREATE TRIGGER cat_catalog_review_confirmations_no_delete
BEFORE DELETE ON cat_catalog_review_confirmations
BEGIN
    SELECT RAISE(ABORT, 'catalog reviews are append-only');
END;
