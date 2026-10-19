CREATE OR REPLACE VIEW tenant_totals AS
SELECT tenant, SUM(NVL(amount, 0)) AS total   -- NVL is the old dialect; should be COALESCE
FROM customers
GROUP BY tenant;
