use itertools::Itertools;
use std::fmt::Debug;

/*
 NOTE:
 This struct only exists because of the assumption that it is faster to use `Vec`s as `HashMap`s.
 That should generally be the case for fewer values, since hashing has some overhead.
 Some quick lookups online show that ~100 elements `HashMap`s can become viable which matches
 TF2's player limit. Since most games are played on 24 or 32 player servers, storing player related
 data in a HashMap means there are at most 24 or 32 entries, making `HashMap`s slower on average.
 Just so cache locality is better, the keys all live in their own `Vec`.
 If this turns out to be slower or not worth it for some reason, feel free to revert this change.
*/
pub struct KeyValueVec<K, V> {
    key: Vec<K>,
    value: Vec<V>,
}

impl<K: Clone, V: Clone> Clone for KeyValueVec<K, V> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            value: self.value.clone(),
        }
    }
}

impl<'a, K, V> KeyValueVec<K, V>
where
    K: 'a,
    K: std::cmp::PartialEq<K>,
{
    #[must_use]
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let index = self.get_index(&key);
        match index {
            Some(index) => {
                let present_value = unsafe { self.value.get_unchecked_mut(index) };
                Some(std::mem::replace(present_value, value))
            }
            None => {
                self.key.push(key);
                self.value.push(value);
                None
            }
        }
    }

    pub fn get(&'a self, key: &'a K) -> Option<&'a V> {
        let index = self.get_index(key)?;
        Some(unsafe { self.value.get_unchecked(index) })
    }

    pub fn get_mut(&'a mut self, key: &'a K) -> Option<&'a mut V> {
        let index = self.get_index(key)?;
        Some(unsafe { self.value.get_unchecked_mut(index) })
    }

    pub fn remove(&'a mut self, key: &'a K) -> Option<V> {
        let index = self.get_index(key)?;
        self.key.swap_remove(index);
        Some(self.value.swap_remove(index))
    }

    fn get_index(&'a self, key: &'a K) -> Option<usize> {
        self.key.iter().enumerate().find(|(_, k)| **k == *key).map(move |(i, _)| i)
    }
}

impl<'a, K, V> KeyValueVec<K, V>
where
    K: 'a,
    V: 'a,
{
    /// creates a new empty `KeyValueVec<K, V>` without allocating space
    pub fn new() -> Self {
        Self {
            key: Vec::new(),
            value: Vec::new(),
        }
    }

    /// iterates over the key-value pairs
    pub fn iter(&'a self) -> std::iter::Zip<std::slice::Iter<'a, K>, std::slice::Iter<'a, V>> {
        self.key.iter().zip(self.value.iter())
    }
}

impl<K, V> Default for KeyValueVec<K, V> {
    fn default() -> Self {
        Self {
            key: Vec::new(),
            value: Vec::new(),
        }
    }
}

impl<K: Debug, V: Debug> Debug for KeyValueVec<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{{}}}", self.iter().map(|(k, v)| format!("{:?}: {:?}", k, v)).join(", "))
    }
}
