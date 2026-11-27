//! The server end to end over a real socket on localhost: the page is
//! served, and a whole rock-paper-scissors match is played over `/ws`.
//!
//! Binding a port may be refused inside a sandbox (EPERM); run these tests
//! outside it then.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use libretable_server::protocol::{
    ClientMessage, Driver, MatchState, ServerMessage, WireObserver, decode, encode,
};
use libretable_server::{DEV, Server, serve};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

type Socket = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;

async fn start() -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind localhost");
    let address = listener.local_addr().unwrap();
    tokio::spawn(serve(listener, Server::new("test")));
    address
}

async fn get(address: std::net::SocketAddr, path: &str) -> String {
    let mut stream = TcpStream::connect(address).await.unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response
}

async fn connect(address: std::net::SocketAddr) -> Socket {
    let (socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}/ws"))
        .await
        .expect("WebSocket handshake");
    socket
}

async fn send(socket: &mut Socket, message: &ClientMessage) {
    socket.send(Message::text(encode(message))).await.unwrap();
}

async fn next(socket: &mut Socket) -> ServerMessage {
    let frame = tokio::time::timeout(Duration::from_secs(10), socket.next())
        .await
        .expect("a message within 10 s")
        .expect("socket open")
        .expect("valid frame");
    decode(frame.to_text().expect("text frame")).expect("protocol message")
}

async fn next_state(socket: &mut Socket) -> MatchState {
    loop {
        match next(socket).await {
            ServerMessage::State(state) => return state,
            ServerMessage::Error(error) => panic!("{error:?}"),
            _ => {}
        }
    }
}

#[tokio::test]
async fn the_page_and_its_files_are_served() {
    let address = start().await;
    let page = get(address, "/").await;
    assert!(page.starts_with("HTTP/1.1 200"), "{page}");
    assert!(page.contains("text/html"));
    assert!(page.contains("<script src=\"app.js\">"));
    let script = get(address, "/app.js").await;
    assert!(script.starts_with("HTTP/1.1 200"));
    assert!(script.contains("text/javascript"));
    let style = get(address, "/style.css").await;
    assert!(style.contains("text/css"));
    assert!(get(address, "/nothing").await.starts_with("HTTP/1.1 404"));
}

#[tokio::test]
async fn a_match_is_played_over_the_websocket() {
    let address = start().await;
    let mut host = connect(address).await;

    send(
        &mut host,
        &ClientMessage::Hello {
            locale: Some("de".to_owned()),
        },
    )
    .await;
    let ServerMessage::Welcome { dev, messages, .. } = next(&mut host).await else {
        panic!("welcome first");
    };
    assert_eq!(dev, DEV);
    assert_eq!(messages["rps-paper"], "Papier");

    send(
        &mut host,
        &ClientMessage::Create {
            game: "rock-paper-scissors".to_owned(),
            seats: 2,
            seed: Some(11),
            variant: None,
            drivers: None,
            observers: None,
        },
    )
    .await;
    let ServerMessage::Joined { match_id, .. } = next(&mut host).await else {
        panic!("joined after create");
    };
    next_state(&mut host).await;

    // A second client watches seat 1 only and plays it by bot.
    let mut guest = connect(address).await;
    send(
        &mut guest,
        &ClientMessage::Join {
            match_id,
            observers: Some(vec![WireObserver::Seat(1)]),
        },
    )
    .await;
    assert!(matches!(
        next(&mut guest).await,
        ServerMessage::Joined { .. }
    ));
    assert_eq!(next_state(&mut guest).await.views.len(), 1);
    send(
        &mut guest,
        &ClientMessage::SetDriver {
            seat: 1,
            driver: Driver::Bot,
        },
    )
    .await;
    assert_eq!(next_state(&mut guest).await.length, 1);
    let mut state = next_state(&mut host).await;
    assert_eq!(state.drivers, [Driver::Human, Driver::Bot]);

    // The host plays seat 0 by hand through the selection protocol.
    while state.outcome.is_none() {
        let prompt = state.prompts.first().expect("seat 0 to act").clone();
        assert_eq!(prompt.seat, 0);
        let message = if prompt.complete {
            ClientMessage::Submit { seat: 0 }
        } else {
            ClientMessage::Choose {
                seat: 0,
                choice: prompt.selectable[0].clone(),
            }
        };
        send(&mut host, &message).await;
        state = next_state(&mut host).await;
    }
    assert_eq!(state.length, 6);
    assert_eq!(state.record.is_some(), DEV);

    // Step back to the start and return to live.
    send(&mut host, &ClientMessage::Seek { position: Some(0) }).await;
    let start = next_state(&mut host).await;
    assert_eq!((start.position, start.live), (0, false));
    send(&mut host, &ClientMessage::Seek { position: None }).await;
    assert_eq!(next_state(&mut host).await, state);

    // The guest followed along.
    let mut seen = next_state(&mut guest).await;
    while seen.length < 6 {
        seen = next_state(&mut guest).await;
    }
    assert!(seen.outcome.is_some());
}

#[tokio::test]
async fn the_omniscient_view_needs_a_dev_build() {
    let address = start().await;
    let mut socket = connect(address).await;
    send(
        &mut socket,
        &ClientMessage::Create {
            game: "rock-paper-scissors".to_owned(),
            seats: 2,
            seed: Some(1),
            variant: None,
            drivers: None,
            observers: Some(vec![WireObserver::Omniscient]),
        },
    )
    .await;
    match next(&mut socket).await {
        ServerMessage::Joined { observers, .. } if DEV => {
            assert_eq!(observers, [WireObserver::Omniscient]);
        }
        ServerMessage::Error(error) if !DEV => {
            assert_eq!(error.code, "server-error-omniscient-unavailable");
        }
        other => panic!("{other:?}"),
    }
}
