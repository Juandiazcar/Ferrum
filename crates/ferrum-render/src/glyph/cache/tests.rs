use super::*;

fn key(id: u16) -> GlyphKey {
    GlyphKey::Font {
        face: FaceId::EMBEDDED,
        id,
        size_q: 64 * 12,
        subpixel_q: 0,
        weight: 400,
        synth: Synthesis::default(),
        antialias: Antialias::Grayscale,
        fit: (14, 16),
    }
}

fn image(bytes: usize) -> GlyphImage {
    GlyphImage {
        left: 0,
        top: 0,
        width: 1,
        height: 1,
        stride: 1,
        advance: Some(7.0),
        pixels: Pixels::Mask(vec![0; bytes]),
    }
}

/// The bytes the cache's containers and pixels really take, from their capacities.
fn real_bytes(cache: &GlyphCache) -> usize {
    let pixels: usize = cache.slots.iter().map(|slot| slot.image.bytes()).sum();
    let slots =
        cache.slots.capacity() * size_of::<Slot>() + cache.free.capacity() * size_of::<u32>();
    let buckets = if cache.map.capacity() == 0 { 0 } else { map_buckets(cache.map.capacity()) };
    let map = buckets * size_of::<(GlyphKey, u32)>()
        + if buckets == 0 { 0 } else { buckets + MAP_GROUP_WIDTH };
    pixels + slots + map
}

#[test]
fn a_hit_returns_the_stored_image() {
    let mut cache = GlyphCache::new(1 << 20);
    let slot = cache.insert(key(1), image(10)).unwrap();
    assert_eq!(cache.lookup(&key(1)), Some(slot));
    assert_eq!(cache.image(slot).map(|image| image.advance), Some(Some(7.0)));
    assert_eq!(cache.lookup(&key(2)), None);
}

#[test]
fn the_least_recently_used_glyph_is_evicted_first() {
    let mut cache = GlyphCache::new(1 << 20);
    for id in 1..=3 {
        cache.insert(key(id), image(100)).unwrap();
    }
    let cap = cache.used_bytes();
    let mut cache = GlyphCache { capacity_bytes: cap, ..cache };
    assert!(cache.lookup(&key(1)).is_some(), "1 becomes the most recent");
    cache.insert(key(4), image(100)).unwrap();
    assert!(cache.lookup(&key(2)).is_none(), "2 was the least recent");
    assert!(cache.lookup(&key(1)).is_some() && cache.lookup(&key(3)).is_some());
    assert_eq!(cache.len(), 3);
}

#[test]
fn the_cap_bounds_pixels_and_containers() {
    for cap in [2_000, 10_000, 100_000] {
        let mut cache = GlyphCache::new(cap);
        for id in 0..3_000 {
            let _slot = cache.insert(key(id), image(usize::from(id % 97) + 1));
            assert_eq!(cache.used_bytes(), real_bytes(&cache), "the accounting is the real memory");
            assert!(cache.used_bytes() <= cap, "{} > {cap}", cache.used_bytes());
        }
        assert!(cache.len() > 0);
    }
}

#[test]
fn an_image_larger_than_the_cache_is_given_back() {
    let mut cache = GlyphCache::new(64);
    assert!(cache.insert(key(1), image(1_000)).is_err());
    assert_eq!(cache.len(), 0);
}

#[test]
fn entries_can_be_evicted_by_key() {
    let mut cache = GlyphCache::new(1 << 20);
    for id in 0..10 {
        cache.insert(key(id), image(8)).unwrap();
    }
    cache.evict_where(|key| matches!(key, GlyphKey::Font { id, .. } if id % 2 == 0));
    assert_eq!(cache.len(), 5);
    assert!(cache.lookup(&key(2)).is_none() && cache.lookup(&key(3)).is_some());
}
