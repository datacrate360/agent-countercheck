from pricing import discount


def test_half_off(price):
    assert discount(price, 50) == 5


def test_never_below_zero(price):
    assert discount(price, 150) == 0
