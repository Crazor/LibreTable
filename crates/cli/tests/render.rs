//! Text-renderer snapshot tests: exact expected output, in English (the
//! default locale). The `demo-` IDs of the hand-built table have no message,
//! so they show unchanged, as any gap in the catalog would.

use libretable_cli::render;
use libretable_conformance::rock_paper_scissors::{Config, RockPaperScissors};
use libretable_game_core::{
    Attribute, Choice, Component, ComponentId, ComponentKind, ComponentSet, Face, Indicator,
    IndicatorKind, IndicatorValue, Layout, Observer, Outcome, Seat, SeatCount, SeatResult, SeatSet,
    Seed, TableModel, Visibility, Zone, ZoneId, ZoneKind,
};
use libretable_session::Session;

fn seat(index: u8) -> Seat {
    Seat::new(index).unwrap()
}

fn card(id: u32, face: Option<(&str, &str, &str)>, rotation: u16) -> Component {
    Component {
        id: ComponentId(id),
        kind: ComponentKind::Card,
        face: face.map_or(Face::Hidden, |(key, suit, rank)| Face::Shown {
            key: key.into(),
            attributes: vec![Attribute::new("suit", suit), Attribute::new("rank", rank)],
        }),
        rotation,
    }
}

/// A table using every feature the renderer knows.
fn sample() -> TableModel {
    TableModel {
        zones: vec![
            Zone {
                id: ZoneId(0),
                kind: ZoneKind::DrawPile,
                label: "demo-zone-stock".into(),
                owner: None,
                visibility: Visibility::Hidden,
                layout: Layout::Stack,
                components: vec![card(7, None, 0), card(8, None, 0)],
            },
            Zone {
                id: ZoneId(1),
                kind: ZoneKind::Hand,
                label: "demo-zone-hand".into(),
                owner: Some(seat(0)),
                visibility: Visibility::Owner,
                layout: Layout::Fan,
                components: vec![
                    card(1, Some(("queen-of-hearts", "hearts", "queen")), 0),
                    card(2, Some(("jack-of-clubs", "clubs", "jack")), 90),
                ],
            },
            Zone {
                id: ZoneId(2),
                kind: ZoneKind::Board,
                label: "demo-zone-board".into(),
                owner: None,
                visibility: Visibility::Public,
                layout: Layout::Grid { columns: 7 },
                components: vec![Component {
                    id: ComponentId(3),
                    kind: ComponentKind::Piece,
                    face: Face::Shown {
                        key: "red".into(),
                        attributes: Vec::new(),
                    },
                    rotation: 0,
                }],
            },
            Zone {
                id: ZoneId(3),
                kind: ZoneKind::Discard,
                label: "demo-zone-discard".into(),
                owner: None,
                visibility: Visibility::Public,
                layout: Layout::Free,
                components: Vec::new(),
            },
        ],
        indicators: vec![
            Indicator {
                kind: IndicatorKind::Turn,
                label: "demo-indicator-turn".into(),
                owner: None,
                value: IndicatorValue::Seat(seat(1)),
            },
            Indicator {
                kind: IndicatorKind::Announcement,
                label: "demo-indicator-wish".into(),
                owner: Some(seat(0)),
                value: IndicatorValue::Message("demo-suit-hearts".into()),
            },
            Indicator {
                kind: IndicatorKind::Score,
                label: "demo-indicator-score".into(),
                owner: Some(seat(1)),
                value: IndicatorValue::Number(-3),
            },
        ],
    }
}

#[test]
fn renders_every_table_feature() {
    assert_eq!(
        render::table(&sample(), Observer::Seat(seat(0))),
        "\
--- Table: seat 0 ---
demo-indicator-turn: seat 1
demo-indicator-wish (seat 0): demo-suit-hearts
demo-indicator-score (seat 1): -3
[0] demo-zone-stock: shared, draw pile, hidden, stack
    #7 card: face down
    #8 card: face down
[1] demo-zone-hand: seat 0, hand, owner only, fan
    #1 card: queen-of-hearts (suit: hearts, rank: queen)
    #2 card: jack-of-clubs (suit: clubs, rank: jack), rotated 90 degrees
[2] demo-zone-board: shared, board, public, grid of 7 columns
    #3 piece: red
[3] demo-zone-discard: shared, discard pile, public, free
    (empty)
"
    );
}

#[test]
fn renders_every_kind_of_choice() {
    let table = sample();
    let selectable = vec![
        Choice::Component(ComponentId(1)),
        Choice::Zone(ZoneId(0)),
        Choice::Option("demo-pass".into()),
        Choice::Number(18),
        Choice::Subset(
            [ComponentId(2), ComponentId(1)]
                .into_iter()
                .collect::<ComponentSet>(),
        ),
        Choice::Component(ComponentId(99)),
    ];
    let made = [Choice::Option("demo-play".into()), Choice::Number(2)];
    assert_eq!(
        render::prompt(&table, seat(0), &made, &selectable),
        "\
Chosen so far: demo-play > 2
Seat 0, choose:
  1) #1 card: queen-of-hearts (suit: hearts, rank: queen) in demo-zone-hand
  2) [0] demo-zone-stock
  3) demo-pass
  4) 18
  5) #1 card: queen-of-hearts (suit: hearts, rank: queen) in demo-zone-hand; #2 card: jack-of-clubs (suit: clubs, rank: jack), rotated 90 degrees in demo-zone-hand
  6) #99
Enter a number, u to undo, q to quit:
"
    );
    assert_eq!(render::choices(&table, &[]), "(no choice needed)");
}

#[test]
fn renders_outcome_with_teams() {
    let outcome = Outcome {
        seats: vec![
            SeatResult { rank: 1, score: 3 },
            SeatResult { rank: 2, score: 1 },
            SeatResult { rank: 1, score: 3 },
        ],
        teams: vec![
            SeatSet::single(seat(0)).with(seat(2)),
            SeatSet::single(seat(1)),
        ],
    };
    assert_eq!(
        render::outcome(&outcome),
        "\
Result:
  Seat 0: rank 1, score 3
  Seat 1: rank 2, score 1
  Seat 2: rank 1, score 3
  Team: 0, 2
  Team: 1
"
    );
}

/// The rock-paper-scissors table of seat 0 and of the spectator after seat 0
/// has thrown: the throw is visible to its owner only.
#[test]
fn renders_rock_paper_scissors_views() {
    let mut session = Session::<RockPaperScissors>::new(
        Config::default(),
        SeatCount::new(2).unwrap(),
        Seed::from_u64(0),
        "test",
    )
    .unwrap();
    let selectable = session.prompt(seat(0), &[]).unwrap().selectable;
    let table = session.table(Observer::Seat(seat(0)));
    assert_eq!(
        render::prompt(&table, seat(0), &[], &selectable),
        "\
Seat 0, choose:
  1) Rock
  2) Paper
  3) Scissors
Enter a number, u to undo, q to quit:
"
    );
    session
        .submit_choices(seat(0), &[Choice::Option("rps-rock".into())])
        .unwrap();
    assert_eq!(
        render::table(
            &session.table(Observer::Seat(seat(0))),
            Observer::Seat(seat(0))
        ),
        "\
--- Table: seat 0 ---
Round: 1
Score (seat 0): 0
Score (seat 1): 0
[0] Throw: seat 0, player board, owner only, stack
    #0 token: Rock (shape: rock)
[1] Throw: seat 1, player board, owner only, stack
    (empty)
[64] Last round: shared, tableau, public, row
    (empty)
"
    );
    assert_eq!(
        render::table(&session.table(Observer::Spectator), Observer::Spectator),
        "\
--- Table: spectator ---
Round: 1
Score (seat 0): 0
Score (seat 1): 0
[0] Throw: seat 0, player board, owner only, stack
    #0 token: face down
[1] Throw: seat 1, player board, owner only, stack
    (empty)
[64] Last round: shared, tableau, public, row
    (empty)
"
    );
}
