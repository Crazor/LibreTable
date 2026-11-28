//! Mau-Mau, the shedding card game. The rules are specified in
//! `docs/games/mau-mau/rules.md`; this crate implements them.
//!
//! # Cards and IDs
//!
//! A card is a [`CardIndex`] `0..32 × decks`: `deck × 32 + suit × 8 +
//! rank`, suits in [`Suit::ALL`] order, ranks seven to ace. A face-up card
//! carries its index as component ID. Every face-down card carries an opaque
//! ID of at least [`FIRST_OPAQUE_ID`], drawn from a counter in the state and
//! never reused: a card that becomes hidden (drawn, covered on the discard
//! pile, reshuffled) gets a fresh one, and a hand's face-down IDs are all
//! reissued whenever the hand changes, so nobody can follow a card by its ID.
//!
//! # Moves
//!
//! Exactly one seat decides at a time. See [`Move`] for the moves and
//! [`Game::choices`] for their choice paths.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod config;
mod table;

use alloc::vec;
use alloc::vec::Vec;

use libretable_cards::{Card, Rank, Suit};
use libretable_game_core::{
    Choice, ComponentId, Determinize, Game, GameId, GameRng, IllegalMove, Observer, Outcome,
    Pending, Seat, SeatCount, SeatResult, SeatSet, TableModel,
};
use serde::{Deserialize, Serialize};

pub use config::{Announce, Config, DECK_SIZE, Scoring};
pub use table::{DISCARD, DRAW_PILE, Flow, HandView, View};

/// A card of the match: `deck × 32 + suit × 8 + rank` (ranks seven to ace).
pub type CardIndex = u8;

/// The smallest opaque component ID; face-up cards use their [`CardIndex`].
pub const FIRST_OPAQUE_ID: u32 = 64;

/// Turns per seat after which a round ends without a winner.
pub const TURN_CAP_PER_SEAT: u32 = 100;

/// The card with index `card`.
#[must_use]
pub const fn card(card: CardIndex) -> Card {
    Card::new(
        Suit::ALL[(card as usize / 8) % 4],
        Rank::PIQUET[card as usize % 8],
    )
}

/// The index of `card` in deck `deck` (0 or 1).
#[must_use]
pub const fn index(deck: u8, card: Card) -> CardIndex {
    let rank = card.rank as u8 - Rank::Seven as u8;
    deck * DECK_SIZE + card.suit as u8 * 8 + rank
}

/// One decision of the seat whose turn it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Move {
    /// Play a card from hand; a Jack names the wished suit, any other card
    /// none. Path: the card, then for a Jack `mau-mau-wish-<suit>`.
    Play {
        /// The card.
        card: CardIndex,
        /// The wished suit, for a Jack only.
        wish: Option<Suit>,
    },
    /// Draw one card, or the open draw penalty. Path: the draw pile.
    Draw,
    /// End the turn after a draw without playing the drawn card. Path:
    /// `mau-mau-keep`.
    Keep,
    /// Pass when nothing can be drawn. Path: `mau-mau-pass`.
    Pass,
    /// Accept being skipped (`eight_counter` only). Path:
    /// `mau-mau-accept-skip`.
    AcceptSkip,
    /// Call "last card" (`announce = manual`). Path: `mau-mau-last-card`.
    Call,
    /// Catch the previous player's missed or false call (`announce =
    /// manual`). Path: `mau-mau-catch`.
    Catch,
}

/// What the seat whose turn it is must do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// Play, draw (also an open penalty) or pass.
    Play,
    /// Play the card just drawn or keep it.
    Drawn,
    /// Counter the skip with an 8 or accept it (`eight_counter`).
    Skip,
}

/// A card in a pile or hand, with the opaque ID it shows face down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Slot {
    card: CardIndex,
    id: u32,
}

/// The result of one round.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoundResult {
    /// Who went out; `None` after a stalemate or the turn cap.
    pub winner: Option<Seat>,
    /// Cards left in each hand, by seat.
    pub cards_left: Vec<u8>,
}

/// The full state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    config: Config,
    seats: u8,
    next_id: u32,
    /// Bottom to top.
    draw: Vec<Slot>,
    /// Bottom to top. The top card is face up; its slot ID is unused.
    discard: Vec<Slot>,
    /// By seat; order of play, the card just drawn last.
    hands: Vec<Vec<Slot>>,
    /// Round being played, from 0; equals `config.rounds` at the end.
    round: u8,
    turn: Seat,
    clockwise: bool,
    phase: Phase,
    /// Cards the seat whose turn it is must draw (open 7s).
    penalty: u8,
    wish: Option<Suit>,
    /// The current seat called "last card" in its current run of turns.
    called: bool,
    /// A play in the current run of turns left exactly one card.
    played_to_one: bool,
    /// The seat of the previous run of turns, and whether it called.
    previous: Option<(Seat, bool)>,
    /// The previous run of turns ended with a missed or false call that has
    /// not been caught and whose catch window is still open.
    fault: bool,
    call_open: bool,
    catch_open: bool,
    /// Passes in a row.
    passes: u8,
    /// Turns completed in this round.
    turns: u32,
    /// Round wins (`win-lose`) or penalty points (`card-count`), by seat.
    scores: Vec<u32>,
    results: Vec<RoundResult>,
}

/// What happened in one step. Events are public: they show nothing that is
/// hidden from any seat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Event {
    /// A new round was dealt.
    Dealt {
        /// The round, from 0.
        round: u8,
        /// Who starts.
        starter: Seat,
        /// The start card.
        start: CardIndex,
    },
    /// A card was played.
    Played {
        /// Who played.
        seat: Seat,
        /// The card.
        card: CardIndex,
        /// The wished suit, after a Jack.
        wish: Option<Suit>,
    },
    /// A seat drew cards.
    Drew {
        /// Who drew.
        seat: Seat,
        /// How many.
        count: u8,
    },
    /// The discards below the top were shuffled into a new draw pile.
    Reshuffled {
        /// Cards in the new draw pile.
        count: u8,
    },
    /// A seat ended its turn after drawing.
    Kept(Seat),
    /// A seat passed.
    Passed(Seat),
    /// A seat was skipped by an 8.
    Skipped(Seat),
    /// The direction of play reversed.
    Reversed,
    /// A seat called "last card".
    Called(Seat),
    /// The game announced a seat's last card (`announce = auto`).
    LastCard(Seat),
    /// A seat tried to catch the previous player.
    Caught {
        /// Who caught.
        catcher: Seat,
        /// Whether there was a missed or false call to catch.
        correct: bool,
        /// Who drew the penalty.
        penalized: Seat,
    },
    /// A round ended.
    RoundEnded(RoundResult),
}

/// The game.
#[derive(Clone, Copy, Debug)]
pub struct MauMau;

/// Every Fluent message ID this game emits, its game ID first. Messages:
/// `locales/<locale>/mau-mau.ftl`, card names in `locales/<locale>/cards.ftl`.
pub const MESSAGE_IDS: &[&str] = &[
    "mau-mau",
    "mau-mau-invalid-decks",
    "mau-mau-invalid-hand-size",
    "mau-mau-invalid-penalty",
    "mau-mau-invalid-rounds",
    "mau-mau-too-few-seats",
    "mau-mau-too-many-seats",
    "mau-mau-deck-too-small",
    "mau-mau-not-your-turn",
    "mau-mau-not-allowed",
    "mau-mau-wish-clubs",
    "mau-mau-wish-spades",
    "mau-mau-wish-hearts",
    "mau-mau-wish-diamonds",
    "mau-mau-keep",
    "mau-mau-pass",
    "mau-mau-accept-skip",
    "mau-mau-last-card",
    "mau-mau-catch",
    "mau-mau-zone-draw-pile",
    "mau-mau-zone-discard",
    "mau-mau-zone-hand",
    "mau-mau-indicator-turn",
    "mau-mau-indicator-direction",
    "mau-mau-indicator-phase",
    "mau-mau-indicator-penalty",
    "mau-mau-indicator-wish",
    "mau-mau-indicator-last-card",
    "mau-mau-indicator-round",
    "mau-mau-indicator-wins",
    "mau-mau-indicator-points",
    "mau-mau-direction-clockwise",
    "mau-mau-direction-counterclockwise",
    "mau-mau-phase-play",
    "mau-mau-phase-drawn",
    "mau-mau-phase-skip",
    "mau-mau-phase-penalty",
    "mau-mau-suit-clubs",
    "mau-mau-suit-spades",
    "mau-mau-suit-hearts",
    "mau-mau-suit-diamonds",
    "card-seven-of-clubs",
    "card-eight-of-clubs",
    "card-nine-of-clubs",
    "card-ten-of-clubs",
    "card-jack-of-clubs",
    "card-queen-of-clubs",
    "card-king-of-clubs",
    "card-ace-of-clubs",
    "card-seven-of-spades",
    "card-eight-of-spades",
    "card-nine-of-spades",
    "card-ten-of-spades",
    "card-jack-of-spades",
    "card-queen-of-spades",
    "card-king-of-spades",
    "card-ace-of-spades",
    "card-seven-of-hearts",
    "card-eight-of-hearts",
    "card-nine-of-hearts",
    "card-ten-of-hearts",
    "card-jack-of-hearts",
    "card-queen-of-hearts",
    "card-king-of-hearts",
    "card-ace-of-hearts",
    "card-seven-of-diamonds",
    "card-eight-of-diamonds",
    "card-nine-of-diamonds",
    "card-ten-of-diamonds",
    "card-jack-of-diamonds",
    "card-queen-of-diamonds",
    "card-king-of-diamonds",
    "card-ace-of-diamonds",
];

/// Fluent message ID of `suit`'s name, shown as the wished suit.
#[must_use]
pub const fn suit_name(suit: Suit) -> &'static str {
    match suit {
        Suit::Clubs => "mau-mau-suit-clubs",
        Suit::Spades => "mau-mau-suit-spades",
        Suit::Hearts => "mau-mau-suit-hearts",
        Suit::Diamonds => "mau-mau-suit-diamonds",
    }
}

/// Fluent message ID of the option that wishes `suit`.
#[must_use]
pub const fn wish_option(suit: Suit) -> &'static str {
    match suit {
        Suit::Clubs => "mau-mau-wish-clubs",
        Suit::Spades => "mau-mau-wish-spades",
        Suit::Hearts => "mau-mau-wish-hearts",
        Suit::Diamonds => "mau-mau-wish-diamonds",
    }
}

const fn illegal(reason: &'static str) -> IllegalMove {
    IllegalMove { reason }
}

fn seat(index: u8) -> Seat {
    Seat::new(index).expect("seat index below the seat count")
}

/// What a play does to the turn order.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Next {
    Normal,
    Skip,
    Again,
}

impl State {
    /// The variant being played.
    #[must_use]
    pub const fn config(&self) -> &Config {
        &self.config
    }

    /// Whether the match is over.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.round >= self.config.rounds
    }

    /// The results of the rounds played so far.
    #[must_use]
    pub fn results(&self) -> &[RoundResult] {
        &self.results
    }

    fn manual(&self) -> bool {
        self.config.manual()
    }

    fn fresh_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn top(&self) -> CardIndex {
        self.discard
            .last()
            .expect("the discard pile is never empty")
            .card
    }

    fn hand(&self, seat: Seat) -> &[Slot] {
        &self.hands[usize::from(seat.index())]
    }

    fn hand_mut(&mut self, seat: Seat) -> &mut Vec<Slot> {
        &mut self.hands[usize::from(seat.index())]
    }

    /// Gives every card of `seat`'s hand a fresh opaque ID.
    fn reissue(&mut self, seat: Seat) {
        let count = self.hand(seat).len();
        for i in 0..count {
            let id = self.fresh_id();
            self.hand_mut(seat)[i].id = id;
        }
    }

    /// The seat `steps` places on in the direction of play.
    fn step(&self, from: Seat, steps: u8) -> Seat {
        let n = self.seats;
        let offset = if self.clockwise {
            steps % n
        } else {
            n - steps % n
        };
        seat((from.index() + offset) % n)
    }

    /// Cards that can be drawn: the draw pile plus the discards below the top.
    fn drawable(&self) -> usize {
        self.draw.len() + self.discard.len() - 1
    }

    /// Whether `card` may be played now (ignoring whether it is in hand).
    fn fits(&self, card_index: CardIndex) -> bool {
        let played = card(card_index);
        let top = card(self.top());
        match self.phase {
            Phase::Skip => return played.rank == Rank::Eight,
            Phase::Play if self.penalty > 0 => {
                return self.config.seven_stacking && played.rank == Rank::Seven;
            }
            _ => {}
        }
        if played.rank == Rank::Jack {
            return top.rank != Rank::Jack || self.config.jack_on_jack;
        }
        match self.wish {
            Some(suit) => played.suit == suit,
            // A Jack without a wish (the start card) lets anything follow.
            None if top.rank == Rank::Jack => true,
            None => played.suit == top.suit || played.rank == top.rank,
        }
    }

    /// The plays of `card`: one, or four wishes for a Jack.
    fn plays(card_index: CardIndex, moves: &mut Vec<Move>) {
        if card(card_index).rank == Rank::Jack {
            moves.extend(Suit::ALL.map(|suit| Move::Play {
                card: card_index,
                wish: Some(suit),
            }));
        } else {
            moves.push(Move::Play {
                card: card_index,
                wish: None,
            });
        }
    }

    /// Whether playing `card` from a hand of `left` cards is allowed by
    /// `jack_finish`.
    fn may_finish_with(&self, card_index: CardIndex, left: usize) -> bool {
        left > 1 || self.config.jack_finish || card(card_index).rank != Rank::Jack
    }

    fn legal_moves(&self, seat: Seat) -> Vec<Move> {
        if self.finished() || seat != self.turn {
            return Vec::new();
        }
        let hand = self.hand(seat);
        let mut moves = Vec::new();
        match self.phase {
            Phase::Drawn => {
                let drawn = hand.last().expect("a seat that drew holds a card").card;
                if self.fits(drawn) {
                    Self::plays(drawn, &mut moves);
                }
                moves.push(Move::Keep);
            }
            Phase::Play | Phase::Skip => {
                let mut cards: Vec<CardIndex> = hand
                    .iter()
                    .map(|slot| slot.card)
                    .filter(|&c| self.fits(c) && self.may_finish_with(c, hand.len()))
                    .collect();
                cards.sort_unstable();
                for c in cards {
                    Self::plays(c, &mut moves);
                }
                if self.phase == Phase::Skip {
                    moves.push(Move::AcceptSkip);
                } else if self.drawable() > 0 {
                    moves.push(Move::Draw);
                } else {
                    moves.push(Move::Pass);
                }
            }
        }
        if self.call_open {
            moves.push(Move::Call);
        }
        if self.catch_open {
            moves.push(Move::Catch);
        }
        moves
    }

    /// Shuffles the discards below the top into a new draw pile.
    fn reshuffle(&mut self, rng: &mut GameRng, events: &mut Vec<Event>) {
        let top = self.discard.pop().expect("the discard pile is never empty");
        let mut cards: Vec<CardIndex> = self.discard.drain(..).map(|slot| slot.card).collect();
        self.discard.push(top);
        rng.shuffle(&mut cards);
        for card in cards {
            let id = self.fresh_id();
            self.draw.push(Slot { card, id });
        }
        events.push(Event::Reshuffled {
            count: u8::try_from(self.draw.len()).unwrap_or(u8::MAX),
        });
    }

    /// `seat` draws up to `count` cards, reshuffling when needed. Returns how
    /// many it drew.
    fn draw_cards(
        &mut self,
        seat: Seat,
        count: u8,
        rng: &mut GameRng,
        events: &mut Vec<Event>,
    ) -> u8 {
        let mut drawn = 0;
        while drawn < count {
            if self.draw.is_empty() && self.discard.len() > 1 {
                self.reshuffle(rng, events);
            }
            let Some(slot) = self.draw.pop() else { break };
            self.hand_mut(seat).push(slot);
            drawn += 1;
        }
        if drawn > 0 {
            self.reissue(seat);
            self.passes = 0;
        }
        events.push(Event::Drew { seat, count: drawn });
        drawn
    }

    /// Deals round `self.round`.
    fn deal(&mut self, rng: &mut GameRng, events: &mut Vec<Event>) {
        let mut cards: Vec<CardIndex> = (0..self.config.cards()).collect();
        rng.shuffle(&mut cards);
        self.draw.clear();
        self.discard.clear();
        for hand in &mut self.hands {
            hand.clear();
        }
        for _ in 0..self.config.hand_size {
            for s in 0..self.seats {
                let card = cards.pop().expect("validated: the deck covers the deal");
                self.hands[usize::from(s)].push(Slot { card, id: 0 });
            }
        }
        let start = cards
            .pop()
            .expect("validated: the deck covers the start card");
        self.discard.push(Slot { card: start, id: 0 });
        for card in cards {
            let id = self.fresh_id();
            self.draw.push(Slot { card, id });
        }
        for s in 0..self.seats {
            self.reissue(seat(s));
        }

        let starter = seat(self.round % self.seats);
        self.clockwise = true;
        self.phase = Phase::Play;
        self.penalty = 0;
        self.wish = None;
        self.called = false;
        self.played_to_one = false;
        self.previous = None;
        self.fault = false;
        self.catch_open = false;
        self.passes = 0;
        self.turns = 0;
        self.turn = starter;
        events.push(Event::Dealt {
            round: self.round,
            starter,
            start,
        });
        // The start card acts as if the starter's predecessor had played it.
        match card(start).rank {
            Rank::Seven => self.penalty = 2,
            Rank::Eight if self.config.eight_counter => self.phase = Phase::Skip,
            Rank::Eight => {
                events.push(Event::Skipped(starter));
                self.turn = self.step(starter, 1);
            }
            Rank::Nine if self.config.nine_reverses => {
                self.clockwise = false;
                events.push(Event::Reversed);
            }
            _ => {}
        }
        self.call_open = self.manual();
    }

    /// Closes the call and catch window: the seat plays, draws, passes or
    /// accepts a skip.
    fn act(&mut self) {
        self.call_open = false;
        self.catch_open = false;
        self.fault = false;
    }

    /// Ends the current seat's turn and starts the next one.
    fn end_turn(&mut self, next: Next, rng: &mut GameRng, events: &mut Vec<Event>) {
        self.turns += 1;
        if self.passes >= self.seats || self.turns >= TURN_CAP_PER_SEAT * u32::from(self.seats) {
            self.end_round(None, rng, events);
            return;
        }
        let current = self.turn;
        let (following, phase) = match next {
            Next::Again => (current, Phase::Play),
            Next::Skip if self.config.eight_counter => (self.step(current, 1), Phase::Skip),
            Next::Skip => {
                events.push(Event::Skipped(self.step(current, 1)));
                (self.step(current, 2), Phase::Play)
            }
            Next::Normal => (self.step(current, 1), Phase::Play),
        };
        self.phase = phase;
        if following != current {
            if self.manual() {
                self.fault = self.played_to_one != self.called;
                self.previous = Some((current, self.called));
                self.catch_open = true;
            }
            self.called = false;
            self.played_to_one = false;
        }
        self.call_open = self.manual() && !self.called;
        self.turn = following;
    }

    /// Ends the round; deals the next one unless the match is over.
    fn end_round(&mut self, winner: Option<Seat>, rng: &mut GameRng, events: &mut Vec<Event>) {
        let cards_left: Vec<u8> = self
            .hands
            .iter()
            .map(|hand| u8::try_from(hand.len()).unwrap_or(u8::MAX))
            .collect();
        match self.config.scoring {
            Scoring::WinLose => {
                if let Some(winner) = winner {
                    self.scores[usize::from(winner.index())] += 1;
                }
            }
            Scoring::CardCount => {
                for (score, &left) in self.scores.iter_mut().zip(&cards_left) {
                    *score += u32::from(left);
                }
            }
        }
        let result = RoundResult { winner, cards_left };
        self.results.push(result.clone());
        events.push(Event::RoundEnded(result));
        self.round += 1;
        self.call_open = false;
        self.catch_open = false;
        self.fault = false;
        if !self.finished() {
            self.deal(rng, events);
        }
    }

    fn play(
        &mut self,
        seat: Seat,
        card_index: CardIndex,
        wish: Option<Suit>,
        rng: &mut GameRng,
        events: &mut Vec<Event>,
    ) {
        self.act();
        let hand = self.hand_mut(seat);
        let at = hand
            .iter()
            .position(|slot| slot.card == card_index)
            .expect("a legal play is in hand");
        hand.remove(at);
        let left = hand.len();
        self.reissue(seat);
        let covered = self.fresh_id();
        if let Some(top) = self.discard.last_mut() {
            top.id = covered;
        }
        self.discard.push(Slot {
            card: card_index,
            id: 0,
        });
        self.passes = 0;
        self.wish = wish;
        events.push(Event::Played {
            seat,
            card: card_index,
            wish,
        });
        if left == 1 {
            self.played_to_one = true;
            if !self.manual() {
                events.push(Event::LastCard(seat));
            }
        }
        let rank = card(card_index).rank;
        let mut next = Next::Normal;
        match rank {
            Rank::Seven => {
                self.penalty = if self.config.seven_stacking {
                    self.penalty + 2
                } else {
                    2
                };
            }
            Rank::Eight => next = Next::Skip,
            Rank::Nine if self.config.nine_reverses => {
                self.clockwise = !self.clockwise;
                events.push(Event::Reversed);
            }
            Rank::Ace if self.config.ace_again => next = Next::Again,
            _ => {}
        }
        if left == 0 {
            if self.penalty > 0 && self.config.scoring == Scoring::CardCount {
                let victim = self.step(seat, 1);
                let penalty = self.penalty;
                self.draw_cards(victim, penalty, rng, events);
            }
            self.penalty = 0;
            self.end_round(Some(seat), rng, events);
            return;
        }
        self.phase = Phase::Play;
        self.end_turn(next, rng, events);
    }

    fn apply(
        &mut self,
        seat: Seat,
        mv: Move,
        rng: &mut GameRng,
    ) -> Result<Vec<Event>, IllegalMove> {
        if self.finished() || seat != self.turn {
            return Err(illegal("mau-mau-not-your-turn"));
        }
        if !self.legal_moves(seat).contains(&mv) {
            return Err(illegal("mau-mau-not-allowed"));
        }
        let mut events = Vec::new();
        match mv {
            Move::Play { card, wish } => self.play(seat, card, wish, rng, &mut events),
            Move::Draw => {
                self.act();
                if self.penalty > 0 {
                    let penalty = self.penalty;
                    self.penalty = 0;
                    self.draw_cards(seat, penalty, rng, &mut events);
                    self.end_turn(Next::Normal, rng, &mut events);
                } else {
                    self.draw_cards(seat, 1, rng, &mut events);
                    self.phase = Phase::Drawn;
                }
            }
            Move::Keep => {
                events.push(Event::Kept(seat));
                self.end_turn(Next::Normal, rng, &mut events);
            }
            Move::Pass => {
                self.act();
                self.penalty = 0;
                self.passes += 1;
                events.push(Event::Passed(seat));
                self.end_turn(Next::Normal, rng, &mut events);
            }
            Move::AcceptSkip => {
                self.act();
                events.push(Event::Skipped(seat));
                self.end_turn(Next::Normal, rng, &mut events);
            }
            Move::Call => {
                self.call_open = false;
                self.called = true;
                events.push(Event::Called(seat));
            }
            Move::Catch => {
                self.catch_open = false;
                let correct = self.fault;
                self.fault = false;
                let penalized = match self.previous {
                    Some((previous, _)) if correct => previous,
                    _ => seat,
                };
                events.push(Event::Caught {
                    catcher: seat,
                    correct,
                    penalized,
                });
                let penalty = self.config.mau_penalty;
                self.draw_cards(penalized, penalty, rng, &mut events);
            }
        }
        Ok(events)
    }
}

impl Game for MauMau {
    type Config = Config;
    type State = State;
    type Move = Move;
    type View = table::View;
    type Event = Event;

    const ID: GameId = GameId::new("mau-mau");

    fn setup(config: &Config, seats: SeatCount, rng: &mut GameRng) -> State {
        let n = seats.get();
        let mut state = State {
            config: config.clone(),
            seats: n,
            next_id: FIRST_OPAQUE_ID,
            draw: Vec::new(),
            discard: Vec::new(),
            hands: vec![Vec::new(); usize::from(n)],
            round: 0,
            turn: seat(0),
            clockwise: true,
            phase: Phase::Play,
            penalty: 0,
            wish: None,
            called: false,
            played_to_one: false,
            previous: None,
            fault: false,
            call_open: false,
            catch_open: false,
            passes: 0,
            turns: 0,
            scores: vec![0; usize::from(n)],
            results: Vec::new(),
        };
        // Setup events are not reported (docs/design/session.md).
        state.deal(rng, &mut Vec::new());
        state
    }

    fn pending(state: &State) -> Pending {
        if state.finished() {
            Pending::Finished
        } else {
            Pending::All(SeatSet::single(state.turn))
        }
    }

    fn legal_moves(state: &State, seat: Seat) -> Vec<Move> {
        state.legal_moves(seat)
    }

    fn apply(
        state: &mut State,
        seat: Seat,
        mv: &Move,
        rng: &mut GameRng,
    ) -> Result<Vec<Event>, IllegalMove> {
        state.apply(seat, *mv, rng)
    }

    fn view(state: &State, observer: Observer) -> table::View {
        table::view(state, observer)
    }

    fn table(view: &table::View) -> TableModel {
        table::table(view)
    }

    fn choices(_view: &table::View, mv: &Move) -> Vec<Choice> {
        let option = |id: &str| vec![Choice::Option(id.into())];
        match *mv {
            Move::Play { card, wish } => {
                let mut path = vec![Choice::Component(ComponentId(u32::from(card)))];
                if let Some(suit) = wish {
                    path.push(Choice::Option(wish_option(suit).into()));
                }
                path
            }
            Move::Draw => vec![Choice::Zone(DRAW_PILE)],
            Move::Keep => option("mau-mau-keep"),
            Move::Pass => option("mau-mau-pass"),
            Move::AcceptSkip => option("mau-mau-accept-skip"),
            Move::Call => option("mau-mau-last-card"),
            Move::Catch => option("mau-mau-catch"),
        }
    }

    fn outcome(state: &State) -> Option<Outcome> {
        if !state.finished() {
            return None;
        }
        let rank_by = |keys: &[u32]| -> Vec<SeatResult> {
            // Lower key ranks better.
            keys.iter()
                .zip(&state.scores)
                .map(|(key, score)| SeatResult {
                    rank: u8::try_from(keys.iter().filter(|other| *other < key).count() + 1)
                        .unwrap_or(u8::MAX),
                    score: i64::from(*score),
                })
                .collect()
        };
        let seats = match state.config.scoring {
            Scoring::CardCount => rank_by(&state.scores),
            Scoring::WinLose => match state.results.as_slice() {
                [only] if only.winner.is_none() => {
                    let left: Vec<u32> = only.cards_left.iter().map(|&c| u32::from(c)).collect();
                    rank_by(&left)
                }
                _ => {
                    let most = state.scores.iter().copied().max().unwrap_or(0);
                    let behind: Vec<u32> = state.scores.iter().map(|s| most - s).collect();
                    rank_by(&behind)
                }
            },
        };
        Some(Outcome {
            seats,
            teams: Vec::new(),
        })
    }
}

impl Determinize for MauMau {
    /// The cards the seat cannot see (the draw pile, the discards below the
    /// top, the other hands) are dealt uniformly at random from the cards
    /// not in its hand and not on top; everything else is in the view. The
    /// seat remembers nothing, so cards played earlier count as unseen.
    fn determinize(view: &View, seat: Seat, rng: &mut GameRng) -> State {
        let mut own: Vec<CardIndex> = view
            .hands
            .iter()
            .find_map(|hand| match hand {
                HandView::Own(cards) => Some(cards.clone()),
                HandView::Hidden(_) => None,
            })
            .unwrap_or_default();
        if let Some(drawn) = view.drawn {
            // The card just drawn is last in hand.
            if let Some(at) = own.iter().position(|&c| c == drawn) {
                own.remove(at);
                own.push(drawn);
            }
        }
        let mut unseen: Vec<CardIndex> = (0..view.config.cards())
            .filter(|c| *c != view.top && !own.contains(c))
            .collect();
        rng.shuffle(&mut unseen);
        let mut deal = |ids: &[u32]| -> Vec<Slot> {
            ids.iter()
                .map(|&id| Slot {
                    card: unseen.pop().expect("the view's counts match the deck"),
                    id,
                })
                .collect()
        };
        let draw = deal(&view.draw);
        let mut discard = deal(&view.buried);
        let mut hands: Vec<Vec<Slot>> = view
            .hands
            .iter()
            .map(|hand| match hand {
                HandView::Hidden(ids) => deal(ids),
                HandView::Own(_) => Vec::new(),
            })
            .collect();
        discard.push(Slot {
            card: view.top,
            id: 0,
        });
        let next_id = hands
            .iter()
            .flatten()
            .chain(&draw)
            .chain(&discard)
            .map(|slot| slot.id + 1)
            .max()
            .unwrap_or(0)
            .max(FIRST_OPAQUE_ID);
        let seats = u8::try_from(view.hands.len()).expect("at most 64 seats");
        let mut state = State {
            config: view.config.clone(),
            seats,
            next_id,
            draw,
            discard,
            hands: Vec::new(),
            round: view.round,
            turn: view.turn.unwrap_or(seat),
            clockwise: view.clockwise,
            phase: view.phase,
            penalty: view.penalty,
            wish: view.wish,
            called: view.flow.called,
            played_to_one: view.flow.played_to_one,
            previous: view.flow.previous,
            fault: view.flow.fault,
            call_open: view.flow.call_open,
            catch_open: view.flow.catch_open,
            passes: view.flow.passes,
            turns: view.flow.turns,
            scores: view.scores.clone(),
            results: view.results.clone(),
        };
        hands[usize::from(seat.index())] =
            own.into_iter().map(|card| Slot { card, id: 0 }).collect();
        state.hands = hands;
        state.reissue(seat);
        state
    }
}

/// Every well-formed move of a match of `config`, for the conformance
/// suite's rejection checks: every play of every card, plus the other moves.
#[must_use]
pub fn candidate_moves(config: &Config) -> Vec<Move> {
    let mut moves = Vec::new();
    for c in 0..config.cards() {
        State::plays(c, &mut moves);
    }
    moves.extend([
        Move::Draw,
        Move::Keep,
        Move::Pass,
        Move::AcceptSkip,
        Move::Call,
        Move::Catch,
    ]);
    moves
}

/// For the conformance suite's leak test: up to `count` states that differ
/// from `state` only in cards hidden from `seat`. The cards of the draw
/// pile, of the discards below the top and of every other hand are permuted
/// among those places; every place keeps its opaque ID.
#[must_use]
pub fn hidden_variants(state: &State, seat: Seat, rng: &mut GameRng, count: usize) -> Vec<State> {
    let own = usize::from(seat.index());
    (0..count)
        .map(|_| {
            let mut variant = state.clone();
            let buried = variant.discard.len() - 1;
            let mut places: Vec<&mut Slot> = variant
                .draw
                .iter_mut()
                .chain(variant.discard[..buried].iter_mut())
                .chain(
                    variant
                        .hands
                        .iter_mut()
                        .enumerate()
                        .filter(|(index, _)| *index != own)
                        .flat_map(|(_, hand)| hand.iter_mut()),
                )
                .collect();
            let mut cards: Vec<CardIndex> = places.iter().map(|slot| slot.card).collect();
            rng.shuffle(&mut cards);
            for (place, card) in places.iter_mut().zip(cards) {
                place.card = card;
            }
            variant
        })
        .collect()
}

/// Test support: replaces the cards of a state, so rule tests can set up
/// exact situations. The arguments are by seat for hands, bottom to top for
/// the piles; every card must appear once overall.
///
/// # Panics
///
/// If the cards do not form the match's deck exactly once.
#[doc(hidden)]
pub fn arrange(
    state: &mut State,
    hands: &[&[CardIndex]],
    discard: &[CardIndex],
    draw: &[CardIndex],
) {
    let mut all: Vec<CardIndex> = hands
        .iter()
        .flat_map(|hand| hand.iter().copied())
        .chain(discard.iter().copied())
        .chain(draw.iter().copied())
        .collect();
    all.sort_unstable();
    assert_eq!(
        all,
        (0..state.config.cards()).collect::<Vec<_>>(),
        "arranged cards must be the deck"
    );
    assert_eq!(hands.len(), state.hands.len(), "one hand per seat");
    assert!(!discard.is_empty(), "the discard pile needs a top card");
    let slot = |card| Slot { card, id: 0 };
    state.hands = hands
        .iter()
        .map(|hand| hand.iter().map(|&c| slot(c)).collect())
        .collect();
    state.discard = discard.iter().map(|&c| slot(c)).collect();
    state.draw = draw.iter().map(|&c| slot(c)).collect();
    for s in &mut state.draw {
        s.id = state.next_id;
        state.next_id += 1;
    }
    let buried = state.discard.len() - 1;
    for s in &mut state.discard[..buried] {
        s.id = state.next_id;
        state.next_id += 1;
    }
    for s in 0..state.seats {
        state.reissue(seat(s));
    }
}

/// Test support: sets whose turn it is and the turn state, as if the turn
/// had just begun with `top` effects already resolved.
#[doc(hidden)]
pub fn set_turn(state: &mut State, turn: Seat, penalty: u8, wish: Option<Suit>, phase: Phase) {
    state.turn = turn;
    state.penalty = penalty;
    state.wish = wish;
    state.phase = phase;
    state.call_open = state.manual() && !state.called;
}

/// Test support: sets the number of turns completed in this round.
#[doc(hidden)]
pub fn set_turns_played(state: &mut State, turns: u32) {
    state.turns = turns;
}
