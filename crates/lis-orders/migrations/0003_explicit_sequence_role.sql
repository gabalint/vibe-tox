-- Forward-only correction for REQ-RES-002: the ordering source supplies the
-- clinical sequence role. Historical requested items remain nullable and
-- cannot be accepted until re-entered; every new item must carry a role.

ALTER TABLE ord_order_items ADD COLUMN sequence_role TEXT
    CHECK (sequence_role IN ('first_line', 'confirmation'));

-- Accepted historical items already froze one role into all their components.
-- Copy that stored fact; never reconstruct it from the item kind.
UPDATE ord_order_items
SET sequence_role = (
    SELECT c.sequence_role FROM ord_order_components c
    WHERE c.item_id = ord_order_items.id
    ORDER BY c.catalog_component_position LIMIT 1
)
WHERE EXISTS (
    SELECT 1 FROM ord_order_components c WHERE c.item_id = ord_order_items.id
);

CREATE TRIGGER ord_order_items_sequence_role_required BEFORE INSERT ON ord_order_items
WHEN NEW.sequence_role IS NULL
BEGIN SELECT RAISE(ABORT, 'order items require an explicit sequence role'); END;

CREATE TRIGGER ord_order_items_sequence_role_immutable BEFORE UPDATE OF sequence_role ON ord_order_items
WHEN NEW.sequence_role IS NOT OLD.sequence_role
BEGIN SELECT RAISE(ABORT, 'order item sequence role is immutable'); END;
