use libretable_cards::{Card, MESSAGE_IDS, Rank, Suit};

#[test]
fn every_card_has_its_own_key() {
    let mut keys = Vec::new();
    for suit in Suit::ALL {
        for rank in Rank::ALL {
            let key = Card::new(suit, rank).key();
            assert_eq!(key, format!("card-{}-of-{}", rank.key(), suit.key()));
            keys.push(key);
        }
    }
    assert_eq!(keys, MESSAGE_IDS);
}

#[test]
fn red_suits() {
    let red: Vec<Suit> = Suit::ALL.into_iter().filter(|s| s.is_red()).collect();
    assert_eq!(red, [Suit::Hearts, Suit::Diamonds]);
}
