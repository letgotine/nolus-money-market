use std::ops::Deref;

use sdk::{
    cosmwasm_std::{Addr, Order, Storage, Timestamp},
    cw_storage_plus::{Bound, Deque, Index, IndexList, IndexedMap as CwIndexedMap, MultiIndex},
};

use crate::AlarmError;

type TimeSeconds = u64;

fn as_seconds(from: Timestamp) -> TimeSeconds {
    from.seconds()
}

struct AlarmIndexes<'a> {
    alarms: MultiIndex<'a, TimeSeconds, TimeSeconds, Addr>,
}

impl<'a> IndexList<TimeSeconds> for AlarmIndexes<'a> {
    fn get_indexes(&self) -> Box<dyn Iterator<Item = &'_ dyn Index<TimeSeconds>> + '_> {
        let v: Vec<&dyn Index<TimeSeconds>> = vec![&self.alarms];

        Box::new(v.into_iter())
    }
}

fn indexed_map<'namespace>(
    namespace_alarms: &'namespace str,
    namespace_index: &'namespace str,
) -> IndexedMap<'namespace> {
    let indexes = AlarmIndexes {
        alarms: MultiIndex::new(|_, d| *d, namespace_alarms, namespace_index),
    };

    IndexedMap::new(namespace_alarms, indexes)
}

enum MaybeOwned<'r, T> {
    Borrowed(&'r T),
    Owned(T),
}

impl<'r, T> Deref for MaybeOwned<'r, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            &Self::Borrowed(value) => value,
            Self::Owned(value) => value,
        }
    }
}

type IndexedMap<'namespace> = CwIndexedMap<'namespace, Addr, TimeSeconds, AlarmIndexes<'namespace>>;

const ALARMS_IN_DELIVERY: Deque<'static, Addr> = Deque::new("in_delivery");

pub struct Alarms<'storage, 'map, 'namespace> {
    storage: &'storage dyn Storage,
    alarms: MaybeOwned<'map, IndexedMap<'namespace>>,
}

impl<'storage, 'namespace> Alarms<'storage, 'static, 'namespace> {
    pub fn new(
        storage: &'storage dyn Storage,
        namespace_alarms: &'namespace str,
        namespace_index: &'namespace str,
    ) -> Self {
        let alarms = MaybeOwned::Owned(indexed_map(namespace_alarms, namespace_index));

        Self { storage, alarms }
    }
}

impl<'storage, 'map, 'namespace> Alarms<'storage, 'map, 'namespace> {
    pub fn alarms_selection(
        &self,
        ctime: Timestamp,
    ) -> impl Iterator<Item = Result<(Addr, TimeSeconds), AlarmError>> + 'storage {
        self.alarms
            .idx
            .alarms
            .range(
                self.storage,
                None,
                Some(Bound::inclusive((as_seconds(ctime), Addr::unchecked("")))),
                Order::Ascending,
            )
            .map(|res| res.map_err(AlarmError::from))
    }
}

pub struct AlarmsMut<'storage, 'namespace> {
    storage: &'storage mut dyn Storage,
    alarms: IndexedMap<'namespace>,
}

impl<'storage, 'namespace> AlarmsMut<'storage, 'namespace> {
    pub fn new(
        storage: &'storage mut dyn Storage,
        namespace_alarms: &'namespace str,
        namespace_index: &'namespace str,
    ) -> Self {
        Self {
            storage,
            alarms: indexed_map(namespace_alarms, namespace_index),
        }
    }

    pub fn as_alarms<'r>(&'r self) -> Alarms<'r, 'r, 'namespace> {
        Alarms {
            storage: self.storage,
            alarms: MaybeOwned::Borrowed(&self.alarms),
        }
    }

    pub fn add(&mut self, subscriber: Addr, time: Timestamp) -> Result<(), AlarmError> {
        self.add_internal(subscriber, as_seconds(time))
    }

    pub fn remove(&mut self, subscriber: Addr) -> Result<(), AlarmError> {
        self.alarms.remove(self.storage, subscriber)?;

        Ok(())
    }

    pub fn out_for_delivery(&mut self, subscriber: Addr) -> Result<(), AlarmError> {
        self.alarms.remove(self.storage, subscriber.clone())?;

        ALARMS_IN_DELIVERY
            .push_back(self.storage, &subscriber)
            .map_err(Into::into)
    }

    pub fn last_delivered(&mut self) -> Result<(), AlarmError> {
        ALARMS_IN_DELIVERY
            .pop_front(self.storage)
            .map(|maybe_alarm: Option<Addr>| debug_assert!(maybe_alarm.is_some()))
            .map_err(Into::into)
    }

    pub fn last_failed(&mut self, now: Timestamp) -> Result<(), AlarmError> {
        ALARMS_IN_DELIVERY
            .pop_front(self.storage)
            .map_err(Into::into)
            .and_then(|maybe_alarm: Option<Addr>| {
                maybe_alarm.ok_or(AlarmError::ReplyOnEmptyAlarmQueue)
            })
            .and_then(|subscriber: Addr| self.add_internal(subscriber, as_seconds(now) - /* Minus one second, to ensure it can be run within the same block */ 1))
    }

    fn add_internal(&mut self, subscriber: Addr, time: TimeSeconds) -> Result<(), AlarmError> {
        self.alarms
            .save(self.storage, subscriber, &time)
            .map_err(Into::into)
    }
}

#[cfg(test)]
pub mod tests {
    use sdk::cosmwasm_std::testing;

    use super::*;

    fn query_alarms(
        storage: &dyn Storage,
        alarms: &Alarms<'_>,
        t_sec: TimeSeconds,
    ) -> Vec<(Addr, TimeSeconds)> {
        alarms
            .alarms_selection(storage, Timestamp::from_seconds(t_sec))
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn test_add() {
        let alarms = Alarms::new("alarms", "alarms_idx");
        let storage = &mut testing::mock_dependencies().storage;

        let t1 = Timestamp::from_seconds(1);
        let t2 = Timestamp::from_seconds(3);
        let addr1 = Addr::unchecked("addr1");
        let addr2 = Addr::unchecked("addr2");

        alarms.add(storage, addr1.clone(), t1).unwrap();

        assert_eq!(
            query_alarms(storage, &alarms, 10),
            vec![(addr1.clone(), as_seconds(t1))]
        );

        // single alarm per addr
        alarms.add(storage, addr1.clone(), t2).unwrap();

        assert_eq!(
            query_alarms(storage, &alarms, 10),
            vec![(addr1.clone(), as_seconds(t2))]
        );

        alarms.add(storage, addr2.clone(), t2).unwrap();

        assert_eq!(
            query_alarms(storage, &alarms, 10),
            vec![(addr1, as_seconds(t2)), (addr2, as_seconds(t2))]
        );
    }

    #[test]
    fn test_remove() {
        let alarms = Alarms::new("alarms", "alarms_idx");
        let storage = &mut testing::mock_dependencies().storage;

        let t1 = Timestamp::from_seconds(10);
        let t2 = Timestamp::from_seconds(20);
        let addr1 = Addr::unchecked("addr1");
        let addr2 = Addr::unchecked("addr2");

        alarms.add(storage, addr1.clone(), t1).unwrap();
        alarms.add(storage, addr2.clone(), t2).unwrap();

        assert_eq!(
            query_alarms(storage, &alarms, 30),
            vec![
                (addr1.clone(), as_seconds(t1)),
                (addr2.clone(), as_seconds(t2))
            ]
        );

        alarms.remove(storage, addr1).unwrap();
        assert_eq!(
            query_alarms(storage, &alarms, 30),
            vec![(addr2, as_seconds(t2))]
        );
    }

    #[test]
    fn test_selection() {
        let alarms = Alarms::new("alarms", "alarms_idx");
        let storage = &mut testing::mock_dependencies().storage;
        let t1 = Timestamp::from_seconds(1);
        let t2 = Timestamp::from_seconds(2);
        let t3_sec = 3;
        let t4 = Timestamp::from_seconds(4);
        let addr1 = Addr::unchecked("addr1");
        let addr2 = Addr::unchecked("addr2");
        let addr3 = Addr::unchecked("addr3");
        let addr4 = Addr::unchecked("addr4");

        // same timestamp
        alarms.add(storage, addr1.clone(), t1).unwrap();
        alarms.add(storage, addr2.clone(), t1).unwrap();
        // different timestamp
        alarms.add(storage, addr3.clone(), t2).unwrap();
        // rest
        alarms.add(storage, addr4, t4).unwrap();

        assert_eq!(
            query_alarms(storage, &alarms, t3_sec),
            vec![
                (addr1, as_seconds(t1)),
                (addr2, as_seconds(t1)),
                (addr3, as_seconds(t2))
            ]
        );
    }
}
