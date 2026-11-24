//! Order placement and cancels as lists, like Hyperliquid's `order` and
//! `cancel` actions: [`PlaceOrders`], [`CancelOrders`] and
//! [`CancelClientOrders`] carry 1..=[`MAX_LIST_ENTRIES`] entries for one
//! account under one signature, and a single order is a list of one. Also the
//! list events that report every entry.

use core::fmt;
use core::marker::PhantomData;

use serde::de::{self, Deserializer, SeqAccess, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

use crate::types::{
    CancelClientOrder, CancelOrder, MarketId, OrderId, PlaceOrder, Side, TimeInForce, TriggerLimb,
    UnfilledReason,
};

/// Most entries one list action carries: an account's whole resting-order
/// allowance.
pub const MAX_LIST_ENTRIES: usize = 100;

/// The entries of a list action: 1..=[`MAX_LIST_ENTRIES`], refused at decode
/// otherwise.
#[derive(Clone, Debug)]
pub struct EntryList<T>(Vec<T>);

impl<T> EntryList<T> {
    pub fn one(entry: T) -> Self {
        Self(vec![entry])
    }

    pub fn as_slice(&self) -> &[T] {
        &self.0
    }
}

impl<T> TryFrom<Vec<T>> for EntryList<T> {
    type Error = ListSizeOutOfRange;

    fn try_from(entries: Vec<T>) -> Result<Self, Self::Error> {
        if entries.is_empty() || entries.len() > MAX_LIST_ENTRIES {
            return Err(ListSizeOutOfRange(entries.len()));
        }
        Ok(Self(entries))
    }
}

/// A list with no entries or more than [`MAX_LIST_ENTRIES`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListSizeOutOfRange(pub usize);

impl fmt::Display for ListSizeOutOfRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "list carries {} entries, must carry 1..={MAX_LIST_ENTRIES}",
            self.0
        )
    }
}

impl<T: Serialize> Serialize for EntryList<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for EntryList<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_seq(EntriesVisitor(PhantomData))
    }
}

/// Reads a bounded list, refusing it as soon as it runs past the bound.
struct EntriesVisitor<T>(PhantomData<T>);

impl<'de, T: Deserialize<'de>> Visitor<'de> for EntriesVisitor<T> {
    type Value = EntryList<T>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a list of 1..={MAX_LIST_ENTRIES} entries")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut entries = Vec::new();
        while let Some(entry) = seq.next_element()? {
            if entries.len() == MAX_LIST_ENTRIES {
                return Err(de::Error::custom(ListSizeOutOfRange(
                    MAX_LIST_ENTRIES.saturating_add(1),
                )));
            }
            entries.push(entry);
        }
        EntryList::try_from(entries).map_err(de::Error::custom)
    }
}

/// Place one or more limit orders for one account. Each order runs in turn as
/// a single order would, and stands or is refused on its own.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaceOrders {
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    pub orders: EntryList<OrderEntry>,
}

impl PlaceOrders {
    /// A list of one order.
    pub fn one(order: PlaceOrder) -> Self {
        Self {
            owner: order.owner,
            orders: EntryList::one(OrderEntry::from(order)),
        }
    }
}

/// One order of a [`PlaceOrders`]: every field of a [`PlaceOrder`] but the
/// owner, with the same defaults.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OrderEntry {
    pub market: MarketId,
    pub side: Side,
    pub price: u64,
    pub quantity: u64,
    pub client_order_id: Option<u64>,
    #[serde(default)]
    pub post_only: bool,
    #[serde(default)]
    pub reduce_only: bool,
    #[serde(default)]
    pub time_in_force: TimeInForce,
    #[serde(default)]
    pub stop_loss: Option<TriggerLimb>,
    #[serde(default)]
    pub take_profit: Option<TriggerLimb>,
}

impl OrderEntry {
    /// This order as `owner` places it.
    pub fn for_owner(&self, owner: [u8; 20]) -> PlaceOrder {
        PlaceOrder {
            market: self.market,
            owner,
            side: self.side,
            price: self.price,
            quantity: self.quantity,
            client_order_id: self.client_order_id,
            post_only: self.post_only,
            reduce_only: self.reduce_only,
            time_in_force: self.time_in_force,
            stop_loss: self.stop_loss.clone(),
            take_profit: self.take_profit.clone(),
        }
    }
}

impl From<PlaceOrder> for OrderEntry {
    fn from(order: PlaceOrder) -> Self {
        Self {
            market: order.market,
            side: order.side,
            price: order.price,
            quantity: order.quantity,
            client_order_id: order.client_order_id,
            post_only: order.post_only,
            reduce_only: order.reduce_only,
            time_in_force: order.time_in_force,
            stop_loss: order.stop_loss,
            take_profit: order.take_profit,
        }
    }
}

/// Cancel one or more resting orders of one account by order id. Each cancel
/// stands or is refused on its own.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CancelOrders {
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    pub order_ids: EntryList<OrderId>,
}

impl CancelOrders {
    /// A list of one cancel.
    pub fn one(cancel: CancelOrder) -> Self {
        Self {
            owner: cancel.owner,
            order_ids: EntryList::one(cancel.order_id),
        }
    }
}

/// Cancel one or more resting orders of one account by client order id. Each
/// cancel stands or is refused on its own.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CancelClientOrders {
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    pub client_order_ids: EntryList<u64>,
}

impl CancelClientOrders {
    /// A list of one cancel.
    pub fn one(cancel: CancelClientOrder) -> Self {
        Self {
            owner: cancel.owner,
            client_order_ids: EntryList::one(cancel.client_order_id),
        }
    }
}

/// How one order of a `PlaceOrders` ended, in `Event::OrdersPlaced`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderPlacement {
    pub market: MarketId,
    pub client_order_id: Option<u64>,
    pub outcome: PlacementOutcome,
}

/// What became of one order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlacementOutcome {
    /// Accepted, with `quantity` left resting on the book.
    Resting {
        order_id: OrderId,
        side: Side,
        price: u64,
        quantity: u64,
        time_in_force: TimeInForce,
        post_only: bool,
        reduce_only: bool,
    },
    /// Accepted and done without resting: filled in full, or an IOC whose
    /// remainder was dropped. `filled_quantity` may be 0; `unfilled` says why
    /// less than `requested_quantity` filled.
    Executed {
        order_id: OrderId,
        requested_quantity: u64,
        filled_quantity: u64,
        unfilled: Option<UnfilledReason>,
    },
    /// Refused for a trading reason; it wrote nothing. `code` is its
    /// `ExecError` code.
    Rejected { code: u32 },
}

/// One entry of `Event::OrdersCancelled`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderCancellation {
    /// The order left the book.
    Cancelled {
        order_id: OrderId,
        market: MarketId,
        client_order_id: Option<u64>,
        /// Quantity originally accepted onto the book.
        original_quantity: u64,
        /// Quantity still resting when the cancel occurred.
        remaining_quantity: u64,
        /// Maker quantity filled before the cancel.
        filled_quantity: u64,
    },
    /// A requested cancel that removed nothing; `code` is its `ExecError`
    /// code.
    Refused { target: CancelTarget, code: u32 },
}

/// The order a cancel named.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CancelTarget {
    OrderId(OrderId),
    ClientOrderId(u64),
}

/// Every order of one `PlaceOrders`, in order. On the ABCI surface, a JSON
/// array with one object of string values per order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OrderPlacements(pub Vec<OrderPlacement>);

/// The entries of one `OrdersCancelled`, in order. On the ABCI surface, a
/// JSON array with one object of string values per entry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OrderCancellations(pub Vec<OrderCancellation>);

/// An absent optional value renders empty, as optional attributes do.
struct OrEmpty<T>(Option<T>);

impl<T: fmt::Display> fmt::Display for OrEmpty<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Some(value) => write!(f, "{value}"),
            None => Ok(()),
        }
    }
}

/// Writes `entries` as a JSON array. Every value is an identifier or a
/// decimal number, so nothing needs escaping.
fn write_list<T>(
    f: &mut fmt::Formatter<'_>,
    entries: &[T],
    write: impl Fn(&mut fmt::Formatter<'_>, &T) -> fmt::Result,
) -> fmt::Result {
    f.write_str("[")?;
    for (index, entry) in entries.iter().enumerate() {
        if index > 0 {
            f.write_str(",")?;
        }
        write(f, entry)?;
    }
    f.write_str("]")
}

impl fmt::Display for OrderPlacements {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_list(f, &self.0, |f, placement| {
            let market = placement.market;
            let client_order_id = OrEmpty(placement.client_order_id);
            match &placement.outcome {
                PlacementOutcome::Resting {
                    order_id,
                    side,
                    price,
                    quantity,
                    time_in_force,
                    post_only,
                    reduce_only,
                } => write!(
                    f,
                    r#"{{"outcome":"resting","market":"{market}","client_order_id":"{client_order_id}","order_id":"{order_id}","side":"{side}","price":"{price}","quantity":"{quantity}","time_in_force":"{time_in_force}","post_only":"{post_only}","reduce_only":"{reduce_only}"}}"#
                ),
                PlacementOutcome::Executed {
                    order_id,
                    requested_quantity,
                    filled_quantity,
                    unfilled,
                } => {
                    let unfilled = OrEmpty(*unfilled);
                    write!(
                        f,
                        r#"{{"outcome":"executed","market":"{market}","client_order_id":"{client_order_id}","order_id":"{order_id}","requested_quantity":"{requested_quantity}","filled_quantity":"{filled_quantity}","unfilled":"{unfilled}"}}"#
                    )
                }
                PlacementOutcome::Rejected { code } => write!(
                    f,
                    r#"{{"outcome":"rejected","market":"{market}","client_order_id":"{client_order_id}","code":"{code}"}}"#
                ),
            }
        })
    }
}

impl fmt::Display for OrderCancellations {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_list(f, &self.0, |f, entry| match entry {
            OrderCancellation::Cancelled {
                order_id,
                market,
                client_order_id,
                original_quantity,
                remaining_quantity,
                filled_quantity,
            } => {
                let client_order_id = OrEmpty(*client_order_id);
                write!(
                    f,
                    r#"{{"outcome":"cancelled","order_id":"{order_id}","market":"{market}","client_order_id":"{client_order_id}","original_quantity":"{original_quantity}","remaining_quantity":"{remaining_quantity}","filled_quantity":"{filled_quantity}"}}"#
                )
            }
            OrderCancellation::Refused {
                target: CancelTarget::OrderId(order_id),
                code,
            } => write!(
                f,
                r#"{{"outcome":"refused","order_id":"{order_id}","code":"{code}"}}"#
            ),
            OrderCancellation::Refused {
                target: CancelTarget::ClientOrderId(client_order_id),
                code,
            } => write!(
                f,
                r#"{{"outcome":"refused","client_order_id":"{client_order_id}","code":"{code}"}}"#
            ),
        })
    }
}

#[cfg(test)]
#[path = "order_lists_tests.rs"]
mod tests;
