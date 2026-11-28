//! Every entry of the registry can be visited under its own ID.

use libretable_registry::{GAMES, RegisteredGame, Visitor, find, visit};

struct Id;

impl Visitor for Id {
    type Output = &'static str;

    fn visit<G: RegisteredGame>(self) -> &'static str {
        G::ID.as_str()
    }
}

#[test]
fn every_entry_visits_its_own_game() {
    assert!(!GAMES.is_empty());
    for entry in GAMES {
        assert_eq!(visit(entry.id, Id), Some(entry.id));
        assert_eq!(find(entry.id), Some(entry));
        assert!(entry.message_ids.contains(&entry.id), "{}", entry.id);
    }
}

#[test]
fn entries_are_sorted_and_unique() {
    for pair in GAMES.windows(2) {
        assert!(pair[0].id < pair[1].id);
    }
}

#[test]
fn unknown_ids_are_none() {
    assert_eq!(visit("no-such-game", Id), None);
    assert_eq!(find("no-such-game"), None);
}

struct Searchable;

impl Visitor for Searchable {
    type Output = bool;

    fn visit<G: RegisteredGame>(self) -> bool {
        false
    }

    fn visit_searchable<G: RegisteredGame + libretable_game_core::Determinize>(self) -> bool {
        true
    }
}

#[test]
fn searchable_games_are_visited_as_such() {
    for entry in GAMES {
        assert_eq!(
            visit(entry.id, Searchable),
            Some(entry.searchable),
            "{}",
            entry.id
        );
    }
}
