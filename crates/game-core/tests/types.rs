//! Seats, seat sets, pending and game IDs.

use libretable_game_core::{GameId, MAX_SEATS, Observer, Pending, Seat, SeatCount, SeatSet};

fn seat(i: u8) -> Seat {
    Seat::new(i).unwrap()
}

#[test]
fn seat_and_count_bounds() {
    assert!(Seat::new(MAX_SEATS - 1).is_some());
    assert!(Seat::new(MAX_SEATS).is_none());
    assert!(SeatCount::new(0).is_none());
    assert!(SeatCount::new(MAX_SEATS).is_some());
    assert!(SeatCount::new(MAX_SEATS + 1).is_none());
}

#[test]
fn seat_count_lists_and_contains_its_seats() {
    let count = SeatCount::new(3).unwrap();
    assert_eq!(
        count.seats().map(Seat::index).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert!(count.contains(seat(2)));
    assert!(!count.contains(seat(3)));
    assert_eq!(count.all().len(), 3);
    assert_eq!(SeatCount::new(MAX_SEATS).unwrap().all().len(), 64);
}

#[test]
fn seat_set_operations() {
    let set: SeatSet = [seat(5), seat(0), seat(63)].into_iter().collect();
    assert_eq!(set.iter().map(Seat::index).collect::<Vec<_>>(), [0, 5, 63]);
    assert!(set.contains(seat(63)));
    assert_eq!(set.without(seat(5)).len(), 2);
    assert!(SeatSet::EMPTY.is_empty());
    assert_eq!(
        set.intersection(SeatSet::single(seat(5))),
        SeatSet::single(seat(5))
    );
    assert_eq!(
        SeatSet::EMPTY
            .with(seat(1))
            .union(SeatSet::single(seat(2)))
            .len(),
        2
    );
}

#[test]
fn pending_seats() {
    let set = SeatSet::single(seat(1));
    assert!(Pending::All(set).allows(seat(1)));
    assert!(Pending::Any(set).allows(seat(1)));
    assert!(!Pending::All(set).allows(seat(0)));
    assert!(Pending::Chance.seats().is_empty());
    assert!(!Pending::Finished.allows(seat(1)));
}

#[test]
fn serde_rejects_out_of_range_seats() {
    assert_eq!(serde_json::to_string(&seat(3)).unwrap(), "3");
    assert!(serde_json::from_str::<Seat>("64").is_err());
    assert!(serde_json::from_str::<SeatCount>("0").is_err());
    let observer = Observer::Seat(seat(2));
    let json = serde_json::to_string(&observer).unwrap();
    assert_eq!(serde_json::from_str::<Observer>(&json).unwrap(), observer);
}

#[test]
fn game_ids() {
    assert_eq!(GameId::new("mau-mau").as_str(), "mau-mau");
    assert_eq!(GameId::new("connect4").to_string(), "connect4");
}

#[test]
fn invalid_game_ids_panic() {
    for id in ["", "Mau-Mau", "mau--mau", "-mau", "mau-", "mau mau", "maü"] {
        assert!(
            std::panic::catch_unwind(|| GameId::new(id)).is_err(),
            "{id:?} was accepted"
        );
    }
}

#[cfg(feature = "dev")]
#[test]
fn omniscient_exists_only_in_dev() {
    assert_ne!(Observer::Omniscient, Observer::Spectator);
}
