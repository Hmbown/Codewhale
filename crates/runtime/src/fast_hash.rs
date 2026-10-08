use std::collections::{HashMap, HashSet};

pub type FastHashMap<K, V> = HashMap<K, V, ahash::RandomState>;
pub type FastHashSet<T> = HashSet<T, ahash::RandomState>;
