use super::*;

#[test]
fn checked_gap_is_per_reader_and_snapshot_has_exact_end() {
    let mut items = Items::new();
    let slow = items.watch_changes(1);
    let mut fast = items.watch_changes(2);
    items.prime(81, 1, (1.0, 64.0, 2.0), (0.0, 0.2, 0.0), 100_000, None);
    // Один TNT действительно пишет Fuse каждый такт: быстрый читатель
    // успевает, медленный теряет Spawn и начало Fuse, как в большой серии.
    for tick in 0..65_540 {
        assert!(items.burn_fuses(&|_, _| true).is_empty());
        if tick % 1000 == 0 {
            let batch = items.changes_since_checked(fast, 2);
            assert!(batch.dropped.is_none());
            fast = batch.end;
        }
    }
    let end = items.changes.count();
    let batch = items.changes_since_checked(slow, 1);
    assert_eq!(batch.end, end);
    assert_eq!(batch.changes.len(), 65_536);
    let snapshot = batch.dropped.unwrap();
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].fuse, Some(34_460));
    assert_eq!(snapshot[0].fuse, items.items()[0].fuse);
    assert!(items.changes_since_checked(fast, 2).dropped.is_none());
    // Снимок не забирает более поздние записи ни у одного читателя.
    items.burn_fuses(&|_, _| true);
    let next = items.changes_since_checked(end, 1);
    assert!(next.dropped.is_none());
    assert_eq!(next.end, end + 1);
    assert!(matches!(next.changes.as_slice(), [Change::Fuse { entity_id: 81, fuse: 34_460 }]));
    assert_eq!(snapshot[0].fuse, Some(34_460), "снимок не меняется вслед за миром");
}

#[test]
fn checked_normal_read_matches_legacy_order_and_cursor() {
    let mut items = Items::new();
    let from = items.watch_changes(1);
    items.watch_changes(2);
    items.prime(81, 1, (0.0, 64.0, 0.0), (0.0, 0.2, 0.0), 30, None);
    items.burn_fuses(&|_, _| true);
    let checked = items.changes_since_checked(from, 1);
    let (legacy, end) = items.changes_since(from, 2);
    assert!(checked.dropped.is_none());
    assert_eq!(checked.end, end);
    assert_eq!(format!("{:?}", checked.changes), format!("{legacy:?}"));
    assert!(items.changes_since_checked(end, 1).changes.is_empty());
}
