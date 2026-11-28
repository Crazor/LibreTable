//! The table model: a generic, game-independent description of what one
//! observer sees, rendered by every client (architecture: table model &
//! rendering).
//!
//! A game builds it from a view with [`Game::table`](crate::Game::table), so
//! it can never contain more than that view. All text is a Fluent message ID
//! or a semantic value; renderers localize.

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::Seat;

/// Identifies a component on the table.
///
/// A face-up component keeps its ID across moves, so renderers can animate it.
/// A face-down component carries an opaque ID that the game reissues when
/// hidden components are shuffled or moved between hidden zones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ComponentId(pub u32);

/// Identifies a zone on the table. Stable for the whole match.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ZoneId(pub u32);

/// Everything one observer sees, as zones of components plus indicators.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableModel {
    /// Zones in the game's preferred reading order.
    pub zones: Vec<Zone>,
    /// Scores, counters, turn and phase displays.
    pub indicators: Vec<Indicator>,
}

impl TableModel {
    /// The zone with `id`, if present.
    #[must_use]
    pub fn zone(&self, id: ZoneId) -> Option<&Zone> {
        self.zones.iter().find(|zone| zone.id == id)
    }

    /// The component with `id` and the zone holding it, if present.
    #[must_use]
    pub fn component(&self, id: ComponentId) -> Option<(&Zone, &Component)> {
        self.zones.iter().find_map(|zone| {
            zone.components
                .iter()
                .find(|component| component.id == id)
                .map(|component| (zone, component))
        })
    }
}

/// A place on the table that holds components: a pile, a hand, a trick.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Zone {
    /// Stable ID, also used by [`Choice::Zone`](crate::Choice::Zone).
    pub id: ZoneId,
    /// What the zone is; drives seat-relative placement.
    pub kind: ZoneKind,
    /// Fluent message ID naming the zone, e.g. `"zone-discard"`.
    pub label: String,
    /// The seat the zone belongs to; `None` for shared zones.
    pub owner: Option<Seat>,
    /// Who may see the faces in this zone. Informational: each component's
    /// [`Face`] is authoritative.
    pub visibility: Visibility,
    /// How components are arranged.
    pub layout: Layout,
    /// Components in order: bottom to top for stacks, left to right
    /// otherwise.
    pub components: Vec<Component>,
}

/// What a zone is. Renderers place zones seat-relative from kind and owner
/// ("my hand: bottom", "opponents: clockwise", "trick: centre").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ZoneKind {
    /// A face-down supply to draw from.
    DrawPile,
    /// A seat's hand.
    Hand,
    /// A discard pile.
    Discard,
    /// Cards played to the current trick.
    Trick,
    /// Shared play area (melds, layouts).
    Tableau,
    /// A seat's own board or play area.
    PlayerBoard,
    /// A score sheet.
    ScoreSheet,
    /// A game board.
    Board,
    /// Anything else; the label says what.
    Other,
}

/// Who may see the faces of a zone's components.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Visibility {
    /// Everybody.
    Public,
    /// Only the owner.
    Owner,
    /// Nobody (a draw pile).
    Hidden,
}

/// How a zone arranges its components.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Layout {
    /// On top of each other; only the top is fully visible.
    Stack,
    /// Overlapping fan, as a hand.
    Fan,
    /// Side by side.
    Row,
    /// Rows of `columns` cells, filled row by row.
    Grid {
        /// Cells per row.
        columns: u16,
    },
    /// Free placement by the renderer.
    Free,
}

/// A card, tile, piece, token, die or marker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    /// Stable or opaque ID (see [`ComponentId`]).
    pub id: ComponentId,
    /// What sort of component it is.
    pub kind: ComponentKind,
    /// What the observer sees of it.
    pub face: Face,
    /// Clockwise rotation in degrees (tile rotation, a turned card); 0 if
    /// irrelevant.
    pub rotation: u16,
}

/// Physical sort of a component.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComponentKind {
    /// A playing card.
    Card,
    /// A tile.
    Tile,
    /// A playing piece.
    Piece,
    /// A token or chip.
    Token,
    /// A die.
    Die,
    /// A marker on a track or board.
    Marker,
}

/// What an observer sees of a component.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Face {
    /// Face down: only the back is visible.
    Hidden,
    /// Face up.
    Shown {
        /// Internal identity used for assets, and the Fluent message ID of the
        /// component's name, e.g. `"card-queen-of-hearts"`. The key itself is
        /// never translated; renderers show the message.
        key: String,
        /// Semantic attributes for rendering and accessibility, e.g.
        /// `suit = hearts`, `rank = queen`.
        attributes: Vec<Attribute>,
    },
}

/// One semantic attribute of a face.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attribute {
    /// Attribute name, e.g. `"suit"`.
    pub name: String,
    /// Attribute value, e.g. `"hearts"`.
    pub value: String,
}

impl Attribute {
    /// An attribute from name and value.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// A score, counter, turn or phase display.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Indicator {
    /// What the indicator shows.
    pub kind: IndicatorKind,
    /// Fluent message ID naming it, e.g. `"indicator-score"`.
    pub label: String,
    /// The seat it belongs to; `None` for global indicators.
    pub owner: Option<Seat>,
    /// The displayed value.
    pub value: IndicatorValue,
}

/// What an indicator shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IndicatorKind {
    /// Points.
    Score,
    /// A count (cards left, rounds played).
    Counter,
    /// Money or chips.
    Money,
    /// Whose turn it is.
    Turn,
    /// The current phase of the game.
    Phase,
    /// A standing announcement (a wished suit, a declared contract).
    Announcement,
}

/// Value of an indicator.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IndicatorValue {
    /// A number.
    Number(i64),
    /// A seat.
    Seat(Seat),
    /// A Fluent message ID, e.g. `"mau-mau-wish-hearts"`.
    Message(String),
}
