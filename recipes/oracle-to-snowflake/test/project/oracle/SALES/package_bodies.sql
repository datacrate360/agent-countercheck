CREATE OR REPLACE PACKAGE BODY        "ORDER_API" AS
  PROCEDURE create_order(p_region VARCHAR2, p_amount NUMBER) IS
  BEGIN
    INSERT INTO orders VALUES (p_region, p_amount);
  END;
  PROCEDURE create_order(p_amount NUMBER) IS
  BEGIN
    create_order('EAST', p_amount);
  END;
END ORDER_API;
/
