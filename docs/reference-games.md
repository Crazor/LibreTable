# Reference games

Looking at well-known games, classic and commercial, sharpens the engine's requirements. None of these games has to be implemented (licences); they are a checklist for the design. Add a row whenever a game brings a new requirement.

| Game | What it demands from the engine |
| --- | --- |
| Mensch ärgere dich nicht (Ludo/Pachisi) | Dice inside a turn (roll, then decide), up to three rolls; per-player paths on a shared track graph; capture |
| Malefiz | Dice; picking up a blocker and placing it on any free node: one choice step with a very large option set |
| Mühle (Nine Men's Morris) | Phases (place, move, fly); a capture as a follow-up choice; draws by repetition need position history |
| Dame (Checkers) | Mandatory capture, multi-jump chains as a path choice, promotion, many national variants |
| Reversi | "Pass" as a legal move; one move flips many pieces (events) |
| Siedler / Catan | Hex board with tiles, corners and edges as three kinds of locations; trading offers answered by several players out of turn; hidden hands with public counts; longest-road graph scoring; snake setup order |
| Catan card game | Personal tableau that grows in two dimensions |
| Risiko | Defender chooses dice during the attacker's turn; repeated attacks within one phase; numeric choices (move N armies); player elimination |
| Phase 10 | Per-player goals that advance individually; rummy melds |
| Monopoly | Money; auctions where all players bid; trading and building out of turn; elimination; very long games |
| Acquire | Hidden money and stock; growing tile chains on a grid; mergers where every affected player decides in order during one player's turn |
| Carcassonne | Unbounded board that grows with each tile; rotation; placing a follower on a part of a tile; feature completion via graph merging |
| Sternenfahrer von Catan | Custom randomizer (the mothership) instead of plain dice |
| El Grande | Secret simultaneous choices (castillo); area-majority scoring |
| Uno | Reaction windows: anyone may catch a missing "Uno" call; challenge of +4 by another player |
| Tactix (new edition of Parker's "Duell") | Chess-like game whose pieces are dice tipped over their edges: distance = the top face, which changes with every step; orientation is part of the piece's state and a move is a path (with a change of direction); perfect information, no chance |
| Abalone | Hex board; moving a group of one to three pieces in line or sideways; pushing |
| Skyjo | Each player's 3×4 grid of face-down cards that even the owner does not know; flip or swap by position; columns of three equal cards vanish; last-round trigger with one more turn for everyone else |
| Die Portale von Molthar | Action points per turn, hand limit, personal display of cards |
| Rummikub / Okey | A turn may rearrange every meld on the table: the move is a draft of the whole table that the core validates; bots need a meld solver |
| Ligretto / Nerts (Pounce) | Real time: everyone plays at once, races on shared piles, arrival order decides; reaction-time bots |
| Kniffel (Yacht, Generala) | Keep any subset of dice and re-roll up to twice; personal score sheet as a table component; choosing a scoring box |
| Schocken | Rolling hidden under a cup and revealing; the first player's number of rolls sets the limit for everyone; turning two sixes into a one; penalty tokens moving between players over two halves |
| Meiern (Mäxchen) | Hidden roll passed on with a claim that may be a lie; the next player believes and rolls, or doubts and reveals |
| Zehntausend (Farkle) | Press your luck: set aside scoring dice, then stop or keep rolling and risk the turn's points |

## Consequences for the engine

1. **Boards are typed graphs.** Locations of several kinds (squares, hexes, corners, edges, track spaces, territories) with adjacency, per-player paths and layout coordinates; boards may grow without bounds (Carcassonne) and the renderer must pan and zoom.
2. **Visibility per component, not per zone.** A card can be hidden from its own owner (Skyjo), public in count but hidden in content (Catan hands), or hidden as a number (Acquire money).
3. **Out-of-turn decisions and reaction windows.** `pending` changes inside a turn (defender dice, merger decisions, trade answers, Kontra). Optional reactions (catching a missing "Uno") get a default move; timeouts live in the session, not the core.
4. **Numeric and large choices.** The selection protocol supports bounded numbers (armies, bids, money), subsets (dice to keep) and very large option sets (any free node). `legal_moves` cannot always be a full list: the core also offers lazy enumeration and validation of a submitted draft.
5. **Negotiation and claims.** Open-ended offers between players (trading, auctions) and claims that may be lies (Meiern) are moves like any other but have no fixed option list; bots need dedicated logic here.
6. **Seat lifecycle.** Players can be eliminated and become spectators; games can last hours, so long matches are saved as snapshots plus the move log, not replayed from the start every time.
7. **Game history in the state.** Repetition draws (Mühle, Dame) and last-round triggers (Skyjo) need history or flags inside the state.
8. **Shared helpers.** Graph scoring (longest road, area majority, feature completion), dice and custom randomizers, money and resource counters, component rotation and orientation.

## Candidates to look at later

Games that would add further requirements; not yet analysed in depth.

- **Hanabi, The Crew** — cooperative play; you see everyone's cards except your own; communication only through restricted hint moves.
- **The Mind** — real time without any communication; waiting is the move.
- **Coup, Stratego** — claimed roles and challenges; identities known only to the owner until revealed.
- **Sushi Go / 7 Wonders** — drafting: everyone picks simultaneously and passes the rest of the hand.
- **RoboRally** — everyone programs moves secretly, then they resolve in order.
- **Scotland Yard** — hidden movement, asymmetric roles, scheduled reveals.
- **Battleship** — simultaneous hidden setup, then alternating play.
- **Backgammon** — doubling cube: an offer the opponent must accept or refuse mid-game.
- **Blokus, Santorini, Hive** — polyomino placement with mirroring; building heights; boardless adjacency with stacking.
- **Dominion** — per-player decks; hundreds of cards with effects (cards as data vs. code).
- **Scrabble** — dictionaries and per-language tile distributions: rules that depend on the language.
- **Memory (Pairs)** — bots that must forget on purpose.
- **Patience games** — single player, undo, solvers for hints, daily seeds.
- **Qwixx, Can't Stop, Heckmeck (Pickomino), Bluff** — commercial dice games: shared rolls used by all players, press-your-luck on a board, stealing tiles.
