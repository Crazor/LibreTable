//! `GameRng`: the only source of randomness for games.
//!
//! # Algorithm (`chacha12-v1`)
//!
//! The generator is the ChaCha stream cipher with 12 rounds (Bernstein's
//! original layout), keyed by the 32-byte [`Seed`]:
//!
//! - Input block: words 0–3 are the constants `"expand 32-byte k"`, words
//!   4–11 the seed read as eight little-endian `u32`, words 12–13 a 64-bit
//!   block counter (low word first) starting at 0, words 14–15 are 0.
//! - Each block yields 16 output words (the ChaCha block function: 6 double
//!   rounds, then the input added word-wise), consumed in order 0..16.
//! - [`GameRng::next_u32`] returns the next word. [`GameRng::next_u64`]
//!   combines two words, the first as the low half.
//! - [`GameRng::below`] maps to `0..bound` without bias by Lemire's method:
//!   `m = next_u32 * bound` (as `u64`); while the low 32 bits of `m` are below
//!   `2^32 mod bound`, draw again; return the high 32 bits.
//! - [`GameRng::shuffle`] is Fisher–Yates from the back: for `i` from `len-1`
//!   down to 1, swap item `i` with item `below(i + 1)`.
//!
//! The same seed gives the same sequence on every platform. Any change to the
//! steps above changes every replay, so it requires a deliberate decision
//! and a new [`GameRng::ALGORITHM`] name. ChaCha is used instead of a smaller PRNG so
//! that players cannot reconstruct the generator from the cards they see and
//! predict future draws.

use core::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

const ROUNDS: usize = 12;
const CONSTANTS: [u32; 4] = [0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574];
const BLOCK_WORDS: usize = 16;

/// The 256-bit seed of a match. The session picks it; a record stores it.
///
/// Serialized as 64 lowercase hex digits in human-readable formats (JSON) and
/// as 32 raw bytes otherwise.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Seed(pub [u8; 32]);

impl Seed {
    /// A seed from a small number, for tests, fixtures and the CLI: the
    /// number's 8 little-endian bytes followed by 24 zero bytes.
    #[must_use]
    pub const fn from_u64(n: u64) -> Self {
        let mut bytes = [0; 32];
        let le = n.to_le_bytes();
        let mut i = 0;
        while i < le.len() {
            bytes[i] = le[i];
            i += 1;
        }
        Self(bytes)
    }
}

impl fmt::Debug for Seed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Seed(")?;
        fmt::Display::fmt(self, f)?;
        f.write_str(")")
    }
}

impl fmt::Display for Seed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

impl Serialize for Seed {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.collect_str(self)
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for Seed {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            deserializer.deserialize_str(HexSeedVisitor)
        } else {
            <[u8; 32]>::deserialize(deserializer).map(Self)
        }
    }
}

struct HexSeedVisitor;

impl de::Visitor<'_> for HexSeedVisitor {
    type Value = Seed;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("64 lowercase hex digits")
    }

    fn visit_str<E: de::Error>(self, s: &str) -> Result<Seed, E> {
        fn nibble(c: u8) -> Option<u8> {
            match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                _ => None,
            }
        }
        let digits = s.as_bytes();
        if digits.len() != 64 {
            return Err(E::invalid_length(digits.len(), &self));
        }
        let mut bytes = [0; 32];
        for (byte, pair) in bytes.iter_mut().zip(digits.chunks_exact(2)) {
            match (nibble(pair[0]), nibble(pair[1])) {
                (Some(hi), Some(lo)) => *byte = hi << 4 | lo,
                _ => return Err(E::invalid_value(de::Unexpected::Str(s), &self)),
            }
        }
        Ok(Seed(bytes))
    }
}

/// Seeded, deterministic random number generator (`chacha12-v1`, see the
/// module documentation).
///
/// Owned by the session and handed to [`Game`](crate::Game) methods. It is
/// serializable so that snapshots can resume a match mid-stream. `Debug`
/// does not print the key.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "RngState", into = "RngState")]
pub struct GameRng {
    key: [u32; 8],
    /// Counter of the next block to generate.
    counter: u64,
    /// Output of block `counter - 1`.
    buffer: [u32; BLOCK_WORDS],
    /// Next unread word in `buffer`; `BLOCK_WORDS` when exhausted.
    index: usize,
}

impl GameRng {
    /// Name and version of the algorithm, for records and diagnostics.
    pub const ALGORITHM: &'static str = "chacha12-v1";

    /// Creates the generator for `seed`.
    #[must_use]
    pub fn from_seed(seed: Seed) -> Self {
        let mut key = [0; 8];
        for (word, chunk) in key.iter_mut().zip(seed.0.chunks_exact(4)) {
            *word = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        Self {
            key,
            counter: 0,
            buffer: [0; BLOCK_WORDS],
            index: BLOCK_WORDS,
        }
    }

    /// The next 32 random bits.
    pub fn next_u32(&mut self) -> u32 {
        if self.index == BLOCK_WORDS {
            self.buffer = chacha_block(&input_block(&self.key, self.counter), ROUNDS);
            self.counter = self.counter.wrapping_add(1);
            self.index = 0;
        }
        let word = self.buffer[self.index];
        self.index += 1;
        word
    }

    /// The next 64 random bits; the first word drawn is the low half.
    pub fn next_u64(&mut self) -> u64 {
        let low = u64::from(self.next_u32());
        let high = u64::from(self.next_u32());
        high << 32 | low
    }

    /// A uniformly distributed number in `0..bound`, without modulo bias.
    ///
    /// # Panics
    ///
    /// If `bound` is 0.
    pub fn below(&mut self, bound: u32) -> u32 {
        assert!(bound > 0, "GameRng::below called with bound 0");
        let mut m = u64::from(self.next_u32()) * u64::from(bound);
        let mut low = m as u32;
        if low < bound {
            let threshold = bound.wrapping_neg() % bound;
            while low < threshold {
                m = u64::from(self.next_u32()) * u64::from(bound);
                low = m as u32;
            }
        }
        (m >> 32) as u32
    }

    /// Shuffles `items` uniformly (Fisher–Yates).
    ///
    /// # Panics
    ///
    /// If `items` has more than `u32::MAX` elements.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        let len = u32::try_from(items.len()).expect("GameRng::shuffle: slice too long");
        for i in (1..len).rev() {
            let j = self.below(i + 1);
            items.swap(i as usize, j as usize);
        }
    }

    /// A uniformly chosen element, or `None` if `items` is empty.
    ///
    /// # Panics
    ///
    /// If `items` has more than `u32::MAX` elements.
    pub fn choose<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            return None;
        }
        let len = u32::try_from(items.len()).expect("GameRng::choose: slice too long");
        items.get(self.below(len) as usize)
    }
}

impl fmt::Debug for GameRng {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GameRng")
            .field("algorithm", &Self::ALGORITHM)
            .field("key", &"<redacted>")
            .field("counter", &self.counter)
            .field("index", &self.index)
            .finish()
    }
}

/// Serialized form of [`GameRng`]: the buffer is recomputed on load.
#[derive(Clone, Serialize, Deserialize)]
struct RngState {
    key: [u32; 8],
    counter: u64,
    index: u8,
}

impl From<GameRng> for RngState {
    fn from(rng: GameRng) -> Self {
        Self {
            key: rng.key,
            counter: rng.counter,
            // index is at most BLOCK_WORDS (16).
            index: rng.index as u8,
        }
    }
}

impl From<RngState> for GameRng {
    fn from(state: RngState) -> Self {
        let index = usize::from(state.index).min(BLOCK_WORDS);
        // Always regenerate, so a restored generator equals the original.
        let buffer = if state.counter > 0 {
            chacha_block(&input_block(&state.key, state.counter - 1), ROUNDS)
        } else {
            [0; BLOCK_WORDS]
        };
        Self {
            key: state.key,
            counter: state.counter,
            buffer,
            index: if state.counter == 0 {
                BLOCK_WORDS
            } else {
                index
            },
        }
    }
}

fn input_block(key: &[u32; 8], counter: u64) -> [u32; BLOCK_WORDS] {
    let mut input = [0; BLOCK_WORDS];
    input[..4].copy_from_slice(&CONSTANTS);
    input[4..12].copy_from_slice(key);
    input[12] = counter as u32;
    input[13] = (counter >> 32) as u32;
    input
}

#[inline(always)]
fn quarter_round(s: &mut [u32; BLOCK_WORDS], a: usize, b: usize, c: usize, d: usize) {
    s[a] = s[a].wrapping_add(s[b]);
    s[d] = (s[d] ^ s[a]).rotate_left(16);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_left(12);
    s[a] = s[a].wrapping_add(s[b]);
    s[d] = (s[d] ^ s[a]).rotate_left(8);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_left(7);
}

/// The ChaCha block function with `rounds` rounds (an even number).
fn chacha_block(input: &[u32; BLOCK_WORDS], rounds: usize) -> [u32; BLOCK_WORDS] {
    let mut s = *input;
    for _ in 0..rounds / 2 {
        quarter_round(&mut s, 0, 4, 8, 12);
        quarter_round(&mut s, 1, 5, 9, 13);
        quarter_round(&mut s, 2, 6, 10, 14);
        quarter_round(&mut s, 3, 7, 11, 15);
        quarter_round(&mut s, 0, 5, 10, 15);
        quarter_round(&mut s, 1, 6, 11, 12);
        quarter_round(&mut s, 2, 7, 8, 13);
        quarter_round(&mut s, 3, 4, 9, 14);
    }
    for (out, inp) in s.iter_mut().zip(input) {
        *out = out.wrapping_add(*inp);
    }
    s
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

    use super::*;

    /// RFC 8439, section 2.3.2: the block function with 20 rounds. The
    /// generator uses the same function with 12 rounds.
    #[test]
    fn block_function_matches_rfc_8439() {
        let input = [
            0x6170_7865,
            0x3320_646e,
            0x7962_2d32,
            0x6b20_6574,
            0x0302_0100,
            0x0706_0504,
            0x0b0a_0908,
            0x0f0e_0d0c,
            0x1312_1110,
            0x1716_1514,
            0x1b1a_1918,
            0x1f1e_1d1c,
            0x0000_0001,
            0x0900_0000,
            0x4a00_0000,
            0x0000_0000,
        ];
        let expected = [
            0xe4e7_f110,
            0x1559_3bd1,
            0x1fdd_0f50,
            0xc471_20a3,
            0xc7f4_d1c7,
            0x0368_c033,
            0x9aaa_2204,
            0x4e6c_d4c3,
            0x4664_82d2,
            0x09aa_9f07,
            0x05d7_c214,
            0xa202_8bd9,
            0xd19c_12b5,
            0xb94e_16de,
            0xe883_d0cb,
            0x4e3c_50a2,
        ];
        assert_eq!(chacha_block(&input, 20), expected);
    }

    /// Pins the `chacha12-v1` output. If this fails, every replay changes.
    #[test]
    fn output_is_pinned() {
        let mut rng = GameRng::from_seed(Seed::from_u64(0));
        let words: Vec<u32> = (0..4).map(|_| rng.next_u32()).collect();
        assert_eq!(words, PINNED_SEED_0);
    }

    // First four words of chacha12 with an all-zero key, counter 0, nonce 0
    // (keystream 9bf49a6a 0755f953 811fce12 5f2683d5 as bytes).
    const PINNED_SEED_0: [u32; 4] = [0x6a9a_f49b, 0x53f9_5507, 0x12ce_1f81, 0xd583_265f];

    #[test]
    fn same_seed_same_sequence() {
        let mut a = GameRng::from_seed(Seed::from_u64(42));
        let mut b = GameRng::from_seed(Seed::from_u64(42));
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_differ() {
        let mut a = GameRng::from_seed(Seed::from_u64(1));
        let mut b = GameRng::from_seed(Seed::from_u64(2));
        let xs: Vec<u32> = (0..8).map(|_| a.next_u32()).collect();
        let ys: Vec<u32> = (0..8).map(|_| b.next_u32()).collect();
        assert_ne!(xs, ys);
    }

    #[test]
    fn clone_continues_identically() {
        let mut a = GameRng::from_seed(Seed::from_u64(7));
        for _ in 0..21 {
            a.next_u32();
        }
        let mut b = a.clone();
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn crosses_block_boundaries_in_order() {
        let mut rng = GameRng::from_seed(Seed::from_u64(3));
        let words: Vec<u32> = (0..48).map(|_| rng.next_u32()).collect();
        let key = GameRng::from_seed(Seed::from_u64(3)).key;
        for (block, chunk) in words.chunks_exact(BLOCK_WORDS).enumerate() {
            let expected = chacha_block(&input_block(&key, block as u64), ROUNDS);
            assert_eq!(chunk, expected);
        }
    }

    #[test]
    fn next_u64_is_low_word_first() {
        let mut a = GameRng::from_seed(Seed::from_u64(5));
        let mut b = a.clone();
        let low = u64::from(b.next_u32());
        let high = u64::from(b.next_u32());
        assert_eq!(a.next_u64(), high << 32 | low);
    }

    #[test]
    fn below_stays_in_range_and_hits_every_value() {
        let mut rng = GameRng::from_seed(Seed::from_u64(9));
        let mut seen = [false; 6];
        for _ in 0..600 {
            let v = rng.below(6);
            assert!(v < 6);
            seen[v as usize] = true;
        }
        assert!(seen.iter().all(|&s| s));
        assert_eq!(rng.below(1), 0);
    }

    #[test]
    fn below_is_roughly_uniform() {
        let mut rng = GameRng::from_seed(Seed::from_u64(11));
        let mut counts = [0u32; 6];
        let n = 60_000;
        for _ in 0..n {
            counts[rng.below(6) as usize] += 1;
        }
        // Expected 10_000 each; 5 standard deviations is about 456.
        for c in counts {
            assert!((9_500..=10_500).contains(&c), "{counts:?}");
        }
    }

    #[test]
    #[should_panic(expected = "bound 0")]
    fn below_zero_panics() {
        GameRng::from_seed(Seed::from_u64(0)).below(0);
    }

    #[test]
    fn shuffle_is_deterministic_permutation() {
        let shuffled = |seed| {
            let mut items: Vec<u32> = (0..32).collect();
            GameRng::from_seed(Seed::from_u64(seed)).shuffle(&mut items);
            items
        };
        let a = shuffled(13);
        assert_eq!(a, shuffled(13));
        assert_ne!(a, shuffled(14));
        let mut sorted = a.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..32).collect::<Vec<_>>());
    }

    #[test]
    fn shuffle_follows_documented_algorithm() {
        let mut items: Vec<u32> = (0..10).collect();
        GameRng::from_seed(Seed::from_u64(17)).shuffle(&mut items);

        let mut rng = GameRng::from_seed(Seed::from_u64(17));
        let mut expected: Vec<u32> = (0..10).collect();
        for i in (1..10u32).rev() {
            let j = rng.below(i + 1);
            expected.swap(i as usize, j as usize);
        }
        assert_eq!(items, expected);
    }

    #[test]
    fn shuffle_handles_tiny_slices() {
        let mut rng = GameRng::from_seed(Seed::from_u64(0));
        let mut empty: [u8; 0] = [];
        rng.shuffle(&mut empty);
        let mut one = [1];
        rng.shuffle(&mut one);
        assert_eq!(one, [1]);
        assert_eq!(rng.choose::<u8>(&[]), None);
        assert_eq!(rng.choose(&[5]), Some(&5));
    }

    #[test]
    fn serde_round_trip_resumes_mid_block() {
        for consumed in [0, 1, 15, 16, 17, 40] {
            let mut rng = GameRng::from_seed(Seed::from_u64(23));
            for _ in 0..consumed {
                rng.next_u32();
            }
            let json = serde_json::to_string(&rng).unwrap();
            let mut restored: GameRng = serde_json::from_str(&json).unwrap();
            assert_eq!(restored, rng, "after {consumed} words");
            for _ in 0..40 {
                assert_eq!(restored.next_u32(), rng.next_u32());
            }
        }
    }

    #[test]
    fn debug_hides_key() {
        let rng = GameRng::from_seed(Seed([0xab; 32]));
        let text = std::format!("{rng:?}");
        assert!(text.contains("redacted"));
        assert!(!text.contains("abab"));
    }

    #[test]
    fn seed_hex_round_trip() {
        let seed = Seed::from_u64(0x0102_0304_0506_0708);
        let json = serde_json::to_string(&seed).unwrap();
        assert_eq!(
            json,
            "\"0807060504030201000000000000000000000000000000000000000000000000\""
        );
        assert_eq!(serde_json::from_str::<Seed>(&json).unwrap(), seed);
        assert!(serde_json::from_str::<Seed>("\"00\"").is_err());
        let upper = serde_json::to_string(&Seed([0xab; 32]))
            .unwrap()
            .to_uppercase();
        assert!(serde_json::from_str::<Seed>(&upper).is_err());
    }
}
