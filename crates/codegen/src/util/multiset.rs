use std::{
    collections::{HashMap, hash_map::Entry},
    hash::{DefaultHasher, Hash, Hasher},
};

pub fn eq<T>(a: &[T], b: &[T]) -> bool
where
    T: Eq + Hash,
{
    if a.len() != b.len() {
        return false;
    }

    let mut counts = HashMap::new();
    for x in a {
        *counts.entry(x).or_insert(0) += 1;
    }

    for x in b {
        let entry = counts.entry(x);
        match entry {
            Entry::Occupied(mut e) => {
                if *e.get() == 0 {
                    return false;
                }
                *e.get_mut() -= 1;
            }
            Entry::Vacant(_) => return false,
        }
    }

    true
}

pub fn hash<T: Hash, H: Hasher>(items: &[T], state: &mut H) {
    let acc = items.iter().fold(0u64, |acc, item| {
        let mut h = DefaultHasher::new();
        item.hash(&mut h);
        acc ^ h.finish()
    });

    acc.hash(state);
    items.len().hash(state);
}

pub fn subset<T: PartialEq + Clone>(needle: &[T], haystack: &[T]) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }

    let mut remaining: Vec<T> = haystack.to_vec();
    for item in needle {
        match remaining.iter().position(|x| x == item) {
            Some(pos) => {
                remaining.swap_remove(pos);
            }
            None => return false,
        }
    }
    true
}
