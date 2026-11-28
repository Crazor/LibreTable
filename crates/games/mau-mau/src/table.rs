//! Views and the table model.

use alloc::string::String;
use alloc::vec::Vec;

use libretable_cards::Suit;
use libretable_game_core::{
    Attribute, Component, ComponentId, ComponentKind, Face, Indicator, IndicatorKind,
    IndicatorValue, Layout, MAX_SEATS, Observer, Seat, SeatSet, TableModel, Visibility, Zone,
    ZoneId, ZoneKind,
};
use serde::{Deserialize, Serialize};

use crate::{CardIndex, Config, Phase, RoundResult, Scoring, State, card, suit_name};

/// The draw pile zone; hands are zones `0..seats`.
pub const DRAW_PILE: ZoneId = ZoneId(MAX_SEATS as u32);

/// The discard pile zone.
pub const DISCARD: ZoneId = ZoneId(MAX_SEATS as u32 + 1);

/// A hand as one observer sees it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HandView {
    /// The observer's own hand, sorted by card.
    Own(Vec<CardIndex>),
    /// Somebody else's hand: the opaque IDs of its face-down cards.
    Hidden(Vec<u32>),
}

/// Public facts about the turn in progress that the table does not show but
/// a match needs to go on: who called, whether a missed or false call is
/// open to be caught, passes and turns so far. Nothing here is hidden from
/// anyone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flow {
    /// The seat whose turn it is called in its current run of turns.
    pub called: bool,
    /// A play in the current run of turns left exactly one card.
    pub played_to_one: bool,
    /// The seat of the previous run of turns, and whether it called.
    pub previous: Option<(Seat, bool)>,
    /// The previous run of turns ended with a missed or false call that is
    /// still open to be caught.
    pub fault: bool,
    /// Calling is still possible this turn.
    pub call_open: bool,
    /// Catching is still possible this turn.
    pub catch_open: bool,
    /// Passes in a row.
    pub passes: u8,
    /// Turns completed in this round.
    pub turns: u32,
}

/// What one observer may know.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct View {
    /// The variant.
    pub config: Config,
    /// Every hand, by seat.
    pub hands: Vec<HandView>,
    /// Opaque IDs of the draw pile, bottom to top.
    pub draw: Vec<u32>,
    /// Opaque IDs of the discards below the top, bottom to top.
    pub buried: Vec<u32>,
    /// The top discard.
    pub top: CardIndex,
    /// Whose turn it is; `None` once the match is over.
    pub turn: Option<Seat>,
    /// Direction of play.
    pub clockwise: bool,
    /// What the seat whose turn it is must do.
    pub phase: Phase,
    /// Cards the seat whose turn it is must draw.
    pub penalty: u8,
    /// The wished suit.
    pub wish: Option<Suit>,
    /// Seats whose last card is announced: holding one card (`auto`), or
    /// having called in the current or previous run of turns (`manual`).
    pub last_card: SeatSet,
    /// Round being played, from 0.
    pub round: u8,
    /// Round wins or penalty points, by seat.
    pub scores: Vec<u32>,
    /// Results of the rounds played.
    pub results: Vec<RoundResult>,
    /// The card the observer just drew, while it decides to play or keep it.
    pub drawn: Option<CardIndex>,
    /// Public facts about the turn in progress.
    pub flow: Flow,
}

pub(crate) fn view(state: &State, observer: Observer) -> View {
    let hands = state
        .hands
        .iter()
        .zip((0..).map_while(Seat::new))
        .map(|(hand, owner)| {
            if observer.sees_private_of(owner) {
                let mut cards: Vec<CardIndex> = hand.iter().map(|slot| slot.card).collect();
                cards.sort_unstable();
                HandView::Own(cards)
            } else {
                HandView::Hidden(hand.iter().map(|slot| slot.id).collect())
            }
        })
        .collect();
    let buried = state.discard.len() - 1;
    let mut last_card = SeatSet::EMPTY;
    if state.manual() {
        if let Some((previous, true)) = state.previous {
            last_card = last_card.with(previous);
        }
        if state.called {
            last_card = last_card.with(state.turn);
        }
    } else {
        for (hand, owner) in state.hands.iter().zip((0..).map_while(Seat::new)) {
            if hand.len() == 1 {
                last_card = last_card.with(owner);
            }
        }
    }
    View {
        config: state.config.clone(),
        hands,
        draw: state.draw.iter().map(|slot| slot.id).collect(),
        buried: state.discard[..buried].iter().map(|slot| slot.id).collect(),
        top: state.top(),
        turn: (!state.finished()).then_some(state.turn),
        clockwise: state.clockwise,
        phase: state.phase,
        penalty: state.penalty,
        wish: state.wish,
        last_card,
        round: state.round,
        scores: state.scores.clone(),
        results: state.results.clone(),
        drawn: (state.phase == Phase::Drawn && observer.seat() == Some(state.turn))
            .then(|| state.hand(state.turn).last().map(|slot| slot.card))
            .flatten(),
        flow: Flow {
            called: state.called,
            played_to_one: state.played_to_one,
            previous: state.previous,
            fault: state.fault,
            call_open: state.call_open,
            catch_open: state.catch_open,
            passes: state.passes,
            turns: state.turns,
        },
    }
}

fn face_up(card_index: CardIndex) -> Component {
    let shown = card(card_index);
    Component {
        id: ComponentId(u32::from(card_index)),
        kind: ComponentKind::Card,
        face: Face::Shown {
            key: shown.key().into(),
            attributes: alloc::vec![
                Attribute::new("suit", shown.suit.key()),
                Attribute::new("rank", shown.rank.key()),
            ],
        },
        rotation: 0,
    }
}

fn face_down(id: u32) -> Component {
    Component {
        id: ComponentId(id),
        kind: ComponentKind::Card,
        face: Face::Hidden,
        rotation: 0,
    }
}

fn message(id: &str) -> IndicatorValue {
    IndicatorValue::Message(String::from(id))
}

fn indicator(
    kind: IndicatorKind,
    label: &str,
    owner: Option<Seat>,
    value: IndicatorValue,
) -> Indicator {
    Indicator {
        kind,
        label: label.into(),
        owner,
        value,
    }
}

pub(crate) fn table(view: &View) -> TableModel {
    let mut indicators = Vec::new();
    if let Some(turn) = view.turn {
        indicators.push(indicator(
            IndicatorKind::Turn,
            "mau-mau-indicator-turn",
            None,
            IndicatorValue::Seat(turn),
        ));
        let phase = match view.phase {
            Phase::Play if view.penalty > 0 => "mau-mau-phase-penalty",
            Phase::Play => "mau-mau-phase-play",
            Phase::Drawn => "mau-mau-phase-drawn",
            Phase::Skip => "mau-mau-phase-skip",
        };
        indicators.push(indicator(
            IndicatorKind::Phase,
            "mau-mau-indicator-phase",
            None,
            message(phase),
        ));
    }
    indicators.push(indicator(
        IndicatorKind::Phase,
        "mau-mau-indicator-direction",
        None,
        message(if view.clockwise {
            "mau-mau-direction-clockwise"
        } else {
            "mau-mau-direction-counterclockwise"
        }),
    ));
    if view.penalty > 0 {
        indicators.push(indicator(
            IndicatorKind::Counter,
            "mau-mau-indicator-penalty",
            None,
            IndicatorValue::Number(i64::from(view.penalty)),
        ));
    }
    if let Some(suit) = view.wish {
        indicators.push(indicator(
            IndicatorKind::Announcement,
            "mau-mau-indicator-wish",
            None,
            message(suit_name(suit)),
        ));
    }
    for seat in view.last_card.iter() {
        indicators.push(indicator(
            IndicatorKind::Announcement,
            "mau-mau-indicator-last-card",
            Some(seat),
            message("mau-mau-last-card"),
        ));
    }
    if view.config.rounds > 1 {
        let round = view.round.min(view.config.rounds - 1) + 1;
        indicators.push(indicator(
            IndicatorKind::Counter,
            "mau-mau-indicator-round",
            None,
            IndicatorValue::Number(i64::from(round)),
        ));
    }
    let score_label = match view.config.scoring {
        Scoring::WinLose => "mau-mau-indicator-wins",
        Scoring::CardCount => "mau-mau-indicator-points",
    };
    for (score, seat) in view.scores.iter().zip((0..).map_while(Seat::new)) {
        indicators.push(indicator(
            IndicatorKind::Score,
            score_label,
            Some(seat),
            IndicatorValue::Number(i64::from(*score)),
        ));
    }

    let mut discard: Vec<Component> = view.buried.iter().map(|&id| face_down(id)).collect();
    discard.push(face_up(view.top));
    let mut zones = alloc::vec![
        Zone {
            id: DISCARD,
            kind: ZoneKind::Discard,
            label: "mau-mau-zone-discard".into(),
            owner: None,
            visibility: Visibility::Public,
            layout: Layout::Stack,
            components: discard,
        },
        Zone {
            id: DRAW_PILE,
            kind: ZoneKind::DrawPile,
            label: "mau-mau-zone-draw-pile".into(),
            owner: None,
            visibility: Visibility::Hidden,
            layout: Layout::Stack,
            components: view.draw.iter().map(|&id| face_down(id)).collect(),
        },
    ];
    zones.extend(
        view.hands
            .iter()
            .zip((0..).map_while(Seat::new))
            .map(|(hand, owner)| Zone {
                id: ZoneId(u32::from(owner.index())),
                kind: ZoneKind::Hand,
                label: "mau-mau-zone-hand".into(),
                owner: Some(owner),
                visibility: Visibility::Owner,
                layout: Layout::Fan,
                components: match hand {
                    HandView::Own(cards) => cards.iter().map(|&c| face_up(c)).collect(),
                    HandView::Hidden(ids) => ids.iter().map(|&id| face_down(id)).collect(),
                },
            }),
    );
    TableModel { zones, indicators }
}
