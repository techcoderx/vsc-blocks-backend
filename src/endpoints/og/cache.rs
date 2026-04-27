use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{ Duration, Instant };
use super::template::MetaTags;

pub struct TtlCache {
  store: RwLock<HashMap<String, Entry>>,
  insertion_order: RwLock<Vec<String>>,
  ttl: Duration,
  max_entries: usize,
}

struct Entry {
  value: MetaTags,
  expires_at: Instant,
}

impl TtlCache {
  pub fn new(ttl: Duration, max_entries: usize) -> Self {
    Self {
      store: RwLock::new(HashMap::new()),
      insertion_order: RwLock::new(Vec::new()),
      ttl,
      max_entries,
    }
  }

  pub fn get(&self, key: &str) -> Option<MetaTags> {
    {
      let store = self.store.read().ok()?;
      if let Some(entry) = store.get(key) {
        if entry.expires_at > Instant::now() {
          return Some(entry.value.clone());
        }
      } else {
        return None;
      }
    }
    // expired — drop it
    let mut store = self.store.write().ok()?;
    let mut order = self.insertion_order.write().ok()?;
    store.remove(key);
    order.retain(|k| k != key);
    None
  }

  pub fn set(&self, key: String, value: MetaTags) {
    let mut store = match self.store.write() {
      Ok(s) => s,
      Err(_) => {
        return;
      }
    };
    let mut order = match self.insertion_order.write() {
      Ok(o) => o,
      Err(_) => {
        return;
      }
    };
    while store.len() >= self.max_entries && !order.is_empty() {
      let oldest = order.remove(0);
      store.remove(&oldest);
    }
    if !store.contains_key(&key) {
      order.push(key.clone());
    }
    store.insert(key, Entry { value, expires_at: Instant::now() + self.ttl });
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn sample() -> MetaTags {
    MetaTags {
      title: "t".into(),
      description: "d".into(),
      canonical: "c".into(),
      og_type: "website".into(),
      image: "i".into(),
      noindex: false,
    }
  }

  #[test]
  fn get_set() {
    let cache = TtlCache::new(Duration::from_secs(60), 10);
    cache.set("k".into(), sample());
    assert!(cache.get("k").is_some());
    assert!(cache.get("missing").is_none());
  }

  #[test]
  fn expires() {
    let cache = TtlCache::new(Duration::from_millis(10), 10);
    cache.set("k".into(), sample());
    std::thread::sleep(Duration::from_millis(30));
    assert!(cache.get("k").is_none());
  }

  #[test]
  fn fifo_evicts() {
    let cache = TtlCache::new(Duration::from_secs(60), 2);
    cache.set("a".into(), sample());
    cache.set("b".into(), sample());
    cache.set("c".into(), sample());
    assert!(cache.get("a").is_none());
    assert!(cache.get("b").is_some());
    assert!(cache.get("c").is_some());
  }
}
