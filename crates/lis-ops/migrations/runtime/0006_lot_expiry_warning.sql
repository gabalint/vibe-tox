-- Days before a lot's expiry date from which the lot is listed as expiring soon (#4557).
-- Versions recorded before this setting existed take the default of `policy.rs`; the
-- CHECK repeats its range as a store guard.
ALTER TABLE ops_settings_versions ADD COLUMN lot_expiry_warn_days INTEGER NOT NULL DEFAULT 14
    CHECK (lot_expiry_warn_days BETWEEN 1 AND 365);
