//! The selection protocol: building a move from choices, so that no client
//! can ever build an illegal move (architecture: interaction model).
//!
//! A game describes each legal move as an ordered list of [`Choice`]s with
//! [`Game::choices`](crate::Game::choices). [`Selection`] then drives the
//! protocol generically: given the choices made so far it returns the next
//! selectable ones, accepts a choice, steps back, and reports the move once
//! the choices made equal a legal move's whole path. [`check_selection`] is the conformance
//! check that every legal move is reachable and no choice path is illegal.

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::{
    ComponentId, Face, Game, GameRng, IllegalMove, Observer, Seat, TableModel, Visibility, ZoneId,
};

/// One step in composing a move.
///
/// Component and zone IDs refer to the [`TableModel`] of the choosing seat's
/// view; options are Fluent message IDs, so they can be shown as buttons.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Choice {
    /// Pick a component ("play the Jack of Clubs").
    Component(ComponentId),
    /// Pick a zone ("draw from the pile", "drop into column 3").
    Zone(ZoneId),
    /// Pick an option by its Fluent message ID ("wish Hearts", "pass").
    Option(String),
    /// Pick a number ("bid 18").
    Number(i64),
    /// Pick a set of components at once ("keep these dice").
    Subset(ComponentSet),
}

/// A set of components, kept sorted and free of duplicates so that equal
/// sets compare equal whatever order they were picked in.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "Vec<ComponentId>", into = "Vec<ComponentId>")]
pub struct ComponentSet(Vec<ComponentId>);

impl ComponentSet {
    /// The members in ascending order.
    #[must_use]
    pub fn as_slice(&self) -> &[ComponentId] {
        &self.0
    }

    /// Whether `id` is a member.
    #[must_use]
    pub fn contains(&self, id: ComponentId) -> bool {
        self.0.binary_search(&id).is_ok()
    }
}

impl FromIterator<ComponentId> for ComponentSet {
    fn from_iter<I: IntoIterator<Item = ComponentId>>(iter: I) -> Self {
        let mut ids: Vec<_> = iter.into_iter().collect();
        ids.sort_unstable();
        ids.dedup();
        Self(ids)
    }
}

impl TryFrom<Vec<ComponentId>> for ComponentSet {
    type Error = &'static str;

    /// Accepts only the canonical form (strictly ascending), so a set has
    /// exactly one encoding.
    fn try_from(ids: Vec<ComponentId>) -> Result<Self, Self::Error> {
        if ids.windows(2).all(|pair| pair[0] < pair[1]) {
            Ok(Self(ids))
        } else {
            Err("component set must be strictly ascending")
        }
    }
}

impl From<ComponentSet> for Vec<ComponentId> {
    fn from(set: ComponentSet) -> Self {
        set.0
    }
}

/// Why the choice paths of a seat's legal moves cannot drive a selection.
/// Indices refer to the list of legal moves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionError {
    /// Two moves have the same choice path, so the second is unreachable.
    Duplicate {
        /// Index of one move.
        first: usize,
        /// Index of the other move.
        second: usize,
    },
    /// One move's path is a proper prefix of another's, so the selection
    /// could not tell whether the shorter move is complete.
    Prefix {
        /// Index of the move with the shorter path.
        shorter: usize,
        /// Index of the move with the longer path.
        longer: usize,
    },
}

/// The chosen value is not selectable at this point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NotSelectable;

/// An ongoing selection: the legal moves of one seat, their choice paths and
/// the choices made so far.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection<M> {
    paths: Vec<(Vec<Choice>, M)>,
    made: Vec<Choice>,
}

impl<M> Selection<M> {
    /// Starts the selection for `seat` from the game's legal moves, with
    /// choice paths described against the seat's own view.
    ///
    /// # Errors
    ///
    /// [`SelectionError`] if two paths are equal or one is a prefix of
    /// another.
    pub fn new<G: Game<Move = M>>(state: &G::State, seat: Seat) -> Result<Self, SelectionError> {
        let view = G::view(state, Observer::Seat(seat));
        let paths = G::legal_moves(state, seat)
            .into_iter()
            .map(|mv| (G::choices(&view, &mv), mv))
            .collect();
        Self::from_paths(paths)
    }

    /// Starts a selection from explicit `(path, move)` pairs.
    ///
    /// # Errors
    ///
    /// [`SelectionError`] if two paths are equal or one is a prefix of
    /// another.
    pub fn from_paths(paths: Vec<(Vec<Choice>, M)>) -> Result<Self, SelectionError> {
        // In lexicographic order, a path that is a prefix of any other is a
        // prefix of its immediate successor.
        let mut order: Vec<usize> = (0..paths.len()).collect();
        order.sort_by(|&a, &b| paths[a].0.cmp(&paths[b].0));
        for pair in order.windows(2) {
            let (a, b) = (&paths[pair[0]].0, &paths[pair[1]].0);
            if a == b {
                let (first, second) = (pair[0].min(pair[1]), pair[0].max(pair[1]));
                return Err(SelectionError::Duplicate { first, second });
            }
            if b.starts_with(a) {
                return Err(SelectionError::Prefix {
                    shorter: pair[0],
                    longer: pair[1],
                });
            }
        }
        Ok(Self {
            paths,
            made: Vec::new(),
        })
    }

    /// The choices made so far.
    #[must_use]
    pub fn made(&self) -> &[Choice] {
        &self.made
    }

    /// Whether the seat has no legal move at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// The moves still possible after the choices made so far, in the order
    /// the game listed them.
    ///
    /// Session-internal: moves come from the full state and may carry hidden
    /// details, so clients receive only [`selectable`](Self::selectable).
    pub fn remaining(&self) -> impl Iterator<Item = &M> {
        self.matching().map(|(_, mv)| mv)
    }

    /// The choices that may be made next, without duplicates, in the order
    /// the game listed the moves. Empty once the selection is complete.
    #[must_use]
    pub fn selectable(&self) -> Vec<Choice> {
        let depth = self.made.len();
        let mut next: Vec<Choice> = Vec::new();
        for (path, _) in self.matching() {
            if let Some(choice) = path.get(depth) {
                if !next.contains(choice) {
                    next.push(choice.clone());
                }
            }
        }
        next
    }

    /// Makes the next choice.
    ///
    /// # Errors
    ///
    /// [`NotSelectable`] if `choice` is not among [`selectable`](Self::selectable);
    /// the selection is then unchanged.
    pub fn choose(&mut self, choice: Choice) -> Result<(), NotSelectable> {
        let depth = self.made.len();
        if self
            .matching()
            .any(|(path, _)| path.get(depth) == Some(&choice))
        {
            self.made.push(choice);
            Ok(())
        } else {
            Err(NotSelectable)
        }
    }

    /// Takes back the last choice ("undo choice").
    pub fn undo(&mut self) -> Option<Choice> {
        self.made.pop()
    }

    /// The selected move once the choices made equal its whole path. A
    /// single remaining move is not complete before that ("play Jack" still
    /// needs the wish).
    #[must_use]
    pub fn complete(&self) -> Option<&M> {
        self.complete_index().map(|index| &self.paths[index].1)
    }

    /// Consumes the selection and returns the selected move, if complete.
    #[must_use]
    pub fn into_move(mut self) -> Option<M> {
        let index = self.complete_index()?;
        Some(self.paths.swap_remove(index).1)
    }

    /// Paths are prefix-free, so a path equal to `made` is the only match.
    fn complete_index(&self) -> Option<usize> {
        self.paths.iter().position(|(path, _)| *path == self.made)
    }

    fn matching(&self) -> impl Iterator<Item = &(Vec<Choice>, M)> {
        self.paths
            .iter()
            .filter(|(path, _)| path.starts_with(&self.made))
    }
}

/// A breach of the selection protocol found by [`check_selection`]. Move
/// indices refer to [`Game::legal_moves`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionViolation {
    /// The choice paths are not unique and prefix-free.
    Ambiguous(SelectionError),
    /// A move's path names a component that is not in the seat's table.
    UnknownComponent {
        /// Index of the move.
        index: usize,
        /// The missing component.
        id: ComponentId,
    },
    /// A move's path contains a [`Choice::Subset`] with no members.
    EmptySubset {
        /// Index of the move.
        index: usize,
    },
    /// Two zones of the seat's table share an ID.
    DuplicateZone(ZoneId),
    /// Two components of the seat's table share an ID, so choices and
    /// animations naming it are ambiguous.
    DuplicateComponent(ComponentId),
    /// A component is shown face up in a zone the seat may not look into
    /// (a hidden zone, or another seat's owner-only zone).
    ShownFace {
        /// The zone.
        zone: ZoneId,
        /// The component.
        component: ComponentId,
    },
    /// A move's path names a zone that is not in the seat's table.
    UnknownZone {
        /// Index of the move.
        index: usize,
        /// The missing zone.
        id: ZoneId,
    },
    /// Walking every choice path did not reach every legal move once.
    /// Unique, prefix-free paths already guarantee reachability; this is a
    /// defensive check of the walk itself.
    Unreachable {
        /// Legal moves.
        moves: usize,
        /// Moves reached through choices.
        reached: usize,
    },
    /// A complete choice path leads to a move that [`Game::apply`] rejects.
    Illegal {
        /// Index of the move.
        index: usize,
        /// The game's reason.
        error: IllegalMove,
    },
}

/// Conformance check of the selection protocol for `seat` in `state`: the
/// seat's table is well-formed (see [`check_table`]), every legal move is
/// reachable through choices, every choice refers to the seat's table, and no
/// choice path leads to an illegal move.
///
/// Walks the whole choice tree from the empty selection and applies each
/// completed move to a copy of `state` with a copy of `rng`.
///
/// # Errors
///
/// The first [`SelectionViolation`] found.
pub fn check_selection<G: Game>(
    state: &G::State,
    seat: Seat,
    rng: &GameRng,
) -> Result<(), SelectionViolation> {
    let view = G::view(state, Observer::Seat(seat));
    let table = G::table(&view);
    check_table(&table, Some(seat))?;
    let moves = G::legal_moves(state, seat);
    let paths: Vec<(Vec<Choice>, usize)> = moves
        .iter()
        .enumerate()
        .map(|(index, mv)| (G::choices(&view, mv), index))
        .collect();
    for (path, index) in &paths {
        check_references(&table, path, *index)?;
    }
    let selection = Selection::from_paths(paths).map_err(SelectionViolation::Ambiguous)?;

    let mut reached = 0;
    walk::<G>(state, seat, rng, &moves, selection, &mut reached)?;
    if reached == moves.len() {
        Ok(())
    } else {
        Err(SelectionViolation::Unreachable {
            moves: moves.len(),
            reached,
        })
    }
}

/// Checks a table model as seen by `seat` (`None` for a spectator): zone and
/// component IDs are unique, and no face is shown in a hidden zone or in an
/// owner-only zone of another seat.
///
/// # Errors
///
/// The first [`SelectionViolation`] found.
pub fn check_table(table: &TableModel, seat: Option<Seat>) -> Result<(), SelectionViolation> {
    let mut zones: Vec<ZoneId> = table.zones.iter().map(|zone| zone.id).collect();
    zones.sort_unstable();
    if let Some(pair) = zones.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(SelectionViolation::DuplicateZone(pair[0]));
    }
    let mut components: Vec<ComponentId> = table
        .zones
        .iter()
        .flat_map(|zone| zone.components.iter().map(|component| component.id))
        .collect();
    components.sort_unstable();
    if let Some(pair) = components.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(SelectionViolation::DuplicateComponent(pair[0]));
    }
    for zone in &table.zones {
        let closed = match zone.visibility {
            Visibility::Public => false,
            Visibility::Owner => zone.owner.is_none() || zone.owner != seat,
            Visibility::Hidden => true,
        };
        if let Some(component) = zone
            .components
            .iter()
            .find(|component| closed && component.face != Face::Hidden)
        {
            return Err(SelectionViolation::ShownFace {
                zone: zone.id,
                component: component.id,
            });
        }
    }
    Ok(())
}

fn check_references(
    table: &TableModel,
    path: &[Choice],
    index: usize,
) -> Result<(), SelectionViolation> {
    let component = |id: ComponentId| {
        if table.component(id).is_some() {
            Ok(())
        } else {
            Err(SelectionViolation::UnknownComponent { index, id })
        }
    };
    for choice in path {
        match choice {
            Choice::Component(id) => component(*id)?,
            Choice::Subset(set) if set.as_slice().is_empty() => {
                return Err(SelectionViolation::EmptySubset { index });
            }
            Choice::Subset(set) => set.as_slice().iter().try_for_each(|id| component(*id))?,
            Choice::Zone(id) => {
                if table.zone(*id).is_none() {
                    return Err(SelectionViolation::UnknownZone { index, id: *id });
                }
            }
            Choice::Option(_) | Choice::Number(_) => {}
        }
    }
    Ok(())
}

fn walk<G: Game>(
    state: &G::State,
    seat: Seat,
    rng: &GameRng,
    moves: &[G::Move],
    selection: Selection<usize>,
    reached: &mut usize,
) -> Result<(), SelectionViolation> {
    if let Some(&index) = selection.complete() {
        *reached += 1;
        let mut state = state.clone();
        let mut rng = rng.clone();
        return G::apply(&mut state, seat, &moves[index], &mut rng)
            .map(drop)
            .map_err(|error| SelectionViolation::Illegal { index, error });
    }
    for choice in selection.selectable() {
        let mut next = selection.clone();
        next.choose(choice)
            .expect("a selectable choice is accepted");
        walk::<G>(state, seat, rng, moves, next, reached)?;
    }
    Ok(())
}
