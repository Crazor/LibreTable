//! The wire format: exact JSON of each message kind and lossless round
//! trips (ADR-0010).

use std::collections::BTreeMap;

use libretable_game_core::{Choice, ComponentId, IndicatorValue, SeatResult, TableModel};
use libretable_server::protocol::{
    ClientMessage, DecodeError, Driver, ErrorMessage, GameInfo, LogEntry, MatchState, ObserverView,
    PendingKind, ServerMessage, WireObserver, WireOutcome, WirePending, WirePrompt, decode, encode,
};
use serde_json::{Value, json};

fn round_trip_client(message: &ClientMessage, expected: &Value) {
    let text = encode(message);
    assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), *expected);
    assert_eq!(decode::<ClientMessage>(&text).as_ref(), Ok(message));
}

fn round_trip_server(message: &ServerMessage, expected: &Value) {
    let text = encode(message);
    assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), *expected);
    assert_eq!(decode::<ServerMessage>(&text).as_ref(), Ok(message));
}

#[test]
fn client_messages() {
    round_trip_client(
        &ClientMessage::Hello {
            locale: Some("de-DE".to_owned()),
        },
        &json!({"v": 1, "type": "hello", "locale": "de-DE"}),
    );
    round_trip_client(
        &ClientMessage::Hello { locale: None },
        &json!({"v": 1, "type": "hello"}),
    );
    round_trip_client(
        &ClientMessage::Create {
            game: "rock-paper-scissors".to_owned(),
            seats: 2,
            seed: Some(42),
            variant: Some(json!({"rounds": 1})),
            drivers: Some(vec![Driver::Human, Driver::Bot]),
            observers: Some(vec![
                WireObserver::Seat(0),
                WireObserver::Spectator,
                WireObserver::Omniscient,
            ]),
        },
        &json!({
            "v": 1, "type": "create", "game": "rock-paper-scissors", "seats": 2,
            "seed": 42, "variant": {"rounds": 1}, "drivers": ["human", "bot"],
            "observers": [{"seat": 0}, "spectator", "omniscient"],
        }),
    );
    round_trip_client(
        &ClientMessage::Join {
            match_id: 3,
            observers: None,
        },
        &json!({"v": 1, "type": "join", "match": 3}),
    );
    round_trip_client(
        &ClientMessage::Choose {
            seat: 1,
            choice: Choice::Option("rps-rock".to_owned()),
        },
        &json!({"v": 1, "type": "choose", "seat": 1, "choice": {"option": "rps-rock"}}),
    );
    round_trip_client(
        &ClientMessage::Choose {
            seat: 0,
            choice: Choice::Subset([ComponentId(4), ComponentId(2)].into_iter().collect()),
        },
        &json!({"v": 1, "type": "choose", "seat": 0, "choice": {"subset": [2, 4]}}),
    );
    round_trip_client(
        &ClientMessage::Undo { seat: 0 },
        &json!({"v": 1, "type": "undo", "seat": 0}),
    );
    round_trip_client(
        &ClientMessage::Submit { seat: 0 },
        &json!({"v": 1, "type": "submit", "seat": 0}),
    );
    round_trip_client(
        &ClientMessage::SetDriver {
            seat: 1,
            driver: Driver::Bot,
        },
        &json!({"v": 1, "type": "set-driver", "seat": 1, "driver": "bot"}),
    );
    round_trip_client(
        &ClientMessage::Seek { position: Some(4) },
        &json!({"v": 1, "type": "seek", "position": 4}),
    );
    round_trip_client(
        &ClientMessage::Seek { position: None },
        &json!({"v": 1, "type": "seek"}),
    );
}

#[test]
fn optional_fields_may_be_null_or_missing() {
    assert_eq!(
        decode::<ClientMessage>(r#"{"v":1,"type":"seek","position":null}"#),
        Ok(ClientMessage::Seek { position: None })
    );
    assert_eq!(
        decode::<ClientMessage>(r#"{"v":1,"type":"create","game":"g","seats":2,"seed":null}"#),
        Ok(ClientMessage::Create {
            game: "g".to_owned(),
            seats: 2,
            seed: None,
            variant: None,
            drivers: None,
            observers: None,
        })
    );
}

#[test]
fn server_messages() {
    round_trip_server(
        &ServerMessage::Welcome {
            protocol: 1,
            build: "b".to_owned(),
            dev: false,
            locale: "en".to_owned(),
            locales: vec!["en".to_owned(), "de".to_owned()],
            games: vec![GameInfo {
                id: "rock-paper-scissors".to_owned(),
                variant: json!({"rounds": 3}),
            }],
            messages: BTreeMap::from([("rps-rock".to_owned(), "Rock".to_owned())]),
        },
        &json!({
            "v": 1, "type": "welcome", "protocol": 1, "build": "b", "dev": false,
            "locale": "en", "locales": ["en", "de"],
            "games": [{"id": "rock-paper-scissors", "variant": {"rounds": 3}}],
            "messages": {"rps-rock": "Rock"},
        }),
    );
    round_trip_server(
        &ServerMessage::Joined {
            match_id: 1,
            game: "rock-paper-scissors".to_owned(),
            seats: 2,
            observers: vec![WireObserver::Seat(1)],
            seed: None,
        },
        &json!({
            "v": 1, "type": "joined", "match": 1, "game": "rock-paper-scissors",
            "seats": 2, "observers": [{"seat": 1}],
        }),
    );
    round_trip_server(
        &ServerMessage::Events {
            match_id: 1,
            position: 2,
            seat: 1,
            events: vec![json!({"Thrown": 1})],
        },
        &json!({
            "v": 1, "type": "events", "match": 1, "position": 2, "seat": 1,
            "events": [{"Thrown": 1}],
        }),
    );
    round_trip_server(
        &ServerMessage::Error(ErrorMessage {
            code: "server-error-illegal".to_owned(),
            args: BTreeMap::from([("seat".to_owned(), "1".to_owned())]),
            reason: Some("rps-not-pending".to_owned()),
        }),
        &json!({
            "v": 1, "type": "error", "code": "server-error-illegal",
            "args": {"seat": "1"}, "reason": "rps-not-pending",
        }),
    );
}

#[test]
fn state_message() {
    let table = TableModel {
        zones: Vec::new(),
        indicators: vec![libretable_game_core::Indicator {
            kind: libretable_game_core::IndicatorKind::Counter,
            label: "rps-indicator-round".to_owned(),
            owner: None,
            value: IndicatorValue::Number(1),
        }],
    };
    let state = MatchState {
        match_id: 1,
        position: 1,
        length: 2,
        live: false,
        pending: WirePending {
            kind: PendingKind::All,
            seats: vec![1],
        },
        drivers: vec![Driver::Human, Driver::Bot],
        views: vec![ObserverView {
            observer: WireObserver::Spectator,
            table,
        }],
        prompts: vec![WirePrompt {
            seat: 1,
            made: Vec::new(),
            selectable: vec![Choice::Option("rps-rock".to_owned())],
            complete: false,
        }],
        log: vec![
            LogEntry {
                seat: 0,
                choices: None,
            },
            LogEntry {
                seat: 1,
                choices: Some(vec![Choice::Component(ComponentId(3))]),
            },
        ],
        outcome: Some(WireOutcome {
            seats: vec![
                SeatResult { rank: 1, score: 2 },
                SeatResult { rank: 2, score: 0 },
            ],
            teams: vec![vec![0], vec![1]],
        }),
        record: None,
    };
    round_trip_server(
        &ServerMessage::State(state),
        &json!({
            "v": 1, "type": "state", "match": 1, "position": 1, "length": 2, "live": false,
            "pending": {"kind": "all", "seats": [1]},
            "drivers": ["human", "bot"],
            "views": [{"observer": "spectator", "table": {
                "zones": [],
                "indicators": [{"kind": "counter", "label": "rps-indicator-round",
                                "owner": null, "value": {"number": 1}}],
            }}],
            "prompts": [{"seat": 1, "made": [], "selectable": [{"option": "rps-rock"}],
                         "complete": false}],
            "log": [{"seat": 0}, {"seat": 1, "choices": [{"component": 3}]}],
            "outcome": {"seats": [{"rank": 1, "score": 2}, {"rank": 2, "score": 0}],
                        "teams": [[0], [1]]},
        }),
    );
}

#[test]
fn the_version_is_checked_before_the_message() {
    assert_eq!(
        decode::<ClientMessage>(r#"{"v": 2, "type": "teleport"}"#),
        Err(DecodeError::Version(2))
    );
    assert!(matches!(
        decode::<ClientMessage>(r#"{"type": "hello"}"#),
        Err(DecodeError::Malformed(_))
    ));
    assert!(matches!(
        decode::<ClientMessage>(r#"{"v": 1, "type": "teleport"}"#),
        Err(DecodeError::Malformed(_))
    ));
    assert!(
        matches!(
            decode::<ClientMessage>(
                r#"{"v": 1, "type": "choose", "seat": 0, "choice": {"subset": [4, 2]}}"#
            ),
            Err(DecodeError::Malformed(_))
        ),
        "subsets must be canonical"
    );
}
