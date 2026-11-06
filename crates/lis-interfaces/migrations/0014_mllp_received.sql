-- Orders received over MLLP (Interfaces, ticket #4555 step 4b): a message read
-- from a partner's connection to the LIS's listener has no file. It keeps the
-- partner's address it came from and the application acknowledgement the LIS
-- answered with (AA, AE or AR), recorded with the message, before the answer
-- leaves. A file read from a folder or SFTP server keeps neither; its file name
-- is empty for a message received over MLLP.

ALTER TABLE ifc_partner_received ADD COLUMN peer TEXT;
ALTER TABLE ifc_partner_received ADD COLUMN ack_code TEXT
    CHECK (ack_code IN ('AA', 'AE', 'AR'));
