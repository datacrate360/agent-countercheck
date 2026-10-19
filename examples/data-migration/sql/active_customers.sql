CREATE OR REPLACE VIEW active_customers AS
SELECT id, name, amount
FROM customers
WHERE status = 'active';
