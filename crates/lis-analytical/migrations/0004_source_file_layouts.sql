-- File templates of result-file sources (result files brief F2, §13;
-- REQ-ANA-003, REQ-INT-005). A registration with a layout row is a file
-- source. The layout is the typed `FileLayout` as JSON, written once while
-- the registration is a draft and fixed by its approval, like its test codes.

CREATE TABLE ana_source_file_layouts (
    registration_id TEXT    NOT NULL PRIMARY KEY REFERENCES ana_source_registrations (id),
    layout_json     TEXT    NOT NULL CHECK (json_valid(layout_json)),
    layout_version  INTEGER NOT NULL CHECK (layout_version = 1)
) STRICT;

CREATE TRIGGER ana_source_file_layouts_draft_only BEFORE INSERT ON ana_source_file_layouts
WHEN (SELECT state FROM ana_source_registrations WHERE id = NEW.registration_id) IS NOT 'draft'
BEGIN SELECT RAISE(ABORT, 'a file layout is fixed once its registration is approved'); END;
CREATE TRIGGER ana_source_file_layouts_no_update BEFORE UPDATE ON ana_source_file_layouts
BEGIN SELECT RAISE(ABORT, 'source file layouts are append-only'); END;
CREATE TRIGGER ana_source_file_layouts_no_delete BEFORE DELETE ON ana_source_file_layouts
BEGIN SELECT RAISE(ABORT, 'source file layouts are append-only'); END;
