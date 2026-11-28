//! French-suited playing cards shared by the card games: suits, ranks and
//! their face keys.
//!
//! A card's face key is also the Fluent message ID of its name
//! (`card-queen-of-hearts` = "Queen of Hearts" / "Herz-Dame"), so renderers
//! name any card face generically. The names are in `locales/*/cards.ftl`;
//! [`MESSAGE_IDS`] lists them. Card names are whole messages rather than
//! rank and suit combined, because languages compose them differently.
//!
//! The crate is `no_std` and holds no game rules.

#![no_std]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// A French suit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Suit {
    /// ♣
    Clubs,
    /// ♠
    Spades,
    /// ♥
    Hearts,
    /// ♦
    Diamonds,
}

impl Suit {
    /// All suits, in this order.
    pub const ALL: [Self; 4] = [Self::Clubs, Self::Spades, Self::Hearts, Self::Diamonds];

    /// Internal key, e.g. `"hearts"`; also the value of the `suit` attribute.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Clubs => "clubs",
            Self::Spades => "spades",
            Self::Hearts => "hearts",
            Self::Diamonds => "diamonds",
        }
    }

    /// Index in [`Self::ALL`].
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Whether the suit is red.
    #[must_use]
    pub const fn is_red(self) -> bool {
        matches!(self, Self::Hearts | Self::Diamonds)
    }
}

/// A rank, two to ace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Rank {
    /// 2
    Two,
    /// 3
    Three,
    /// 4
    Four,
    /// 5
    Five,
    /// 6
    Six,
    /// 7
    Seven,
    /// 8
    Eight,
    /// 9
    Nine,
    /// 10
    Ten,
    /// J
    Jack,
    /// Q
    Queen,
    /// K
    King,
    /// A
    Ace,
}

impl Rank {
    /// All ranks, two to ace.
    pub const ALL: [Self; 13] = [
        Self::Two,
        Self::Three,
        Self::Four,
        Self::Five,
        Self::Six,
        Self::Seven,
        Self::Eight,
        Self::Nine,
        Self::Ten,
        Self::Jack,
        Self::Queen,
        Self::King,
        Self::Ace,
    ];

    /// The ranks of a 32-card (piquet or Skat) deck: seven to ace.
    pub const PIQUET: [Self; 8] = [
        Self::Seven,
        Self::Eight,
        Self::Nine,
        Self::Ten,
        Self::Jack,
        Self::Queen,
        Self::King,
        Self::Ace,
    ];

    /// Internal key, e.g. `"queen"`; also the value of the `rank` attribute.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Two => "two",
            Self::Three => "three",
            Self::Four => "four",
            Self::Five => "five",
            Self::Six => "six",
            Self::Seven => "seven",
            Self::Eight => "eight",
            Self::Nine => "nine",
            Self::Ten => "ten",
            Self::Jack => "jack",
            Self::Queen => "queen",
            Self::King => "king",
            Self::Ace => "ace",
        }
    }
}

/// A card: a rank of a suit. Copies of a card in a multi-deck game are told
/// apart by the game, not here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Card {
    /// The suit.
    pub suit: Suit,
    /// The rank.
    pub rank: Rank,
}

impl Card {
    /// A card from suit and rank.
    #[must_use]
    pub const fn new(suit: Suit, rank: Rank) -> Self {
        Self { suit, rank }
    }

    /// Face key and message ID of the card's name, e.g.
    /// `"card-queen-of-hearts"`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        MESSAGE_IDS[self.suit.index() * Rank::ALL.len() + self.rank as usize]
    }
}

macro_rules! names {
    ($($suit:literal),*) => {
        /// The message ID of every card's name, suit by suit in [`Suit::ALL`]
        /// order, ranks two to ace. Messages: `locales/<locale>/cards.ftl`.
        pub const MESSAGE_IDS: &[&str] = &[$(
            concat!("card-two-of-", $suit),
            concat!("card-three-of-", $suit),
            concat!("card-four-of-", $suit),
            concat!("card-five-of-", $suit),
            concat!("card-six-of-", $suit),
            concat!("card-seven-of-", $suit),
            concat!("card-eight-of-", $suit),
            concat!("card-nine-of-", $suit),
            concat!("card-ten-of-", $suit),
            concat!("card-jack-of-", $suit),
            concat!("card-queen-of-", $suit),
            concat!("card-king-of-", $suit),
            concat!("card-ace-of-", $suit),
        )*];
    };
}

names!("clubs", "spades", "hearts", "diamonds");
