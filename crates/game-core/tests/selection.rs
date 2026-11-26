//! Exercises the selection protocol and the table model with a tiny card
//! game: seat 0 holds a hand and may play a card, play a Jack and wish a
//! suit, draw from the pile, or discard a pair. A `Fault` in the config makes
//! the game describe its moves wrongly, so the conformance check can be shown
//! to catch each kind of mistake.

use libretable_game_core::{
    Attribute, Choice, Component, ComponentId, ComponentKind, ComponentSet, Face, Game, GameId,
    GameRng, IllegalMove, Indicator, IndicatorKind, IndicatorValue, InvalidVariant, Layout,
    NotSelectable, Observer, Outcome, Pending, Seat, SeatCount, SeatSet, Seed, Selection,
    SelectionError, SelectionViolation, TableModel, Variant, Visibility, Zone, ZoneId, ZoneKind,
    check_selection, check_table,
};
use serde::{Deserialize, Serialize};

const HAND: ZoneId = ZoneId(0);
const PILE: ZoneId = ZoneId(1);
const PILE_TOP: ComponentId = ComponentId(1000);
const JACK: u32 = 2;
const SUITS: [&str; 4] = ["suit-clubs", "suit-spades", "suit-hearts", "suit-diamonds"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
enum Fault {
    #[default]
    None,
    /// Draw gets the same path as playing card 0.
    Duplicate,
    /// Jack of clubs with a clubs wish stops before the wish.
    Prefix,
    /// Playing card 5 names a component that is not on the table.
    UnknownComponent,
    /// Drawing names a zone that is not on the table.
    UnknownZone,
    /// `legal_moves` lists a wish for a fifth suit that `apply` rejects.
    IllegalLeaf,
    /// Discarding a pair is described as an empty subset.
    EmptySubset,
}

impl Variant for Fault {
    fn validate(&self, _seats: SeatCount) -> Result<(), InvalidVariant> {
        Ok(())
    }
}

/// Card `n` has suit `n % 4` and rank `n / 4`; rank 2 is the Jack.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct State {
    fault: Fault,
    hand: Vec<u32>,
    done: bool,
}

/// Seat 0 sees its hand; everybody else sees only how many cards it holds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct View {
    fault: Fault,
    hand: Option<Vec<u32>>,
    hand_len: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Move {
    Play(u32),
    Jack { card: u32, wish: u8 },
    Draw,
    Discard(u32, u32),
}

struct Wish;

fn rank(card: u32) -> u32 {
    card / 4
}

fn valid(state: &State, mv: &Move) -> bool {
    let held = |card: &u32| state.hand.contains(card);
    match mv {
        Move::Play(card) => held(card) && rank(*card) != JACK,
        Move::Jack { card, wish } => held(card) && rank(*card) == JACK && *wish < 4,
        Move::Draw => true,
        Move::Discard(a, b) => a < b && held(a) && held(b) && rank(*a) != JACK && rank(*b) != JACK,
    }
}

impl Game for Wish {
    type Config = Fault;
    type State = State;
    type Move = Move;
    type View = View;
    type Event = Move;

    const ID: GameId = GameId::new("wish");

    fn setup(fault: &Fault, _seats: SeatCount, _rng: &mut GameRng) -> State {
        State {
            fault: *fault,
            hand: vec![0, 5, 8, 9],
            done: false,
        }
    }

    fn pending(state: &State) -> Pending {
        if state.done {
            Pending::Finished
        } else {
            Pending::All(SeatSet::single(Seat::new(0).unwrap()))
        }
    }

    fn legal_moves(state: &State, seat: Seat) -> Vec<Move> {
        if !Self::pending(state).allows(seat) {
            return Vec::new();
        }
        let mut moves = Vec::new();
        for &card in &state.hand {
            if rank(card) == JACK {
                moves.extend((0..4).map(|wish| Move::Jack { card, wish }));
            } else {
                moves.push(Move::Play(card));
            }
        }
        moves.push(Move::Draw);
        for &a in &state.hand {
            for &b in &state.hand {
                let mv = Move::Discard(a, b);
                if valid(state, &mv) {
                    moves.push(mv);
                }
            }
        }
        if state.fault == Fault::IllegalLeaf {
            moves.push(Move::Jack { card: 8, wish: 4 });
        }
        moves
    }

    fn apply(
        state: &mut State,
        seat: Seat,
        mv: &Move,
        _rng: &mut GameRng,
    ) -> Result<Vec<Move>, IllegalMove> {
        if !Self::pending(state).allows(seat) || !valid(state, mv) {
            return Err(IllegalMove {
                reason: "wish-illegal",
            });
        }
        state.done = true;
        Ok(vec![mv.clone()])
    }

    fn view(state: &State, observer: Observer) -> View {
        let own = observer == Observer::Seat(Seat::new(0).unwrap());
        View {
            fault: state.fault,
            hand: own.then(|| state.hand.clone()),
            hand_len: state.hand.len() as u32,
        }
    }

    fn table(view: &View) -> TableModel {
        let back = |id: u32| Component {
            id: ComponentId(id),
            kind: ComponentKind::Card,
            face: Face::Hidden,
            rotation: 0,
        };
        let card = |n: u32| Component {
            id: ComponentId(n),
            kind: ComponentKind::Card,
            face: Face::Shown {
                key: format!("card-{n}"),
                attributes: vec![
                    Attribute::new("suit", SUITS[(n % 4) as usize]),
                    Attribute::new("rank", format!("{}", rank(n))),
                ],
            },
            rotation: 0,
        };
        TableModel {
            zones: vec![
                Zone {
                    id: HAND,
                    kind: ZoneKind::Hand,
                    label: "zone-hand".into(),
                    owner: Seat::new(0),
                    visibility: Visibility::Owner,
                    layout: Layout::Fan,
                    components: match &view.hand {
                        Some(hand) => hand.iter().copied().map(card).collect(),
                        None => (0..view.hand_len).map(|i| back(2000 + i)).collect(),
                    },
                },
                Zone {
                    id: PILE,
                    kind: ZoneKind::DrawPile,
                    label: "zone-draw-pile".into(),
                    owner: None,
                    visibility: Visibility::Hidden,
                    layout: Layout::Stack,
                    components: vec![back(PILE_TOP.0)],
                },
            ],
            indicators: vec![Indicator {
                kind: IndicatorKind::Turn,
                label: "indicator-turn".into(),
                owner: None,
                value: IndicatorValue::Seat(Seat::new(0).unwrap()),
            }],
        }
    }

    fn choices(view: &View, mv: &Move) -> Vec<Choice> {
        let fault = view.fault;
        match mv {
            Move::Draw if fault == Fault::Duplicate => vec![Choice::Component(ComponentId(0))],
            Move::Draw if fault == Fault::UnknownZone => vec![Choice::Zone(ZoneId(7))],
            Move::Draw => vec![Choice::Zone(PILE)],
            Move::Play(5) if fault == Fault::UnknownComponent => {
                vec![Choice::Component(ComponentId(99))]
            }
            Move::Play(card) => vec![Choice::Component(ComponentId(*card))],
            Move::Jack { card: 8, wish: 0 } if fault == Fault::Prefix => {
                vec![Choice::Component(ComponentId(8))]
            }
            Move::Jack { card, wish } => vec![
                Choice::Component(ComponentId(*card)),
                Choice::Option(
                    SUITS
                        .get(usize::from(*wish))
                        .copied()
                        .unwrap_or("suit-none")
                        .into(),
                ),
            ],
            Move::Discard(..) if fault == Fault::EmptySubset => {
                vec![Choice::Subset(ComponentSet::default())]
            }
            Move::Discard(a, b) => vec![Choice::Subset(
                [ComponentId(*a), ComponentId(*b)].into_iter().collect(),
            )],
        }
    }

    fn outcome(_state: &State) -> Option<Outcome> {
        None
    }
}

fn start(fault: Fault) -> (State, GameRng) {
    let mut rng = GameRng::from_seed(Seed::from_u64(0));
    let state = Wish::setup(&fault, SeatCount::new(2).unwrap(), &mut rng);
    (state, rng)
}

fn seat(n: u8) -> Seat {
    Seat::new(n).unwrap()
}

fn component(n: u32) -> Choice {
    Choice::Component(ComponentId(n))
}

fn option(key: &str) -> Choice {
    Choice::Option(key.into())
}

#[test]
fn a_correct_game_passes_the_check() {
    let (state, rng) = start(Fault::None);
    check_selection::<Wish>(&state, seat(0), &rng).unwrap();
    // A seat that is not pending has nothing to select.
    check_selection::<Wish>(&state, seat(1), &rng).unwrap();
    assert!(Selection::new::<Wish>(&state, seat(1)).unwrap().is_empty());
}

#[test]
fn the_check_catches_every_fault() {
    let check = |fault| {
        let (state, rng) = start(fault);
        check_selection::<Wish>(&state, seat(0), &rng).unwrap_err()
    };
    // Move order: Play(0), Play(5), Jack 8 x4 (2..=5), Jack 9 x4 (6..=9),
    // Draw (10), Discard(0, 5) (11), then the illegal wish (12).
    assert_eq!(
        check(Fault::Duplicate),
        SelectionViolation::Ambiguous(SelectionError::Duplicate {
            first: 0,
            second: 10
        })
    );
    assert_eq!(
        check(Fault::Prefix),
        SelectionViolation::Ambiguous(SelectionError::Prefix {
            shorter: 2,
            longer: 5
        })
    );
    assert_eq!(
        check(Fault::UnknownComponent),
        SelectionViolation::UnknownComponent {
            index: 1,
            id: ComponentId(99)
        }
    );
    assert_eq!(
        check(Fault::UnknownZone),
        SelectionViolation::UnknownZone {
            index: 10,
            id: ZoneId(7)
        }
    );
    assert_eq!(
        check(Fault::EmptySubset),
        SelectionViolation::EmptySubset { index: 11 }
    );
    assert_eq!(
        check(Fault::IllegalLeaf),
        SelectionViolation::Illegal {
            index: 12,
            error: IllegalMove {
                reason: "wish-illegal"
            }
        }
    );
}

#[test]
fn choices_narrow_down_to_one_move() {
    let (state, _) = start(Fault::None);
    let mut selection = Selection::new::<Wish>(&state, seat(0)).unwrap();
    assert_eq!(
        selection.selectable(),
        vec![
            component(0),
            component(5),
            component(8),
            component(9),
            Choice::Zone(PILE),
            Choice::Subset([ComponentId(5), ComponentId(0)].into_iter().collect()),
        ]
    );
    assert_eq!(selection.complete(), None);

    selection.choose(component(8)).unwrap();
    assert_eq!(selection.remaining().count(), 4);
    assert_eq!(
        selection.selectable(),
        SUITS.iter().map(|s| option(s)).collect::<Vec<_>>()
    );
    assert_eq!(selection.complete(), None);

    selection.choose(option("suit-hearts")).unwrap();
    assert_eq!(selection.selectable(), Vec::new());
    assert_eq!(selection.complete(), Some(&Move::Jack { card: 8, wish: 2 }));
    assert_eq!(selection.made(), [component(8), option("suit-hearts")]);
    assert_eq!(
        selection.clone().into_move(),
        Some(Move::Jack { card: 8, wish: 2 })
    );

    assert_eq!(selection.undo(), Some(option("suit-hearts")));
    assert_eq!(selection.undo(), Some(component(8)));
    assert_eq!(selection.undo(), None);
    selection.choose(component(0)).unwrap();
    assert_eq!(selection.complete(), Some(&Move::Play(0)));
}

#[test]
fn unselectable_choices_are_rejected() {
    let (state, _) = start(Fault::None);
    let mut selection = Selection::new::<Wish>(&state, seat(0)).unwrap();
    assert_eq!(selection.choose(option("suit-hearts")), Err(NotSelectable));
    assert_eq!(selection.choose(component(99)), Err(NotSelectable));
    assert_eq!(selection.choose(Choice::Number(0)), Err(NotSelectable));
    assert!(selection.made().is_empty());
    selection.choose(component(0)).unwrap();
    assert_eq!(selection.choose(component(5)), Err(NotSelectable));
    assert_eq!(selection.made(), [component(0)]);
}

#[test]
fn a_single_move_may_need_no_choice() {
    let selection = Selection::from_paths(vec![(Vec::new(), "pass")]).unwrap();
    assert_eq!(selection.complete(), Some(&"pass"));
    assert_eq!(
        Selection::from_paths(vec![(Vec::new(), "pass"), (vec![Choice::Number(1)], "bid")]),
        Err(SelectionError::Prefix {
            shorter: 0,
            longer: 1
        })
    );
}

#[test]
fn component_sets_are_canonical() {
    let set: ComponentSet = [3, 1, 3, 2].into_iter().map(ComponentId).collect();
    assert_eq!(
        set.as_slice(),
        [ComponentId(1), ComponentId(2), ComponentId(3)]
    );
    assert!(set.contains(ComponentId(2)));
    assert!(!set.contains(ComponentId(4)));
    let json = serde_json::to_string(&set).unwrap();
    assert_eq!(json, "[1,2,3]");
    assert_eq!(serde_json::from_str::<ComponentSet>(&json).unwrap(), set);
    assert!(serde_json::from_str::<ComponentSet>("[2,1]").is_err());
    assert!(serde_json::from_str::<ComponentSet>("[1,1]").is_err());
}

#[test]
fn choices_have_a_readable_wire_form() {
    let cases = [
        (component(3), r#"{"component":3}"#),
        (Choice::Zone(ZoneId(1)), r#"{"zone":1}"#),
        (option("suit-hearts"), r#"{"option":"suit-hearts"}"#),
        (Choice::Number(-18), r#"{"number":-18}"#),
        (
            Choice::Subset([ComponentId(4), ComponentId(2)].into_iter().collect()),
            r#"{"subset":[2,4]}"#,
        ),
    ];
    for (choice, json) in cases {
        assert_eq!(serde_json::to_string(&choice).unwrap(), json);
        assert_eq!(serde_json::from_str::<Choice>(json).unwrap(), choice);
    }
}

#[test]
fn table_model_round_trips_and_finds_things() {
    let (state, _) = start(Fault::None);
    let table = Wish::table(&Wish::view(&state, Observer::Seat(seat(0))));
    let json = serde_json::to_string(&table).unwrap();
    assert_eq!(serde_json::from_str::<TableModel>(&json).unwrap(), table);
    assert!(json.contains(r#""kind":"draw-pile""#), "{json}");
    assert!(json.contains(r#""face":"hidden""#), "{json}");

    assert_eq!(table.zone(PILE).unwrap().kind, ZoneKind::DrawPile);
    assert_eq!(table.zone(ZoneId(9)), None);
    let (zone, card) = table.component(ComponentId(5)).unwrap();
    assert_eq!(zone.id, HAND);
    assert_eq!(card.kind, ComponentKind::Card);
    assert!(table.component(ComponentId(6)).is_none());
}

#[test]
fn other_seats_see_only_card_backs() {
    let (state, _) = start(Fault::None);
    let table = Wish::table(&Wish::view(&state, Observer::Seat(seat(1))));
    let hand = table.zone(HAND).unwrap();
    assert_eq!(hand.components.len(), 4);
    assert!(hand.components.iter().all(|c| c.face == Face::Hidden));
    check_table(&table, Some(seat(1))).unwrap();
    check_table(&table, None).unwrap();
}

#[test]
fn the_table_check_catches_broken_tables() {
    let (state, _) = start(Fault::None);
    let own = Wish::table(&Wish::view(&state, Observer::Seat(seat(0))));
    check_table(&own, Some(seat(0))).unwrap();
    // Seat 0's open hand must not reach anyone else.
    assert_eq!(
        check_table(&own, Some(seat(1))),
        Err(SelectionViolation::ShownFace {
            zone: HAND,
            component: ComponentId(0)
        })
    );
    assert!(check_table(&own, None).is_err());

    let mut shown_pile = own.clone();
    shown_pile.zones[1].components[0].face = own.zones[0].components[0].face.clone();
    assert_eq!(
        check_table(&shown_pile, Some(seat(0))),
        Err(SelectionViolation::ShownFace {
            zone: PILE,
            component: PILE_TOP
        })
    );

    let mut twin_zone = own.clone();
    twin_zone.zones[1].id = HAND;
    assert_eq!(
        check_table(&twin_zone, Some(seat(0))),
        Err(SelectionViolation::DuplicateZone(HAND))
    );

    let mut twin_card = own.clone();
    twin_card.zones[1].components[0].id = ComponentId(5);
    assert_eq!(
        check_table(&twin_card, Some(seat(0))),
        Err(SelectionViolation::DuplicateComponent(ComponentId(5)))
    );
}
