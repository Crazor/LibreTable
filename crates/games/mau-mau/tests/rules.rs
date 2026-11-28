//! Rule tests from docs/games/mau-mau/rules.md.

use libretable_cards::{Card, Rank, Suit};
use libretable_game_core::{
    ComponentId, Face, Game, GameRng, Observer, Pending, Seat, SeatCount, Seed, Variant,
};
use libretable_mau_mau::{
    Announce, Config, Event, MauMau, Move, Phase, Scoring, State, TURN_CAP_PER_SEAT, arrange,
    index, set_turn, set_turns_played,
};

// ---------------------------------------------------------------- helpers

fn seat(n: u8) -> Seat {
    Seat::new(n).unwrap()
}

fn rng() -> GameRng {
    GameRng::from_seed(Seed::from_u64(7))
}

/// A card of the first deck by short name: rank (`7`…`10`, `J`, `Q`, `K`,
/// `A`) and suit (`C`, `S`, `H`, `D`), e.g. `"10H"`, `"JC"`.
fn c(name: &str) -> u8 {
    let (rank, suit) = name.split_at(name.len() - 1);
    let suit = match suit {
        "C" => Suit::Clubs,
        "S" => Suit::Spades,
        "H" => Suit::Hearts,
        "D" => Suit::Diamonds,
        _ => panic!("suit of {name}"),
    };
    let rank = match rank {
        "7" => Rank::Seven,
        "8" => Rank::Eight,
        "9" => Rank::Nine,
        "10" => Rank::Ten,
        "J" => Rank::Jack,
        "Q" => Rank::Queen,
        "K" => Rank::King,
        "A" => Rank::Ace,
        _ => panic!("rank of {name}"),
    };
    index(0, Card::new(suit, rank))
}

fn cards(names: &str) -> Vec<u8> {
    names.split_whitespace().map(c).collect()
}

fn play(name: &str) -> Move {
    Move::Play {
        card: c(name),
        wish: None,
    }
}

fn jack(name: &str, wish: Suit) -> Move {
    Move::Play {
        card: c(name),
        wish: Some(wish),
    }
}

/// A match of `config` for `n` seats with the given hands and discard pile
/// (bottom to top). The draw pile holds `draw_top` on top of every card not
/// named; seat `turn` is to play with no open effect.
fn table(config: Config, hands: &[&str], discard: &str, draw_top: &str, turn: u8) -> State {
    let n = u8::try_from(hands.len()).unwrap();
    let seats = SeatCount::new(n).unwrap();
    config.validate(seats).unwrap();
    let mut state = MauMau::setup(&config, seats, &mut rng());
    let hands: Vec<Vec<u8>> = hands.iter().map(|h| cards(h)).collect();
    let discard = cards(discard);
    let top = cards(draw_top);
    let named: Vec<u8> = hands
        .iter()
        .flatten()
        .chain(&discard)
        .chain(&top)
        .copied()
        .collect();
    let mut draw: Vec<u8> = (0..config.cards()).filter(|c| !named.contains(c)).collect();
    draw.extend(top.iter().rev());
    let hand_refs: Vec<&[u8]> = hands.iter().map(Vec::as_slice).collect();
    arrange(&mut state, &hand_refs, &discard, &draw);
    set_turn(&mut state, seat(turn), 0, None, Phase::Play);
    state
}

fn standard(hands: &[&str], discard: &str, turn: u8) -> State {
    table(Config::default(), hands, discard, "", turn)
}

fn legal(state: &State) -> Vec<Move> {
    match MauMau::pending(state) {
        Pending::All(set) => MauMau::legal_moves(state, set.iter().next().unwrap()),
        _ => Vec::new(),
    }
}

fn turn(state: &State) -> Seat {
    match MauMau::pending(state) {
        Pending::All(set) => {
            assert_eq!(set.len(), 1, "exactly one seat decides");
            set.iter().next().unwrap()
        }
        other => panic!("not a turn: {other:?}"),
    }
}

fn apply(state: &mut State, mv: Move) -> Vec<Event> {
    let seat = turn(state);
    MauMau::apply(state, seat, &mv, &mut rng())
        .unwrap_or_else(|e| panic!("{mv:?} of {seat:?} rejected: {}", e.reason))
}

fn rejects(state: &State, mv: Move) -> &'static str {
    let mut probe = state.clone();
    MauMau::apply(&mut probe, turn(state), &mv, &mut rng())
        .expect_err("move should be rejected")
        .reason
}

fn hand_len(state: &State, s: u8) -> usize {
    let table = MauMau::table(&MauMau::view(state, Observer::Spectator));
    table
        .zone(libretable_game_core::ZoneId(u32::from(s)))
        .unwrap()
        .components
        .len()
}

fn top(state: &State) -> u8 {
    MauMau::view(state, Observer::Spectator).top
}

fn plays(state: &State) -> Vec<u8> {
    let mut cards: Vec<u8> = legal(state)
        .into_iter()
        .filter_map(|mv| match mv {
            Move::Play { card, .. } => Some(card),
            _ => None,
        })
        .collect();
    cards.dedup();
    cards
}

// ------------------------------------------------------------- variants

#[test]
fn seat_and_deck_limits() {
    let ok = |config: &Config, n| config.validate(SeatCount::new(n).unwrap());
    let standard = Config::default();
    assert_eq!(
        ok(&standard, 1).unwrap_err().reason,
        "mau-mau-too-few-seats"
    );
    assert!(ok(&standard, 2).is_ok());
    assert!(ok(&standard, 5).is_ok());
    assert_eq!(
        ok(&standard, 6).unwrap_err().reason,
        "mau-mau-too-many-seats"
    );
    let six = Config {
        hand_size: 6,
        ..Config::default()
    };
    // 5 × 6 + 1 + 5 > 32: the draw pile must hold a card per seat.
    assert_eq!(ok(&six, 5).unwrap_err().reason, "mau-mau-deck-too-small");
    assert!(ok(&six, 4).is_ok());
    let two = Config {
        decks: 2,
        ..Config::default()
    };
    assert!(ok(&two, 10).is_ok());
    assert_eq!(ok(&two, 11).unwrap_err().reason, "mau-mau-too-many-seats");
    for (config, reason) in [
        (
            Config {
                decks: 3,
                ..Config::default()
            },
            "mau-mau-invalid-decks",
        ),
        (
            Config {
                hand_size: 2,
                ..Config::default()
            },
            "mau-mau-invalid-hand-size",
        ),
        (
            Config {
                hand_size: 8,
                ..Config::default()
            },
            "mau-mau-invalid-hand-size",
        ),
        (
            Config {
                mau_penalty: 3,
                ..Config::default()
            },
            "mau-mau-invalid-penalty",
        ),
        (
            Config {
                rounds: 0,
                ..Config::default()
            },
            "mau-mau-invalid-rounds",
        ),
        (
            Config {
                rounds: 21,
                ..Config::default()
            },
            "mau-mau-invalid-rounds",
        ),
    ] {
        assert_eq!(ok(&config, 2).unwrap_err().reason, reason);
    }
}

#[test]
fn variant_json_defaults_missing_fields() {
    let config: Config = serde_json::from_str(r#"{"rounds":3,"announce":"manual"}"#).unwrap();
    assert_eq!(
        config,
        Config {
            rounds: 3,
            announce: Announce::Manual,
            ..Config::default()
        }
    );
    assert!(serde_json::from_str::<Config>(r#"{"no_such_rule":true}"#).is_err());
}

// ---------------------------------------------------------------- setup

#[test]
fn the_deal() {
    let config = Config::default();
    let state = MauMau::setup(&config, SeatCount::new(3).unwrap(), &mut rng());
    for s in 0..3 {
        assert_eq!(hand_len(&state, s), 5);
    }
    let view = MauMau::view(&state, Observer::Spectator);
    assert_eq!(view.draw.len(), 32 - 15 - 1);
    assert!(view.buried.is_empty());
}

#[test]
fn start_card_effects() {
    // Search seeds for each start card rank; the effect follows the rank.
    let config = Config {
        nine_reverses: true,
        ..Config::default()
    };
    let mut found = [false; 4];
    for seed in 0..400 {
        let mut r = GameRng::from_seed(Seed::from_u64(seed));
        let state = MauMau::setup(&config, SeatCount::new(3).unwrap(), &mut r);
        let view = MauMau::view(&state, Observer::Spectator);
        let start = libretable_mau_mau::card(view.top);
        match start.rank {
            Rank::Seven => {
                assert_eq!((turn(&state), view.penalty), (seat(0), 2));
                found[0] = true;
            }
            Rank::Eight => {
                assert_eq!(turn(&state), seat(1), "seat 0 is skipped");
                found[1] = true;
            }
            Rank::Jack => {
                // Any card but a Jack may follow; there is no wish.
                assert_eq!(view.wish, None);
                assert!(
                    plays(&state)
                        .iter()
                        .all(|&c| libretable_mau_mau::card(c).rank != Rank::Jack)
                );
                found[2] = true;
            }
            Rank::Nine => {
                assert!(!view.clockwise);
                assert_eq!(turn(&state), seat(0));
                found[3] = true;
            }
            _ => assert_eq!((turn(&state), view.penalty), (seat(0), 0)),
        }
    }
    assert_eq!(found, [true; 4]);
}

#[test]
fn start_jack_lets_any_suit_follow() {
    let state = standard(&["7H 8S QD", "9C 10C KC"], "JD", 0);
    assert_eq!(plays(&state), cards("8S 7H QD"));
}

// ------------------------------------------------------------ basic play

#[test]
fn suit_or_rank_must_match() {
    let state = standard(&["7H QS 9C KD", "8C 9D 10D AC"], "QH", 0);
    assert_eq!(plays(&state), cards("QS 7H"));
    assert!(legal(&state).contains(&Move::Draw));
    assert_eq!(rejects(&state, play("9C")), "mau-mau-not-allowed");
    let mut probe = state.clone();
    let err = MauMau::apply(&mut probe, seat(1), &play("8C"), &mut rng()).unwrap_err();
    assert_eq!(err.reason, "mau-mau-not-your-turn");
}

#[test]
fn play_passes_the_turn_clockwise() {
    let mut state = standard(&["QS 10S", "KS 9C", "AS 9D"], "QH", 0);
    apply(&mut state, play("QS"));
    assert_eq!(turn(&state), seat(1));
    apply(&mut state, play("KS"));
    assert_eq!(turn(&state), seat(2));
    assert_eq!(top(&state), c("KS"));
}

#[test]
fn drawing_is_always_allowed_and_the_drawn_card_may_be_played() {
    let mut state = table(Config::default(), &["QH 9C", "8C 9D"], "10H", "7H", 0);
    // A playable card in hand does not forbid drawing.
    assert!(legal(&state).contains(&play("QH")));
    apply(&mut state, Move::Draw);
    assert_eq!(hand_len(&state, 0), 3);
    // Only the drawn card may be played now; keeping it is always offered.
    assert_eq!(legal(&state), [play("7H"), Move::Keep]);
    apply(&mut state, play("7H"));
    assert_eq!(top(&state), c("7H"));
}

#[test]
fn keep_is_offered_even_when_the_drawn_card_does_not_fit() {
    let mut state = table(Config::default(), &["QH 9C", "8C 9D"], "10H", "7S", 0);
    apply(&mut state, Move::Draw);
    assert_eq!(legal(&state), [Move::Keep]);
    apply(&mut state, Move::Keep);
    assert_eq!(turn(&state), seat(1));
}

// ---------------------------------------------------------------- sevens

#[test]
fn seven_makes_the_next_player_draw_two() {
    let mut state = standard(&["7H 9S", "8C 9D 10D"], "KH", 0);
    apply(&mut state, play("7H"));
    assert_eq!(turn(&state), seat(1));
    // No 7 in hand: drawing the penalty is the only card move. A Jack would
    // not defuse it either.
    assert_eq!(legal(&state), [Move::Draw]);
    apply(&mut state, Move::Draw);
    assert_eq!(hand_len(&state, 1), 5);
    assert_eq!(turn(&state), seat(0), "drawing the penalty ends the turn");
    // The 7 is now an ordinary top card.
    assert_eq!(MauMau::view(&state, Observer::Spectator).penalty, 0);
}

#[test]
fn sevens_stack() {
    let mut state = standard(&["7H 9S", "7C JD 9D", "8D 10S"], "KH", 0);
    apply(&mut state, play("7H"));
    assert_eq!(
        legal(&state),
        [play("7C"), Move::Draw],
        "no Jack against a 7"
    );
    apply(&mut state, play("7C"));
    assert_eq!(MauMau::view(&state, Observer::Spectator).penalty, 4);
    apply(&mut state, Move::Draw);
    assert_eq!(hand_len(&state, 2), 6);
}

#[test]
fn without_stacking_a_seven_cannot_counter() {
    let config = Config {
        seven_stacking: false,
        ..Config::default()
    };
    let mut state = table(config, &["7H 9S", "7C 9D"], "KH", "", 0);
    apply(&mut state, play("7H"));
    assert_eq!(legal(&state), [Move::Draw]);
}

// ---------------------------------------------------------------- eights

#[test]
fn eight_skips_the_next_player() {
    let mut state = standard(&["8H 9S", "8C 9D", "10H 10S"], "KH", 0);
    let events = apply(&mut state, play("8H"));
    assert!(events.contains(&Event::Skipped(seat(1))));
    assert_eq!(turn(&state), seat(2));
}

#[test]
fn with_two_players_the_eight_player_moves_again() {
    let mut state = standard(&["8H 9H", "8C 9D"], "KH", 0);
    apply(&mut state, play("8H"));
    assert_eq!(turn(&state), seat(0));
}

#[test]
fn eight_counter_passes_the_skip_on() {
    let config = Config {
        eight_counter: true,
        ..Config::default()
    };
    let mut state = table(config, &["8H 9S", "8C 9D", "10H 10S"], "KH", "", 0);
    apply(&mut state, play("8H"));
    assert_eq!(turn(&state), seat(1));
    assert_eq!(legal(&state), [play("8C"), Move::AcceptSkip]);
    apply(&mut state, play("8C"));
    assert_eq!(turn(&state), seat(2));
    assert_eq!(legal(&state), [Move::AcceptSkip]);
    apply(&mut state, Move::AcceptSkip);
    assert_eq!(turn(&state), seat(0));
}

// ----------------------------------------------------------------- jacks

#[test]
fn jack_wishes_a_suit_and_the_wish_holds_through_a_draw() {
    let mut state = table(
        Config::default(),
        &["JC 9S", "QD 10D KS", "AS 10S"],
        "KH",
        "7H",
        0,
    );
    let paths: Vec<Move> = legal(&state);
    assert!(Suit::ALL.iter().all(|&s| paths.contains(&jack("JC", s))));
    assert_eq!(
        rejects(&state, play("JC")),
        "mau-mau-not-allowed",
        "wish is mandatory"
    );
    apply(&mut state, jack("JC", Suit::Diamonds));
    // King of spades does not follow the wish, even matching nothing else.
    assert_eq!(plays(&state), cards("10D QD"));
    apply(&mut state, Move::Draw);
    // The drawn 7 of hearts does not fit the wish.
    apply(&mut state, Move::Keep);
    assert_eq!(
        MauMau::view(&state, Observer::Spectator).wish,
        Some(Suit::Diamonds)
    );
}

#[test]
fn no_jack_on_a_jack_unless_allowed() {
    let state = standard(&["JC 9S", "JD 10D"], "KH", 0);
    let mut s = state.clone();
    apply(&mut s, jack("JC", Suit::Spades));
    assert_eq!(plays(&s), Vec::<u8>::new());

    let config = Config {
        jack_on_jack: true,
        ..Config::default()
    };
    let mut s = table(config, &["JC 9S", "JD 10D"], "KH", "", 0);
    apply(&mut s, jack("JC", Suit::Spades));
    assert_eq!(plays(&s), cards("JD"));
}

// -------------------------------------------------------- aces and nines

#[test]
fn ace_again_gives_another_turn() {
    let config = Config {
        ace_again: true,
        ..Config::default()
    };
    let mut state = table(config, &["AH AS 9S", "8C 9D"], "KH", "", 0);
    apply(&mut state, play("AH"));
    assert_eq!(turn(&state), seat(0));
    apply(&mut state, play("AS"));
    assert_eq!(turn(&state), seat(0));
    apply(&mut state, play("9S"));
    assert_eq!(MauMau::outcome(&state).unwrap().seats[0].rank, 1);
}

#[test]
fn nine_reverses_the_direction() {
    let config = Config {
        nine_reverses: true,
        ..Config::default()
    };
    let mut state = table(config.clone(), &["9H 10H", "9C 9D", "10S 10C"], "KH", "", 0);
    apply(&mut state, play("9H"));
    assert_eq!(turn(&state), seat(2));
    // With two players a reversal changes nothing.
    let mut two = table(config, &["9H 10H", "9C 9D"], "KH", "", 0);
    apply(&mut two, play("9H"));
    assert_eq!(turn(&two), seat(1));
}

// -------------------------------------------- reshuffle, passes, the end

#[test]
fn empty_draw_pile_reshuffles_the_discards_below_the_top() {
    // Every card not named sits in hands, so the draw pile is empty.
    let mut state = standard(
        &[
            "7C 8C 9C 10C JC QC KC AC 7S 8S 9S 10S JS QS KS AS 9D 10D QD",
            "AH 7D 8D AD 10H",
        ],
        "KD 7H 8H 9H JH QH KH JD",
        0,
    );
    let view = MauMau::view(&state, Observer::Spectator);
    assert!(view.draw.is_empty());
    let buried_before = view.buried.clone();
    let events = apply(&mut state, Move::Draw);
    assert!(events.contains(&Event::Reshuffled { count: 7 }));
    let view = MauMau::view(&state, Observer::Spectator);
    assert_eq!(view.top, c("JD"), "the top discard stays");
    assert!(view.buried.is_empty());
    assert_eq!(view.draw.len(), 6);
    // The new draw pile carries fresh opaque IDs.
    assert!(view.draw.iter().all(|id| !buried_before.contains(id)));
}

#[test]
fn pass_when_nothing_can_be_drawn_and_stalemate() {
    // All cards in hands except the top; nothing to draw.
    let mut state = standard(
        &[
            "7C 8C 9C 10C JC QC KC AC 7S 8S 9S 10S 7D 8D 9D 10D",
            "JS QS KS AS 7H 8H 9H 10H JH QH KH AH JD QD KD",
        ],
        "AD",
        0,
    );
    // Passing is allowed even with a playable card.
    assert!(legal(&state).contains(&Move::Pass));
    assert!(!legal(&state).contains(&Move::Draw));
    apply(&mut state, Move::Pass);
    let events = apply(&mut state, Move::Pass);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::RoundEnded(r) if r.winner.is_none()))
    );
    // Win-lose without a winner: fewer cards rank better.
    let outcome = MauMau::outcome(&state).unwrap();
    assert_eq!((outcome.seats[0].rank, outcome.seats[1].rank), (2, 1));
}

#[test]
fn going_out_wins_the_round() {
    let mut state = standard(&["QH", "8C 9D", "10S 10C"], "KH", 0);
    let events = apply(&mut state, play("QH"));
    assert!(!events.contains(&Event::LastCard(seat(0))));
    assert_eq!(MauMau::pending(&state), Pending::Finished);
    let outcome = MauMau::outcome(&state).unwrap();
    let ranks: Vec<u8> = outcome.seats.iter().map(|s| s.rank).collect();
    assert_eq!(ranks, [1, 2, 2]);
    assert_eq!(outcome.seats[0].score, 1);
}

#[test]
fn jack_finish_off_forbids_a_jack_as_last_card() {
    let config = Config {
        jack_finish: false,
        ..Config::default()
    };
    let state = table(config, &["JH", "8C 9D"], "KH", "", 0);
    assert_eq!(legal(&state), [Move::Draw]);
    let state = standard(&["JH", "8C 9D"], "KH", 0);
    assert!(legal(&state).contains(&jack("JH", Suit::Clubs)));
}

#[test]
fn auto_mode_announces_the_last_card() {
    let mut state = standard(&["QH 9S", "8C 9D"], "KH", 0);
    assert!(!legal(&state).contains(&Move::Call));
    let events = apply(&mut state, play("QH"));
    assert!(events.contains(&Event::LastCard(seat(0))));
    assert!(
        MauMau::view(&state, Observer::Spectator)
            .last_card
            .contains(seat(0))
    );
}

// ------------------------------------------------- scoring and rounds

#[test]
fn card_count_scoring_over_two_rounds() {
    let config = Config {
        rounds: 2,
        scoring: Scoring::CardCount,
        ..Config::default()
    };
    let mut state = table(config, &["QH", "8C 9D", "10S 10C 7D"], "KH", "", 0);
    let events = apply(&mut state, play("QH"));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Dealt { round: 1, starter, .. } if *starter == seat(1)))
    );
    let view = MauMau::view(&state, Observer::Spectator);
    assert_eq!(view.scores, [0, 2, 3]);
    assert_eq!(MauMau::outcome(&state), None);
}

#[test]
fn going_out_on_a_seven_under_card_count_makes_the_next_player_draw() {
    let config = Config {
        scoring: Scoring::CardCount,
        ..Config::default()
    };
    let mut state = table(config, &["7H", "8C 9D", "10S"], "KH", "", 0);
    apply(&mut state, play("7H"));
    let outcome = MauMau::outcome(&state).unwrap();
    let scores: Vec<i64> = outcome.seats.iter().map(|s| s.score).collect();
    assert_eq!(scores, [0, 4, 1]);
    let ranks: Vec<u8> = outcome.seats.iter().map(|s| s.rank).collect();
    assert_eq!(ranks, [1, 3, 2]);
}

#[test]
fn the_turn_cap_ends_a_round_without_a_winner() {
    let mut state = table(Config::default(), &["QH 9H", "8C 9D 10D"], "KH", "", 0);
    set_turns_played(&mut state, TURN_CAP_PER_SEAT * 2 - 1);
    apply(&mut state, play("QH"));
    assert_eq!(MauMau::pending(&state), Pending::Finished);
    assert_eq!(state.results()[0].winner, None);
}

// ------------------------------------------------- manual announcements

fn manual() -> Config {
    Config {
        announce: Announce::Manual,
        ..Config::default()
    }
}

#[test]
fn call_is_always_offered_and_catch_after_another_seats_turn() {
    let mut state = table(manual(), &["QH 9S 10S", "8C 9D 10D"], "KH", "", 0);
    // First turn of the round: call, no catch.
    assert!(legal(&state).contains(&Move::Call));
    assert!(!legal(&state).contains(&Move::Catch));
    apply(&mut state, play("QH"));
    assert!(legal(&state).contains(&Move::Call));
    assert!(legal(&state).contains(&Move::Catch));
    // Acting closes both.
    apply(&mut state, Move::Draw);
    assert!(!legal(&state).contains(&Move::Call));
    assert!(!legal(&state).contains(&Move::Catch));
}

#[test]
fn a_missed_call_caught_costs_the_offender() {
    let mut state = table(manual(), &["QH 9H", "8C 9D 10D"], "KH", "", 0);
    apply(&mut state, play("QH"));
    let events = apply(&mut state, Move::Catch);
    assert!(events.contains(&Event::Caught {
        catcher: seat(1),
        correct: true,
        penalized: seat(0),
    }));
    assert_eq!(hand_len(&state, 0), 3);
    // The catcher's turn goes on.
    assert_eq!(turn(&state), seat(1));
    assert!(!legal(&state).contains(&Move::Catch));
}

#[test]
fn a_correct_call_makes_a_catch_cost_the_catcher() {
    let mut state = table(manual(), &["QH 9H", "8C 9D 10D"], "KH", "", 0);
    apply(&mut state, Move::Call);
    assert!(!legal(&state).contains(&Move::Call), "once per turn");
    apply(&mut state, play("QH"));
    assert!(
        MauMau::view(&state, Observer::Spectator)
            .last_card
            .contains(seat(0))
    );
    let events = apply(&mut state, Move::Catch);
    assert!(events.contains(&Event::Caught {
        catcher: seat(1),
        correct: false,
        penalized: seat(1),
    }));
    assert_eq!(hand_len(&state, 1), 5);
}

#[test]
fn a_false_call_can_be_caught() {
    let mut state = table(manual(), &["QH 9H 10S", "8C 9D 10D"], "KH", "", 0);
    apply(&mut state, Move::Call);
    apply(&mut state, play("QH"));
    let events = apply(&mut state, Move::Catch);
    assert!(events.contains(&Event::Caught {
        catcher: seat(1),
        correct: true,
        penalized: seat(0),
    }));
}

#[test]
fn an_uncaught_fault_goes_unpunished() {
    let mut state = table(manual(), &["QH 9H", "8C 9D 10D"], "KH", "", 0);
    apply(&mut state, play("QH"));
    apply(&mut state, Move::Draw);
    apply(&mut state, Move::Keep);
    // Seat 0's turn: seat 1 made no fault; catching now would be false.
    let events = apply(&mut state, Move::Catch);
    assert!(events.contains(&Event::Caught {
        catcher: seat(0),
        correct: false,
        penalized: seat(0),
    }));
}

#[test]
fn consecutive_own_turns_count_as_one() {
    let config = Config {
        ace_again: true,
        ..manual()
    };
    let mut state = table(config, &["AH QH 9H", "8C 9D 10D"], "KH", "", 0);
    apply(&mut state, Move::Call);
    apply(&mut state, play("AH"));
    // Same seat again: no catch, and the call is already made.
    assert!(!legal(&state).contains(&Move::Catch));
    assert!(!legal(&state).contains(&Move::Call));
    apply(&mut state, play("QH"));
    // The call covered the run of turns: catching is false.
    let events = apply(&mut state, Move::Catch);
    assert!(events.contains(&Event::Caught {
        catcher: seat(1),
        correct: false,
        penalized: seat(1),
    }));
}

// ---------------------------------------------------------------- views

#[test]
fn views_hide_other_hands_the_draw_pile_and_buried_discards() {
    let mut state = standard(&["QH 9S 7C", "8C 9D 10D"], "KH 10H", 0);
    let mine = MauMau::table(&MauMau::view(&state, Observer::Seat(seat(0))));
    let hand = mine.zone(libretable_game_core::ZoneId(0)).unwrap();
    let ids: Vec<u32> = hand.components.iter().map(|c| c.id.0).collect();
    let sorted: Vec<u32> = cards("7C 9S QH").into_iter().map(u32::from).collect();
    assert_eq!(ids, sorted, "own hand face up, sorted");
    let other = mine.zone(libretable_game_core::ZoneId(1)).unwrap();
    assert!(other.components.iter().all(|c| c.face == Face::Hidden));
    let discard = mine.zone(libretable_mau_mau::DISCARD).unwrap();
    assert_eq!(discard.components[0].face, Face::Hidden, "buried discard");
    assert!(matches!(
        &discard.components[1].face,
        Face::Shown { key, .. } if key == "card-ten-of-hearts"
    ));

    // A hand's face-down IDs are reissued when it changes.
    let theirs = |state: &State| -> Vec<ComponentId> {
        MauMau::table(&MauMau::view(state, Observer::Seat(seat(1))))
            .zone(libretable_game_core::ZoneId(0))
            .unwrap()
            .components
            .iter()
            .map(|c| c.id)
            .collect()
    };
    let before = theirs(&state);
    apply(&mut state, play("QH"));
    let after = theirs(&state);
    assert!(after.iter().all(|id| !before.contains(id)));
}

#[test]
fn message_ids_list_every_card_of_the_deck() {
    let listed: Vec<&str> = libretable_mau_mau::MESSAGE_IDS
        .iter()
        .copied()
        .filter(|id| id.starts_with("card-"))
        .collect();
    let deck: Vec<&str> = (0..32).map(|i| libretable_mau_mau::card(i).key()).collect();
    assert_eq!(listed, deck);
}
