-- Bounded order-list paging before per-order counts and identity hydration
-- (REQ-OPS-004, NFR-05).

CREATE INDEX ord_orders_received_page
    ON ord_orders (received_at DESC, id DESC);

CREATE INDEX ord_orders_status_received_page
    ON ord_orders (status, received_at DESC, id DESC);

CREATE INDEX ord_orders_subject_received_page
    ON ord_orders (subject_id, received_at DESC, id DESC);
