-- The report number in a report's file name is `{report}` (ticket #4703); `{accession}`
-- is now the accession number alone. Patterns saved before stood `{accession}` for the
-- report number: they read `{report}` now, so every connection keeps writing the same
-- file names. The acceptance key reads `{report}` as the earlier `{accession}`, so a
-- connection keeps its acceptance. Connection changes keep the pattern as it was saved.

UPDATE ifc_partner_connections
SET file_name_pattern = replace(file_name_pattern, '{accession}', '{report}')
WHERE message_type IN ('report_pdf', 'hl7_oru');
