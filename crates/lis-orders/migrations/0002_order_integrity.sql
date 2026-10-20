-- Defense in depth for accepted add-on authority and append-only order records.

CREATE TRIGGER ord_add_on_authority_on_insert BEFORE INSERT ON ord_order_items
WHEN NEW.kind = 'add_on' AND NEW.state = 'accepted'
  AND (NEW.authority_reference IS NULL OR trim(NEW.authority_reference) = '')
BEGIN SELECT RAISE(ABORT, 'accepted add-on requires authority evidence'); END;

CREATE TRIGGER ord_add_on_authority_on_update BEFORE UPDATE ON ord_order_items
WHEN NEW.kind = 'add_on' AND NEW.state = 'accepted'
  AND (NEW.authority_reference IS NULL OR trim(NEW.authority_reference) = '')
BEGIN SELECT RAISE(ABORT, 'accepted add-on requires authority evidence'); END;

CREATE TRIGGER ord_billing_review_items_no_update BEFORE UPDATE ON ord_billing_review_items
BEGIN SELECT RAISE(ABORT, 'billing review items are append-only'); END;
CREATE TRIGGER ord_billing_review_items_no_delete BEFORE DELETE ON ord_billing_review_items
BEGIN SELECT RAISE(ABORT, 'billing review items are append-only'); END;

CREATE TRIGGER ord_standing_orders_no_update BEFORE UPDATE ON ord_standing_orders
BEGIN SELECT RAISE(ABORT, 'standing orders are append-only'); END;
CREATE TRIGGER ord_standing_orders_no_delete BEFORE DELETE ON ord_standing_orders
BEGIN SELECT RAISE(ABORT, 'standing orders are append-only'); END;
