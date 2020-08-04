use std::cmp::Eq;
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;

type Result<T> = std::result::Result<Option<T>, Box<dyn std::error::Error>>;

pub trait Store<K, V> {
    fn new() -> Self;
    fn set(&mut self, key: K, val: V) -> Result<V>;
    fn get(&self, key: &K) -> Result<&V>;
}

#[derive(Debug)]
pub struct HashStore<K, V> {
    store: HashMap<K, V>,
}

impl<K, V> Store<K, V> for HashStore<K, V>
where
    K: Eq + PartialEq + Hash,
{
    fn new() -> Self {
        HashStore {
            store: HashMap::new(),
        }
    }

    fn get(&self, key: &K) -> Result<&V> {
        // If some constraints are not fulfilled, return an error
        match self.store.get(&key) {
            Some(val) => Ok(Some(val)),
            None => Ok(None),
        }
    }

    fn set(&mut self, key: K, val: V) -> Result<V> {
        // If some constraints are not fulfilled, return an error
        match self.store.insert(key, val) {
            Some(val) => Ok(Some(val)),
            None => Ok(None),
        }
    }
}

#[derive(Debug, Clone)]
struct OperationalError;

impl fmt::Display for OperationalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Operational Error occured")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_store() {
        let mut store: HashStore<String, u32> = Store::new();
        let _ = store.set("foo".to_string(), 5);
        assert_eq!(store.get(&"foo".to_string()).unwrap(), Some(&5));
        assert_eq!(store.get(&"bar".to_string()).unwrap(), None);
        assert_eq!(store.set("baz".to_string(), 7).unwrap(), None);
        assert_eq!(store.set("foo".to_string(), 8).unwrap(), Some(5));
    }
}