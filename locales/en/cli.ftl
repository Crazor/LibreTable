# The command-line runner `table` (crates/cli).
# Source locale. Message IDs: libretable_cli::l10n::MESSAGE_IDS.

## Help

cli-usage =
    Usage:
    table [--lang <locale>] games
    {"    "}List the games.
    table [--lang <locale>] play <game> [--seats human,bot,...] [--seed <n>] [--variant <json>] [--record <file>]
    {"    "}Play a match in the terminal. Each seat is human or bot (default: human,bot).
    table [--lang <locale>] replay <record.json> [--seat <n>] [--each]
    {"    "}Replay a game record, showing the table of a seat (default: spectator).
    table [--lang <locale>] simulate <game> [-n <matches>] [--seats <count>] [--seed <n>] [--variant <json>]
    {"    "}Play matches between random bots and summarize the results.
    table help
    {"    "}Show this help.
    Without --lang, the locale comes from LC_ALL, LC_MESSAGES or LANG, else English.
cli-games-header = Games:
cli-games-entry = { $id }: { $name }

## Errors

cli-error = Error: { $message }
cli-error-unknown-command = Unknown command "{ $command }".
cli-error-missing-argument = Missing argument: { $name }.
cli-error-unknown-option = Unknown option "{ $option }".
cli-error-option-value = Option { $option } needs a value.
cli-error-bad-number = { $option }: "{ $value }" is not a valid number.
cli-error-unknown-game = Unknown game "{ $game }". Run "table games" for the list.
cli-error-seat-kind = Unknown seat kind "{ $kind }"; use human or bot.
cli-error-seat-range = Seat { $seat } is not in this match.
cli-error-seat-count = A match needs 1 to 64 seats, not { $count }.
cli-error-variant-json = The variant is not valid for this game: { $reason }
cli-error-invalid-variant = Invalid variant: { $reason }
cli-error-read = Cannot read { $path }: { $reason }
cli-error-write = Cannot write { $path }: { $reason }
cli-error-io = Input/output error: { $reason }
cli-error-record-json = { $path } is not a valid game record: { $reason }
cli-error-replay-schema = The record uses schema version { $found }; this build reads version { $expected }.
cli-error-replay-wrong-game = The record belongs to another game.
cli-error-replay-not-pending = Move { $number } was recorded for a seat that could not act.
cli-error-replay-illegal = Move { $number } is illegal: { $reason }
cli-error-not-pending = Seat { $seat } may not act now.
cli-error-not-selectable = Choice { $number } is not selectable.
cli-error-incomplete = The choices do not form a complete move.
cli-error-ambiguous = The game's choice paths are ambiguous.
cli-error-illegal = Illegal move: { $reason }
cli-error-too-long = Match { $number } did not finish within { $moves } moves.

## Play

cli-play-header =
    Playing { $game } with { $seats ->
        [one] one seat
       *[other] { $seats } seats
    }, seed { $seed }.
cli-seat-human = Seat { $seat }: human
cli-seat-bot = Seat { $seat }: bot
cli-prompt = Seat { $seat }, choose:
cli-prompt-item = { $number }) { $choice }
cli-prompt-made = Chosen so far: { $choices }
cli-prompt-input = Enter a number, u to undo, q to quit:
cli-input-invalid = Please enter a number from 1 to { $max }, u or q.
cli-handover = Pass the device to seat { $seat } and press Enter.
cli-bot-acted = Seat { $seat } (bot) has decided.
cli-quit = Match abandoned.
cli-record-written = Record written to { $path }.

## Text renderer

cli-table-header = --- Table: { $observer } ---
cli-observer-seat = seat { $seat }
cli-observer-spectator = spectator
cli-indicator = { $label }: { $value }
cli-indicator-owned = { $label } ({ $owner }): { $value }
cli-zone = [{ $id }] { $label }: { $owner }, { $kind }, { $visibility }, { $layout }
cli-zone-empty = (empty)
cli-owner-seat = seat { $seat }
cli-owner-shared = shared
cli-zone-kind-draw-pile = draw pile
cli-zone-kind-hand = hand
cli-zone-kind-discard = discard pile
cli-zone-kind-trick = trick
cli-zone-kind-tableau = tableau
cli-zone-kind-player-board = player board
cli-zone-kind-score-sheet = score sheet
cli-zone-kind-board = board
cli-zone-kind-other = other
cli-visibility-public = public
cli-visibility-owner = owner only
cli-visibility-hidden = hidden
cli-layout-stack = stack
cli-layout-fan = fan
cli-layout-row = row
cli-layout-grid =
    grid of { $columns ->
        [one] one column
       *[other] { $columns } columns
    }
cli-layout-free = free
cli-component-kind-card = card
cli-component-kind-tile = tile
cli-component-kind-piece = piece
cli-component-kind-token = token
cli-component-kind-die = die
cli-component-kind-marker = marker
cli-component-hidden = #{ $id } { $kind }: face down
cli-component-shown = #{ $id } { $kind }: { $name }
cli-component-shown-attributes = #{ $id } { $kind }: { $name } ({ $attributes })
cli-attribute = { $name }: { $value }
cli-rotated = { $component }, rotated { $degrees } degrees
cli-choice-component = { $component } in { $zone }
cli-choice-zone = [{ $id }] { $zone }
cli-choice-subset = { $components }
cli-choice-none = (no choice needed)

## Results

cli-outcome-header = Result:
cli-outcome-seat = Seat { $seat }: rank { $rank }, score { $score }
cli-outcome-team = Team: { $seats }
cli-unfinished = The match is not finished.

## Replay and simulate

cli-replay-header =
    Replaying { $game } with { $seats ->
        [one] one seat
       *[other] { $seats } seats
    }, { $moves ->
        [one] one move
       *[other] { $moves } moves
    }.
cli-replay-move = Move { $number }: seat { $seat }: { $choices }
cli-simulate-summary =
    { $matches ->
        [one] One match
       *[other] { $matches } matches
    } of { $game } with { $seats ->
        [one] one seat
       *[other] { $seats } seats
    }, { $moves ->
        [one] one move
       *[other] { $moves } moves
    } in total.
cli-simulate-seat =
    Seat { $seat }: { $wins ->
        [one] one win
       *[other] { $wins } wins
    }, average score { $score }
