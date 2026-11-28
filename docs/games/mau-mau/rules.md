# Mau-Mau

Game ID `mau-mau`. A shedding game for 2 to 5 players (up to 10 with two decks): match the top card of the discard pile by suit or rank, and be the first to get rid of all your cards. Signed off by the architect on 2026-11-28; amended the same day with the architect so that keeping a drawn card is always offered.

## Sources and licence

Mau-Mau is a traditional German card game of the Crazy Eights family. No authoritative rulebook exists; this spec follows the rules common to the German and English Wikipedia articles ([de](https://de.wikipedia.org/wiki/Mau-Mau_(Kartenspiel)), [en](https://en.wikipedia.org/wiki/Mau-Mau_(card_game))) and the architect's choices where they differ. The game is in the public domain. Commercial relatives (Uno and others) are references only; nothing of their names, cards or artwork is used.

## Components

- One 32-card French-suited deck: suits clubs, spades, hearts and diamonds; ranks 7, 8, 9, 10, Jack, Queen, King, Ace. With the option `decks = 2`, two such decks (64 cards); equal cards are distinct components.
- Zones: a face-down **draw pile**, a face-up **discard pile**, one **hand** per seat.
- German-suited decks (Eichel, Grün, Rot, Schellen; Unter instead of Jack) are a later theme over the same cards, not a rules change.

## Standard ruleset

### Setup

1. Shuffle the deck and deal `hand_size` (default 5) cards to each seat, one at a time, starting with seat 0.
2. Turn the top card of the remaining pile face up as the first discard. The rest is the draw pile.
3. Seat 0 takes the first turn; play goes clockwise (ascending seat order). In a match of several rounds, round *r* (counting from 0) starts at seat *r* mod seats, clockwise.

The start card takes effect as if seat 0's predecessor had played it ([special cards](#special-cards)): a 7 makes seat 0 draw two or counter, an 8 skips seat 0, a Jack lets any suit follow without a wish (the Jack-on-Jack rule still applies). A start Ace has no effect; a start 9 (with `nine_reverses`) reverses the direction before seat 0 moves.

### A turn

On their turn a player does exactly one of:

- **Play** one card from hand that fits the top discard: same suit or same rank. A Jack fits anything except another Jack ([Jack](#special-cards)). If a suit is wished, only that suit (or, with `jack_on_jack`, a Jack) fits.
- **Draw** one card from the draw pile. This is always allowed, even with a playable card. Then the player plays the drawn card if it fits (only that card) or keeps it, which ends the turn. Keeping is offered even when the card does not fit, so whether a hidden drawn card fits never shows in whose turn it is.
- **Pass**, only when nothing can be drawn (draw pile and reshuffle both empty). Passing is then allowed even with a playable card.

A player whose hand becomes empty has gone out; the round ends at once.

### Special cards

| Card | Effect |
| --- | --- |
| 7 | The next player must draw two cards and their turn ends, or counter with a 7 of their own (`seven_stacking`, default on). A counter adds two to the open penalty and passes it on (4, 6, …). Only a 7 counters; a Jack does not. Without `seven_stacking` the next player always draws two and their turn ends. Once drawn, the penalty is consumed and the 7 is an ordinary top card. |
| 8 | The next player is skipped. With two players the player of the 8 moves again. With `eight_counter` (default off) the skipped player may instead play an 8 of their own and pass the skip on; otherwise they accept it. |
| Jack | Wild: fits any card except a Jack (`jack_on_jack`, default off, allows Jack on Jack). The player wishes a suit (one of four, its own included); the wish is mandatory. The wish holds until a card of the wished suit is played; drawing does not end it. |
| Ace | Only with `ace_again` (default off): the same player takes another turn at once, with the normal choices (play, which may be another Ace; draw and maybe play; pass). |
| 9 | Only with `nine_reverses` (default off): the direction of play reverses. With two players this changes nothing. |

A player may go out on any card, a special one included. With `jack_finish` off, a Jack cannot be the last card played. Going out on a 7 ends the round at once; under `card-count` scoring the next player still draws the open penalty before the hands are counted (no counter).

### Drawing and reshuffling

When a draw is needed and the draw pile is empty, every discard except the top one is shuffled to form a new draw pile, with fresh opaque IDs. A penalty that exceeds the cards available is drawn as far as possible; the rest is dropped. When nothing can be drawn at all, a draw is not possible and the player may pass instead.

### Last-card announcement

Option `announce`, default `auto`.

- **`auto`:** when a play leaves a player with one card, the game announces it (event and indicator). Nobody can forget, and there is no penalty.
- **`manual`:** the player has to call "last card" ("Letzte Karte") themselves, and nobody but the next player may notice when they forget.
  - **Call** is a move of its own, available at the start of every own turn, before playing or drawing, once per turn, whether or not it is needed. It is always offered, so its presence gives nothing away.
  - **Missed call:** a turn in which a play left exactly one card in hand and no call was made. **False call:** a turn with a call in which no play left exactly one card. A player who already holds one card does not call again. Going out needs no call.
  - **Catch** is a move of its own for the player who takes the next turn, available at the start of that turn, before playing or drawing, once per turn, always offered. If the previous turn ended with a missed or false call, the offender draws `mau_penalty` cards (default 2). Otherwise the catcher draws them. Either way the catcher's turn continues.
  - The window closes when the next player plays, draws, passes or accepts a skip. An uncaught fault goes unpunished.
  - Turns one seat takes in a row (an Ace with `ace_again`, an 8 with two players) count as one turn for calling and catching. The call is offered at the start of each of them but made once in all. Catch is offered only at the start of a turn that follows another seat's turn.
  - Catching by any seat in real time (`Pending::Any`) is planned once drivers can handle "act or wait"; until then only the next player catches.

### End of the round

- A player goes out: they win the round.
- **Stalemate:** every seat passes in a row (nothing to draw, nothing played).
- **Turn cap:** the round reaches 100 × seats turns, a constant, not an option. (A turn is one seat's completed turn; a skipped seat has none.)

After a stalemate or the cap, no one wins the round.

### Scoring and end of the match

Option `rounds` (1 to 20, default 1); option `scoring`, default `win-lose`.

- **`win-lose`:** a round's winner gets one round win. One round: the winner ranks 1, all others share rank 2. Without a winner (stalemate, cap), fewer cards in hand ranks better, ties share. Several rounds: rank by rounds won, ties share; the score is rounds won.
- **`card-count`:** each card left in hand is one penalty point; the winner scores 0. Over all rounds the penalty points are summed; the lowest total ranks 1, ties share. The score is the penalty total.

## House-rule options

The config type is `Variant`; `Default` is the standard ruleset above.

| Option | Type | Default | Meaning |
| --- | --- | --- | --- |
| `decks` | 1..=2 | 1 | Number of 32-card decks. |
| `hand_size` | 3..=7 | 5 | Cards dealt to each seat. |
| `seven_stacking` | bool | true | A 7 may counter an open draw-two penalty. |
| `eight_counter` | bool | false | An 8 may pass a skip on. |
| `jack_on_jack` | bool | false | A Jack may be played on a Jack. |
| `jack_finish` | bool | true | A Jack may be the last card. |
| `ace_again` | bool | false | After an Ace the same player moves again. |
| `nine_reverses` | bool | false | A 9 reverses the direction of play. |
| `announce` | `auto` \| `manual` | `auto` | Who announces the last card ([above](#last-card-announcement)). |
| `mau_penalty` | 1..=2 | 2 | Cards drawn for a caught missed or false call or a false catch; no effect under `auto`. |
| `rounds` | 1..=20 | 1 | Rounds in a match. |
| `scoring` | `win-lose` \| `card-count` | `win-lose` | How rounds are scored. |

Validation (`validate(seats)`) rejects:

- fewer than 2 seats, more than 5 seats with one deck, or more than 10 with two;
- `seats × hand_size + 1 + seats > 32 × decks`: after the deal and the start card, the draw pile must hold at least one card per seat (5 seats × 5 cards is valid, 5 × 6 is not).

## Moves and choice paths

Choice paths are unique and prefix-free for one seat at one point ([game-core](../../design/game-core.md#selection-protocol)).

| Move | Path | When |
| --- | --- | --- |
| Play a card | `component(card)` | The card fits. |
| Play a Jack | `component(jack)`, `option(mau-mau-wish-<suit>)` | The Jack fits; four wish options. |
| Draw | `zone(draw pile)` | A draw is possible. Also takes an open 7 penalty. |
| Play the drawn card | `component(card)` (Jack: plus wish) | Right after a draw, the drawn card fits. |
| End turn after drawing | `option(mau-mau-keep)` | Right after a draw, always. |
| Pass | `option(mau-mau-pass)` | Nothing can be drawn. |
| Accept a skip | `option(mau-mau-accept-skip)` | Only with `eight_counter`, as the skipped player. Without it, a skipped seat gets no decision. |
| Call last card | `option(mau-mau-last-card)` | `manual`, start of own turn, not yet called this turn. |
| Catch | `option(mau-mau-catch)` | `manual`, start of own turn, not yet caught this turn. |

Exactly one seat decides at a time (`Pending::All` with one seat). Dealing happens in `setup`; draws use the `rng` passed to `apply`.

## Views and table

- A seat sees its own hand, the number of cards in every other hand, in the draw pile and in the discard pile, the top discard face up, and every indicator. A spectator sees the same without any hand.
- Only the top discard is shown. Every played card was public when it was played, but the view does not keep that history: remembering the pile is up to the player. A bot gets only the current view, so what it remembers of past views is the bot's own state and can scale with its difficulty level (bots design, issue 9).
- The draw pile, the discards below the top and other seats' hands are face down with opaque IDs. A card that becomes hidden (drawn, covered on the discard pile, reshuffled) gets a fresh opaque ID, and IDs are reissued whenever hidden cards are shuffled, so nobody can follow a card by its ID. Another seat's hand gets all-new opaque IDs whenever it changes, as if its owner had sorted it, so which hidden card was played or kept is not traceable either.
- The view also carries the public facts the table does not show but a match needs to go on (who called, whether a missed or false call is open to be caught, passes, turns, round results) and the seat's own just-drawn card, so a bot can rebuild a state from it ([bots](../../design/bots.md)). Nothing in it is hidden from anyone; the table does not show whether a fault is open, so humans still have to notice.
- Indicators: whose turn it is, the direction of play, an open draw penalty, the wished suit, last-card announcements, the round, and round wins or penalty points.
- Zones: `draw pile` (stack, hidden), `discard` (stack, public; only the top card face up), one `hand` per seat (fan, owner only).

## Example games (golden replays)

Each becomes a golden replay recorded during implementation, with its seed and moves.

1. **Two players, standard rules:** plays, a draw that is then played, a draw-two countered with a 7 (four drawn), a Jack with a wish, going out.
2. **Three players, reshuffle:** `hand_size = 7`, long enough to empty the draw pile and reshuffle; an 8 skip and a Jack wish that holds through a draw.
3. **Four players, house rules:** `announce = manual`, `ace_again`, `nine_reverses`, `scoring = card-count`, `rounds = 2`: a missed call caught, a false call caught, a false catch, an Ace extra turn and a reversal.

## Not in this version

Jacks doubling the score (Parlett), the Netherlands penalty of a whole new hand, Aces that must be covered, 36-card decks (Tschau Sepp), and catching by any seat in real time. Each can be added later as a typed option.

## Implementation

Crate `crates/games/mau-mau` (`libretable-mau-mau`), card types and names from `crates/cards`. Tests: `tests/rules.rs` (rules of this spec), `tests/conformance.rs` (the conformance suite over the standard rules and every house rule), `tests/golden.rs` with the example games above as records in `tests/golden/`. Bots and humans play it through the CLI (`table play mau-mau`) and the dev server.

