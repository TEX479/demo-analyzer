use itertools::Itertools;
use std::{borrow::Borrow, fmt::Debug};

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

impl<'a, K, V> KeyValueVec<K, V>
where
    K: 'a,
    V: 'a,
{
    /// creates a new empty `KeyValueVec<K, V>` without allocating space
    ///
    /// # Example
    /// Not sure why you'd need an example on how to use `new()`, but here ya go
    /// ```
    /// # use analysis_template::util::helpers::KeyValueVec;
    /// let kv = KeyValueVec::<String, u32>::new();
    /// ```
    pub fn new() -> Self {
        Self {
            key: Vec::new(),
            value: Vec::new(),
        }
    }

    /// how many key-value pairs exist
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::KeyValueVec;
    /// let mut kv = KeyValueVec::new();
    /// assert_eq!(kv.len(), 0);
    ///
    /// for i in 1..=5 {
    ///     kv.insert(format!("Element {}", i), i * i);
    ///     assert_eq!(kv.len(), i);
    /// }
    ///
    /// for i in (1..=5).rev() {
    ///     assert_eq!(kv.len(), i);
    ///     kv.remove(&format!("Element {}", i));
    /// }
    /// assert_eq!(kv.len(), 0);
    /// ```
    pub fn len(&'a self) -> usize {
        self.key.len()
    }

    /// returns true if there are no key-value pairs stored
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::KeyValueVec;
    /// let mut kv = KeyValueVec::new();
    /// assert!(kv.is_empty());
    /// kv.insert("Apple", 42);
    /// assert!(!kv.is_empty());
    /// kv.remove("Apple");
    /// assert!(kv.is_empty());
    /// ```
    pub fn is_empty(&'a self) -> bool {
        self.key.is_empty()
    }

    /// iterates over the key-value pairs
    ///
    /// TODO: add exmaple
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::KeyValueVec;
    /// ```
    pub fn iter(&'a self) -> std::iter::Zip<std::slice::Iter<'a, K>, std::slice::Iter<'a, V>> {
        self.key.iter().zip(self.value.iter())
    }

    /// iterates over the key-value-pairs
    ///
    /// the keys can not be provided as mutable, since changing them to equal another key is undefined behaviour
    /// TODO: add exmaple
    pub fn iter_mut(&'a mut self) -> std::iter::Zip<std::slice::Iter<'a, K>, std::slice::IterMut<'a, V>> {
        self.key.iter().zip(self.value.iter_mut())
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// In other words, remove all pairs `(k, v)` for which `f(&k, &mut v)` returns `false`.
    /// The elements are visited in unsorted (and unspecified) order.
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::KeyValueVec;
    /// let mut kv: KeyValueVec<String, i32> = Default::default();
    /// kv.insert("Apple".to_owned(), 1);
    /// kv.insert("Melon".to_owned(), 3);
    /// kv.insert("Onion".to_owned(), -2);
    ///
    /// kv.retain(|key, value| *value >= 0);
    ///
    /// assert_eq!(kv.get("Apple"), Some(&1));
    /// assert_eq!(kv.get("Melon"), Some(&3));
    /// assert_eq!(kv.get("Onion"), None);
    /// ```
    pub fn retain<F>(&'a mut self, mut f: F)
    where
        F: FnMut(&K, &mut V) -> bool,
    {
        let mut i = 0;
        while let (Some(key), Some(value)) = (self.key.get(i), self.value.get_mut(i)) {
            let keep_value = f(key, value);
            if keep_value {
                i += 1;
            } else {
                self.key.swap_remove(i);
                self.value.swap_remove(i);
            }
        }
    }

    /// inserts a value at `key`
    ///
    /// If `key` is already present, this returns the `key`'s value
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::KeyValueVec;
    /// let mut kv = KeyValueVec::new();
    /// assert_eq!(kv.insert("some key", 42), None);
    /// assert_eq!(kv.insert("some key", 69), Some(42));
    /// ```
    #[must_use]
    pub fn insert(&mut self, key: K, value: V) -> Option<V>
    where
        K: Eq,
    {
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

    /// gets the index where a key (and value) lives at, based on the key
    fn get_index<Q>(&'a self, key: &Q) -> Option<usize>
    where
        K: Borrow<Q>,
        Q: ?Sized + Eq,
    {
        self.key.iter().enumerate().find(|(_, k)| (*k).borrow().eq(key)).map(move |(i, _)| i)
    }

    /// gets the value at `key` or insertes the default one
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::KeyValueVec;
    /// let mut kv: KeyValueVec<u32, i32> = Default::default();
    /// assert_eq!(kv.get(&1), None);
    /// let value_at_one = kv.get_mut_or_insert_default(1);
    /// *value_at_one = 42;
    /// assert_eq!(kv.get(&1), Some(&42));
    ///
    /// kv.insert(5, 68);
    /// assert_eq!(kv.get(&5), Some(&68));
    /// let value_at_five = kv.get_mut_or_insert_default(5);
    /// *value_at_five += 1;
    /// assert_eq!(kv.get(&5), Some(&69));
    /// ```
    #[must_use]
    pub fn get_mut_or_insert_default(&'a mut self, key: K) -> &'a mut V
    where
        K: Eq,
        V: Default,
    {
        if let Some(index) = self.get_index(&key) {
            unsafe { self.value.get_unchecked_mut(index) }
        } else {
            _ = self.insert(key, V::default());
            let value = self.value.last_mut();
            unsafe { value.unwrap_unchecked() }
        }
    }

    /// inserts the default `Value` at `key`
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::KeyValueVec;
    /// let mut kv: KeyValueVec<i32, i32> = Default::default();
    /// assert_eq!(kv.get(&42), None);
    /// kv.insert_default(42);
    /// assert_eq!(kv.get(&42), Some(&0));
    /// ```
    #[must_use]
    pub fn insert_default(&'a mut self, key: K) -> Option<V>
    where
        K: Eq,
        V: Default,
    {
        self.insert(key, V::default())
    }

    /// gets the `key`'s value
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::KeyValueVec;
    /// let mut kv = KeyValueVec::new();
    ///
    /// assert_eq!(kv.get("Apple"), None);
    /// kv.insert("Apple", 42);
    /// assert_eq!(kv.get("Apple"), Some(&42));
    /// ```
    pub fn get<Q>(&'a self, key: &Q) -> Option<&'a V>
    where
        Q: ?Sized + Eq,
        K: Borrow<Q>,
    {
        let index = self.get_index(key)?;
        Some(unsafe { self.value.get_unchecked(index) })
    }

    /// gets a mutable reference to the `key`'s value
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::KeyValueVec;
    /// let mut kv = KeyValueVec::new();
    /// assert_eq!(kv.get_mut("Apple"), None);
    /// kv.insert("Apple", 42);
    /// if let Some(value_ref) = kv.get_mut("Apple") {
    ///     *value_ref += 27;
    /// }
    /// assert_eq!(kv.get_mut("Apple"), Some(&mut 69));
    /// ```
    pub fn get_mut<Q>(&'a mut self, key: &Q) -> Option<&'a mut V>
    where
        Q: ?Sized + Eq,
        K: Borrow<Q>,
    {
        let index = self.get_index(key)?;
        Some(unsafe { self.value.get_unchecked_mut(index) })
    }

    /// removes a `key` and returns it's value if present
    ///
    /// # Example
    /// ```
    /// # use analysis_template::util::helpers::KeyValueVec;
    /// let mut kv = KeyValueVec::new();
    /// assert_eq!(kv.remove("Apple"), None);
    /// kv.insert("Apple", 42);
    /// assert_eq!(kv.remove("Apple"), Some(42));
    /// ```
    pub fn remove<Q>(&'a mut self, key: &Q) -> Option<V>
    where
        Q: ?Sized + Eq,
        K: Borrow<Q>,
    {
        let index = self.get_index(key)?;
        self.key.swap_remove(index);
        Some(self.value.swap_remove(index))
    }
}

impl<K: Debug, V: Debug> Debug for KeyValueVec<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{{}}}", self.iter().map(|(k, v)| format!("{:?}: {:?}", k, v)).join(", "))
    }
}

impl<K: Clone, V: Clone> Clone for KeyValueVec<K, V> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            value: self.value.clone(),
        }
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
