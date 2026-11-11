use crate::indexmap::IndexMap;
use crate::indexmap::map::Slice as MapSlice;
use crate::indexmap::serde::cautious_capacity;
use crate::indexmap::set::Slice as SetSlice;
use core::fmt::{self, Formatter};
use core::hash::{BuildHasher, Hash};
use core::marker::PhantomData;
use serde_core::de::{Deserialize, Deserializer, SeqAccess, Visitor};
use serde_core::ser::{Serialize, Serializer};
impl<K, V> Serialize for MapSlice<K, V>
where
    K: Serialize,
    V: Serialize,
{
    fn serialize<T>(&self, serializer: T) -> Result<T::Ok, T::Error>
    where
        T: Serializer,
    {
        serializer.collect_seq(self)
    }
}
impl<T> Serialize for SetSlice<T>
where
    T: Serialize,
{
    fn serialize<Se>(&self, serializer: Se) -> Result<Se::Ok, Se::Error>
    where
        Se: Serializer,
    {
        serializer.collect_seq(self)
    }
}
pub fn serialize<K, V, S, T>(map: &IndexMap<K, V, S>, serializer: T) -> Result<T::Ok, T::Error>
where
    K: Serialize,
    V: Serialize,
    T: Serializer,
{
    serializer.collect_seq(map)
}
struct SeqVisitor<K, V, S>(PhantomData<(K, V, S)>);
impl<'de, K, V, S> Visitor<'de> for SeqVisitor<K, V, S>
where
    K: Deserialize<'de> + Eq + Hash,
    V: Deserialize<'de>,
    S: Default + BuildHasher,
{
    type Value = IndexMap<K, V, S>;
    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "a sequenced map")
    }
    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let capacity = cautious_capacity::<K, V>(seq.size_hint());
        let mut map = IndexMap::with_capacity_and_hasher(capacity, S::default());
        while let Some((key, value)) = seq.next_element()? {
            map.insert(key, value);
        }
        Ok(map)
    }
}
pub fn deserialize<'de, D, K, V, S>(deserializer: D) -> Result<IndexMap<K, V, S>, D::Error>
where
    D: Deserializer<'de>,
    K: Deserialize<'de> + Eq + Hash,
    V: Deserialize<'de>,
    S: Default + BuildHasher,
{
    deserializer.deserialize_seq(SeqVisitor(PhantomData))
}
