# The server `table-server` (crates/server) and its dev multiclient page
# (apps/web/dev). Source locale. Message IDs: libretable_server::MESSAGE_IDS.
# The page receives these messages formatted without arguments and fills in
# `{$name}` itself, so page messages use no selectors (docs/design/server.md).

## Console

server-usage =
    Usage:
    table-server [--listen <address>]
    {"    "}Run the dev server (default address: 127.0.0.1:8080).
    The locale comes from LC_ALL, LC_MESSAGES or LANG, else English.
server-listening = Dev server listening on { $url }
server-dev-build = Dev build: the omniscient view, records and events are served.
server-bind-failed = Cannot listen on { $address }: { $reason }
server-bad-argument = Unknown arguments: { $arguments }

## Errors sent to clients

server-error-protocol-version = This server speaks protocol version { $expected }, not { $version }.
server-error-bad-message = Malformed message: { $detail }
server-error-unknown-game = Unknown game "{ $game }".
server-error-bad-seats = A match needs 1 to 64 seats, not { $seats }.
server-error-drivers = There are { $seats } seats but { $drivers } drivers.
server-error-variant-json = The variant is not valid for this game: { $detail }
server-error-invalid-variant = Invalid variant: { $reason }
server-error-unknown-match = There is no match { $match }.
server-error-no-match = Create or join a match first.
server-error-bad-seat = Seat { $seat } is not in this match.
server-error-seat-is-bot = Seat { $seat } is played by a bot.
server-error-no-ismcts = This game has no ISMCTS bot.
server-error-not-pending = Seat { $seat } may not act now.
server-error-not-selectable = That choice is not selectable.
server-error-incomplete = The choices do not form a complete move yet.
server-error-ambiguous = The game's choice paths are ambiguous.
server-error-illegal = Illegal move: { $reason }
server-error-bad-position = Move { $position } does not exist; the match has { $length } moves.
server-error-omniscient-unavailable = The omniscient view is only available in dev builds.
server-error-internal = Internal error: { $detail }

## Web page

server-page-title = Dev multiclient
server-page-connecting = Connecting…
server-page-connected = Connected
server-page-disconnected = Disconnected. Reload the page to reconnect.
server-page-dev-build = Dev build
server-page-language = Language
server-page-game = Game
server-page-seats = Seats
server-page-seed = Seed
server-page-seed-random = random
server-page-variant = Variant (JSON)
server-page-create = New match
server-page-match-id = Match number
server-page-join = Join
server-page-match = Match { $match }: { $game }
server-page-seat = Seat { $seat }
server-page-spectator = Spectator
server-page-omniscient = Omniscient
server-page-driver = Played by
server-page-driver-human = Human
server-page-driver-bot = Bot
server-page-driver-ismcts = ISMCTS bot
server-page-to-act = To act
server-page-waiting = Waiting
server-page-chosen = Chosen:
server-page-nothing-chosen = nothing yet
server-page-choose = Choose:
server-page-complete = Complete; submit or undo.
server-page-undo = Undo
server-page-submit = Submit
server-page-owner = seat { $seat }
server-page-empty = (empty)
server-page-face-down = face down
server-page-log = Move log
server-page-log-start = Start
server-page-log-entry = { $number }. Seat { $seat }: { $choices }
server-page-log-hidden = hidden
server-page-first = First
server-page-previous = Back
server-page-next = Forward
server-page-last = Last
server-page-live = Live
server-page-position = After move { $position } of { $length }
server-page-history = Showing an earlier position; input is off until you return to live.
server-page-outcome = Result
server-page-outcome-seat = Seat { $seat }: rank { $rank }, score { $score }
server-page-record = Game record
server-page-download-record = Download record
server-page-error = Error: { $message }
