use rust_decimal::Decimal;
use std::collections::BTreeMap;

/// In-memory orderbook maintaining bids and asks
/// Uses BTreeMap for ordered iteration
#[derive(Debug, Clone)]
pub struct Orderbook {
    // Asks: price -> qty (ascending order, lowest first)
    asks: BTreeMap<Decimal, Decimal>,
    // Bids: price -> qty (ascending order, but we iterate reverse for highest first)
    bids: BTreeMap<Decimal, Decimal>,
}

impl Orderbook {
    pub fn new() -> Self {
        Self {
            asks: BTreeMap::new(),
            bids: BTreeMap::new(),
        }
    }

    /// Apply a snapshot (replace all levels)
    pub fn apply_snapshot(&mut self, bids: Vec<(Decimal, Decimal)>, asks: Vec<(Decimal, Decimal)>) {
        self.bids.clear();
        self.asks.clear();
        
        for (price, qty) in bids {
            if qty > Decimal::ZERO {
                self.bids.insert(price, qty);
            }
        }
        
        for (price, qty) in asks {
            if qty > Decimal::ZERO {
                self.asks.insert(price, qty);
            }
        }
    }

    /// Apply updates (incremental changes)
    pub fn apply_updates(&mut self, bid_updates: Vec<(Decimal, Decimal)>, ask_updates: Vec<(Decimal, Decimal)>) {
        // Apply bid updates
        for (price, qty) in bid_updates {
            if qty == Decimal::ZERO {
                self.bids.remove(&price);
            } else {
                self.bids.insert(price, qty);
            }
        }
        
        // Apply ask updates
        for (price, qty) in ask_updates {
            if qty == Decimal::ZERO {
                self.asks.remove(&price);
            } else {
                self.asks.insert(price, qty);
            }
        }
    }

    /// Truncate to depth (keep best N levels)
    pub fn truncate(&mut self, depth: usize) {
        // Truncate asks: keep lowest (first) `depth` levels
        if self.asks.len() > depth {
            let keys_to_remove: Vec<Decimal> = self.asks
                .keys()
                .skip(depth)
                .cloned()
                .collect();
            for key in keys_to_remove {
                self.asks.remove(&key);
            }
        }
        
        // Truncate bids: keep highest (last) `depth` levels
        if self.bids.len() > depth {
            let keys_to_remove: Vec<Decimal> = self.bids
                .keys()
                .take(self.bids.len() - depth)
                .cloned()
                .collect();
            for key in keys_to_remove {
                self.bids.remove(&key);
            }
        }
    }

    /// Get best bid (highest)
    pub fn best_bid(&self) -> Option<(Decimal, Decimal)> {
        self.bids.iter().next_back().map(|(p, q)| (*p, *q))
    }

    /// Get best ask (lowest)
    pub fn best_ask(&self) -> Option<(Decimal, Decimal)> {
        self.asks.iter().next().map(|(p, q)| (*p, *q))
    }

    /// Get spread (best_ask - best_bid)
    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some((ask, _)), Some((bid, _))) => Some(ask - bid),
            _ => None,
        }
    }

    /// Get mid price ((best_ask + best_bid) / 2)
    pub fn mid(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some((ask, _)), Some((bid, _))) => Some((ask + bid) / Decimal::from(2)),
            _ => None,
        }
    }

    /// Iterate asks in ascending order (low to high)
    pub fn asks_iter(&self) -> impl Iterator<Item = (&Decimal, &Decimal)> {
        self.asks.iter()
    }

    /// Iterate bids in descending order (high to low)
    pub fn bids_iter_rev(&self) -> impl Iterator<Item = (&Decimal, &Decimal)> {
        self.bids.iter().rev()
    }

    /// Get all asks as vector (for API responses)
    pub fn asks_vec(&self, limit: Option<usize>) -> Vec<(Decimal, Decimal)> {
        let iter = self.asks.iter();
        if let Some(lim) = limit {
            iter.take(lim).map(|(p, q)| (*p, *q)).collect()
        } else {
            iter.map(|(p, q)| (*p, *q)).collect()
        }
    }

    /// Get all bids as vector (for API responses)
    pub fn bids_vec(&self, limit: Option<usize>) -> Vec<(Decimal, Decimal)> {
        let iter = self.bids.iter().rev();
        if let Some(lim) = limit {
            iter.take(lim).map(|(p, q)| (*p, *q)).collect()
        } else {
            iter.map(|(p, q)| (*p, *q)).collect()
        }
    }

    /// Get depth (number of levels)
    pub fn depth(&self) -> (usize, usize) {
        (self.asks.len(), self.bids.len())
    }

    // Helper methods for testing
    #[cfg(test)]
    pub fn update_bid(&mut self, price: Decimal, qty: Decimal) {
        if qty == Decimal::ZERO {
            self.bids.remove(&price);
        } else {
            self.bids.insert(price, qty);
        }
    }

    #[cfg(test)]
    pub fn update_ask(&mut self, price: Decimal, qty: Decimal) {
        if qty == Decimal::ZERO {
            self.asks.remove(&price);
        } else {
            self.asks.insert(price, qty);
        }
    }
}

impl Default for Orderbook {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_snapshot() {
        let mut book = Orderbook::new();
        book.apply_snapshot(
            vec![(dec!(100.0), dec!(1.0)), (dec!(99.0), dec!(2.0))],
            vec![(dec!(101.0), dec!(1.0)), (dec!(102.0), dec!(2.0))],
        );
        
        assert_eq!(book.best_bid(), Some((dec!(100.0), dec!(1.0))));
        assert_eq!(book.best_ask(), Some((dec!(101.0), dec!(1.0))));
    }

    #[test]
    fn test_update_remove() {
        let mut book = Orderbook::new();
        book.apply_snapshot(
            vec![(dec!(100.0), dec!(1.0))],
            vec![(dec!(101.0), dec!(1.0))],
        );
        
        // Remove a level
        book.apply_updates(
            vec![(dec!(100.0), dec!(0.0))],
            vec![],
        );
        
        assert_eq!(book.best_bid(), None);
    }

    #[test]
    fn test_truncate() {
        let mut book = Orderbook::new();
        let mut bids = Vec::new();
        let mut asks = Vec::new();
        
        for i in 0..20 {
            let bid_price = Decimal::from(100) - Decimal::from(i);
            let ask_price = Decimal::from(101) + Decimal::from(i);
            bids.push((bid_price, dec!(1.0)));
            asks.push((ask_price, dec!(1.0)));
        }
        
        book.apply_snapshot(bids, asks);
        book.truncate(10);
        
        assert_eq!(book.bids.len(), 10);
        assert_eq!(book.asks.len(), 10);
        
        // Best bid should be highest (100.0)
        assert_eq!(book.best_bid(), Some((dec!(100.0), dec!(1.0))));
        // Best ask should be lowest (101.0)
        assert_eq!(book.best_ask(), Some((dec!(101.0), dec!(1.0))));
    }
}

