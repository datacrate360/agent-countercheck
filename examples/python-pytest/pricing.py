def discount(price, pct):
    """Price after a percentage discount. Never below zero."""
    return price - price * pct / 100   # planted bug: no floor at zero
