//! The server's request handling without networking: matches by hand and by
//! bot, the selection protocol, log stepping and rejections.

mod common;

use common::{Client, create, errors, last_state, play_out, rock};
use libretable_game_core::Choice;
use libretable_server::protocol::{
    ClientMessage, Driver, PendingKind, ServerMessage, WireObserver,
};
use libretable_server::{DEV, Server};

#[test]
fn hello_lists_games_and_formats_messages() {
    let server = Server::new("test");
    let mut client = Client::new(&server);
    for (locale, expected, rock) in [("en-GB", "en", "Rock"), ("de-AT", "de", "Stein")] {
        let replies = client.send(ClientMessage::Hello {
            locale: Some(locale.to_owned()),
        });
        let [
            ServerMessage::Welcome {
                protocol,
                dev,
                locale,
                locales,
                games,
                messages,
                ..
            },
        ] = replies.as_slice()
        else {
            panic!("{replies:?}");
        };
        assert_eq!(*protocol, 1);
        assert_eq!(*dev, DEV);
        assert_eq!(locale, expected);
        assert_eq!(locales, &["en", "de"]);
        assert_eq!(games[0].id, "rock-paper-scissors");
        assert_eq!(games[0].variant, serde_json::json!({"rounds": 3}));
        assert_eq!(messages["rps-rock"], rock);
        // Arguments are left for the client to fill in.
        assert!(messages["server-page-seat"].contains("{$seat}"));
        for id in libretable_server::MESSAGE_IDS {
            assert!(messages.contains_key(*id), "{id}");
        }
    }
}

#[test]
fn a_match_by_hand_runs_to_the_end() {
    let server = Server::new("test");
    let mut client = Client::new(&server);
    let replies = client.send(create(2, None));
    let ServerMessage::Joined {
        match_id,
        game,
        seats,
        observers,
        ..
    } = &replies[0]
    else {
        panic!("{replies:?}");
    };
    assert_eq!(game, "rock-paper-scissors");
    assert_eq!(*seats, 2);
    assert_eq!(
        observers[..3],
        [
            WireObserver::Seat(0),
            WireObserver::Seat(1),
            WireObserver::Spectator
        ]
    );
    let state = last_state(&replies);
    assert_eq!(state.match_id, *match_id);
    assert_eq!((state.position, state.length, state.live), (0, 0, true));
    assert_eq!(state.pending.kind, PendingKind::All);
    assert_eq!(state.pending.seats, [0, 1]);
    assert_eq!(state.drivers, [Driver::Human, Driver::Human]);
    assert_eq!(state.prompts.len(), 2);
    assert_eq!(state.prompts[0].selectable.len(), 3);

    let end = play_out(&mut client, state);
    assert_eq!(end.length, 6, "three rounds of two throws");
    assert_eq!(end.position, 6);
    assert_eq!(end.pending.kind, PendingKind::Finished);
    assert!(end.prompts.is_empty());
    assert_eq!(end.log.len(), 6);
    assert!(end.log.iter().all(|entry| entry.choices.is_some()));
    let outcome = end.outcome.expect("finished");
    assert_eq!(outcome.seats.len(), 2);
    assert_eq!(end.record.is_some(), DEV);
}

#[test]
fn choices_can_be_undone_and_bad_ones_are_rejected() {
    let server = Server::new("test");
    let mut client = Client::new(&server);
    assert_eq!(
        errors(&client.send(ClientMessage::Undo { seat: 0 })),
        ["server-error-no-match"]
    );
    client.send(create(2, None));

    let chosen = client.send(ClientMessage::Choose {
        seat: 0,
        choice: rock(),
    });
    let state = last_state(&chosen);
    assert_eq!(state.prompts[0].made, [rock()]);
    assert!(state.prompts[0].complete);
    assert!(state.prompts[0].selectable.is_empty());

    let undone = last_state(&client.send(ClientMessage::Undo { seat: 0 }));
    assert!(undone.prompts[0].made.is_empty());
    assert!(!undone.prompts[0].complete);

    for (message, code) in [
        (ClientMessage::Submit { seat: 0 }, "server-error-incomplete"),
        (
            ClientMessage::Choose {
                seat: 0,
                choice: Choice::Option("rps-lizard".to_owned()),
            },
            "server-error-not-selectable",
        ),
        (
            ClientMessage::Choose {
                seat: 5,
                choice: rock(),
            },
            "server-error-bad-seat",
        ),
        (
            ClientMessage::Seek { position: Some(1) },
            "server-error-bad-position",
        ),
    ] {
        assert_eq!(errors(&client.send(message)), [code]);
    }

    // After seat 0 threw, it may not act until seat 1 has.
    client.send(ClientMessage::Choose {
        seat: 0,
        choice: rock(),
    });
    client.send(ClientMessage::Submit { seat: 0 });
    assert_eq!(
        errors(&client.send(ClientMessage::Choose {
            seat: 0,
            choice: rock(),
        })),
        ["server-error-not-pending"]
    );
}

#[test]
fn bots_play_their_seats_and_can_take_over() {
    let server = Server::new("test");

    let mut bots = Client::new(&server);
    let state = last_state(&bots.send(create(2, Some(vec![Driver::Bot, Driver::Bot]))));
    assert_eq!(state.length, 6, "a bot-only match ends at once");
    assert!(state.outcome.is_some());

    let mut mixed = Client::new(&server);
    let state = last_state(&mixed.send(create(2, Some(vec![Driver::Human, Driver::Bot]))));
    assert_eq!(state.length, 1, "the bot threw first");
    assert_eq!(state.pending.seats, [0]);
    assert_eq!(state.prompts.len(), 1);
    assert_eq!(play_out(&mut mixed, state).length, 6);

    let mut switch = Client::new(&server);
    switch.send(create(2, None));
    let state = last_state(&switch.send(ClientMessage::SetDriver {
        seat: 0,
        driver: Driver::Bot,
    }));
    assert_eq!(state.drivers, [Driver::Bot, Driver::Human]);
    assert_eq!(state.length, 1);
    assert_eq!(state.pending.seats, [1]);
    assert_eq!(
        errors(&switch.send(ClientMessage::Choose {
            seat: 0,
            choice: rock(),
        })),
        ["server-error-seat-is-bot"]
    );
    let state = last_state(&switch.send(ClientMessage::SetDriver {
        seat: 1,
        driver: Driver::Bot,
    }));
    assert_eq!(state.length, 6);
}

#[test]
fn bot_matches_are_reproducible_from_the_seed() {
    let server = Server::new("test");
    let logs: Vec<_> = (0..2)
        .map(|_| {
            let mut client = Client::new(&server);
            last_state(&client.send(create(3, Some(vec![Driver::Bot; 3])))).log
        })
        .collect();
    assert_eq!(logs[0], logs[1]);
}

#[test]
fn the_log_can_be_stepped_through() {
    let server = Server::new("test");
    let mut client = Client::new(&server);
    let live = last_state(&client.send(create(2, Some(vec![Driver::Bot, Driver::Bot]))));

    let start = last_state(&client.send(ClientMessage::Seek { position: Some(0) }));
    assert_eq!((start.position, start.length, start.live), (0, 6, false));
    assert!(start.outcome.is_none());
    assert!(start.prompts.is_empty());
    assert_eq!(start.pending.seats, [0, 1]);
    assert_eq!(start.log, live.log, "the whole log stays visible");

    let middle = last_state(&client.send(ClientMessage::Seek { position: Some(2) }));
    assert_eq!(middle.position, 2);
    assert_ne!(middle.views, start.views);

    // A pinned position stays put; `null` returns to the live match.
    let end = last_state(&client.send(ClientMessage::Seek { position: Some(6) }));
    assert_eq!((end.position, end.live), (6, false));
    assert_eq!(end.views, live.views);
    let back = last_state(&client.send(ClientMessage::Seek { position: None }));
    assert_eq!(back, live);
}

#[test]
fn other_connections_follow_the_match_and_see_only_their_seats_moves() {
    let server = Server::new("test");
    let mut host = Client::new(&server);
    let replies = host.send(create(2, None));
    let ServerMessage::Joined { match_id, .. } = replies[0] else {
        panic!("{replies:?}");
    };

    let mut guest = Client::new(&server);
    let joined = guest.send(ClientMessage::Join {
        match_id,
        observers: Some(vec![WireObserver::Seat(1)]),
    });
    let state = last_state(&joined);
    assert_eq!(state.views.len(), 1);
    assert_eq!(state.prompts.len(), 1);
    assert_eq!(state.prompts[0].seat, 1);

    host.send(ClientMessage::Choose {
        seat: 0,
        choice: rock(),
    });
    host.send(ClientMessage::Submit { seat: 0 });
    let seen = last_state(&guest.receive());
    assert_eq!(seen.length, 1);
    assert_eq!(seen.log[0].seat, 0);
    assert_eq!(seen.log[0].choices, None, "seat 0's throw is not seat 1's");

    guest.send(ClientMessage::Choose {
        seat: 1,
        choice: rock(),
    });
    let seen = last_state(&guest.send(ClientMessage::Submit { seat: 1 }));
    assert_eq!(seen.log[1].choices, Some(vec![rock()]));
    assert_eq!(last_state(&host.receive()).length, 2);

    assert_eq!(
        errors(&guest.send(ClientMessage::Join {
            match_id: 999,
            observers: None
        })),
        ["server-error-unknown-match"]
    );
}

#[test]
fn bad_requests_are_rejected() {
    let server = Server::new("test");
    let mut client = Client::new(&server);
    let create_with =
        |game: &str, seats: u8, variant: Option<serde_json::Value>| ClientMessage::Create {
            game: game.to_owned(),
            seats,
            seed: None,
            variant,
            drivers: None,
            observers: None,
        };
    assert_eq!(
        errors(&client.send(create_with("chess", 2, None))),
        ["server-error-unknown-game"]
    );
    assert_eq!(
        errors(&client.send(create_with("rock-paper-scissors", 0, None))),
        ["server-error-bad-seats"]
    );
    assert_eq!(
        errors(&client.send(create_with(
            "rock-paper-scissors",
            2,
            Some(serde_json::json!({"rounds": "many"}))
        ))),
        ["server-error-variant-json"]
    );
    let invalid = client.send(create_with(
        "rock-paper-scissors",
        2,
        Some(serde_json::json!({"rounds": 0})),
    ));
    let [ServerMessage::Error(error)] = invalid.as_slice() else {
        panic!("{invalid:?}");
    };
    assert_eq!(error.code, "server-error-invalid-variant");
    assert_eq!(error.reason.as_deref(), Some("rps-no-rounds"));
    assert_eq!(
        errors(&client.send(create(2, Some(vec![Driver::Bot])))),
        ["server-error-drivers"]
    );
    assert_eq!(
        errors(&client.send(common::create_observing(vec![WireObserver::Seat(2)]))),
        ["server-error-bad-seat"]
    );

    assert_eq!(
        errors(&client.send_text(r#"{"v": 2, "type": "hello"}"#)),
        ["server-error-protocol-version"]
    );
    for text in [
        "not json",
        r#"{"type": "hello"}"#,
        r#"{"v": 1, "type": "dance"}"#,
    ] {
        assert_eq!(
            errors(&client.send_text(text)),
            ["server-error-bad-message"],
            "{text}"
        );
    }
    assert!(client.connection_is_idle());
}

trait Idle {
    fn connection_is_idle(&mut self) -> bool;
}

impl Idle for Client {
    /// Rejected requests created no match.
    fn connection_is_idle(&mut self) -> bool {
        errors(&self.send(ClientMessage::Seek { position: None })) == ["server-error-no-match"]
    }
}
