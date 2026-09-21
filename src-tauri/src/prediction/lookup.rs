use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

pub const MAGIC: &[u8; 8] = b"SWPRED02";
const MAX_FILE: u64 = 128 * 1024 * 1024;
const MAX_WORDS: usize = 250_000;
const MAX_ROWS: usize = 1_500_000;
const MAX_WORD_BYTES: usize = 512;
const MAX_TEXT_BYTES: usize = 32 * 1024 * 1024;
const NONE: u32 = u32::MAX;

struct Reader<R> {
    input: R,
    hash: Sha256,
}
impl<R: Read> Reader<R> {
    fn bytes(&mut self, bytes: &mut [u8]) -> Result<(), ()> {
        self.input.read_exact(bytes).map_err(|_| ())?;
        self.hash.update(bytes);
        Ok(())
    }
    fn number(&mut self) -> Result<u32, ()> {
        let mut bytes = [0; 4];
        self.bytes(&mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }
}
struct Word {
    id: u32,
    text: String,
    folded: String,
}
pub struct Lookup {
    words: Vec<Word>,
    ids: HashMap<String, u32>,
    groups: HashMap<[u32; 3], Vec<u32>>,
    prefixes: Vec<u32>,
}
impl Lookup {
    pub fn open(path: &Path) -> Result<Self, ()> {
        let file = File::open(path).map_err(|_| ())?;
        let metadata = file.metadata().map_err(|_| ())?;
        if !metadata.is_file() || !(60..=MAX_FILE).contains(&metadata.len()) {
            return Err(());
        }
        Self::read(BufReader::new(file.take(MAX_FILE + 1)))
    }
    fn read(mut input: impl Read) -> Result<Self, ()> {
        let mut magic = [0; 8];
        input.read_exact(&mut magic).map_err(|_| ())?;
        if &magic != MAGIC {
            return Err(());
        }
        let mut expected = [0; 32];
        input.read_exact(&mut expected).map_err(|_| ())?;
        let mut r = Reader {
            input,
            hash: Sha256::new(),
        };
        let count = r.number()? as usize;
        if count == 0 || count > MAX_WORDS {
            return Err(());
        }
        let mut words: Vec<Word> = Vec::new();
        words.try_reserve_exact(count).map_err(|_| ())?;
        let mut ids = HashMap::new();
        ids.try_reserve(count).map_err(|_| ())?;
        let mut unique = HashSet::new();
        unique.try_reserve(count).map_err(|_| ())?;
        let mut previous = None;
        let mut total = 0usize;
        for rank in 0..count {
            let id = r.number()?;
            let frequency = r.number()?;
            if id == 0
                || !unique.insert(id)
                || previous.is_some_and(|(f, i)| frequency > f || (frequency == f && id <= i))
            {
                return Err(());
            }
            previous = Some((frequency, id));
            let size = r.number()? as usize;
            total = total.checked_add(size).ok_or(())?;
            if size > MAX_WORD_BYTES || total > MAX_TEXT_BYTES {
                return Err(());
            }
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(size).map_err(|_| ())?;
            bytes.resize(size, 0);
            r.bytes(&mut bytes)?;
            let text = String::from_utf8(bytes).map_err(|_| ())?;
            let folded = text.to_lowercase();
            ids.entry(folded.clone()).or_insert(rank as u32);
            words.push(Word { id, text, folded });
        }
        drop(unique);
        let mut groups: HashMap<[u32; 3], Vec<u32>> = HashMap::new();
        for length in 1..=3 {
            let rows = r.number()? as usize;
            if rows > MAX_ROWS {
                return Err(());
            }
            let mut previous = None;
            for _ in 0..rows {
                let mut key = [NONE; 3];
                for slot in &mut key[..length] {
                    *slot = r.number()?;
                    if *slot as usize >= count {
                        return Err(());
                    }
                }
                let rank = r.number()?;
                let frequency = r.number()?;
                let word = words.get(rank as usize).ok_or(())?;
                if previous.is_some_and(|(f, i)| frequency > f || (frequency == f && word.id < i)) {
                    return Err(());
                }
                previous = Some((frequency, word.id));
                if !groups.contains_key(&key) {
                    groups.try_reserve(1).map_err(|_| ())?;
                }
                let list = groups.entry(key).or_default();
                list.try_reserve(1).map_err(|_| ())?;
                list.push(rank);
            }
        }
        if r.number()? as usize != count {
            return Err(());
        }
        let mut prefixes = Vec::new();
        prefixes.try_reserve_exact(count).map_err(|_| ())?;
        let mut seen = Vec::new();
        seen.try_reserve_exact(count).map_err(|_| ())?;
        seen.resize(count, false);
        for _ in 0..count {
            let rank = r.number()?;
            let word = words.get(rank as usize).ok_or(())?;
            if seen[rank as usize] {
                return Err(());
            }
            seen[rank as usize] = true;
            if let Some(&p) = prefixes.last() {
                let previous: &Word = &words[p as usize];
                if (&previous.folded, p) >= (&word.folded, rank) {
                    return Err(());
                }
            }
            prefixes.push(rank);
        }
        let mut trailing = [0];
        if r.input.read(&mut trailing).map_err(|_| ())? != 0
            || r.hash.finalize().as_slice() != expected
        {
            return Err(());
        }
        Ok(Self {
            words,
            ids,
            groups,
            prefixes,
        })
    }
    pub fn predict(&self, context: &[String], prefix: &str) -> Vec<String> {
        let prefix = prefix.to_lowercase();
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        for count in (1..=context.len().min(3)).rev() {
            let mut key = [NONE; 3];
            let mut known = true;
            for (slot, word) in key.iter_mut().zip(&context[context.len() - count..]) {
                match self.ids.get(&word.to_lowercase()) {
                    Some(rank) => *slot = *rank,
                    None => {
                        known = false;
                        break;
                    }
                }
            }
            if let Some(list) = known.then(|| self.groups.get(&key)).flatten() {
                for &rank in list {
                    self.push(rank, &prefix, &mut result, &mut seen);
                    if result.len() == 5 {
                        return result;
                    }
                }
            }
        }
        if prefix.is_empty() {
            for rank in 0..self.words.len() {
                self.push(rank as u32, &prefix, &mut result, &mut seen);
                if result.len() == 5 {
                    break;
                }
            }
        } else {
            let start = self
                .prefixes
                .partition_point(|&i| self.words[i as usize].folded.as_str() < prefix.as_str());
            let mut best: Vec<u32> = Vec::new();
            for &rank in self.prefixes[start..]
                .iter()
                .take_while(|&&i| self.words[i as usize].folded.starts_with(&prefix))
            {
                let word = &self.words[rank as usize];
                if usable(&word.text) && !seen.contains(&word.folded) {
                    if let Some(i) = best
                        .iter()
                        .position(|&r| self.words[r as usize].folded == word.folded)
                    {
                        best[i] = best[i].min(rank);
                    } else {
                        best.push(rank);
                    }
                    best.sort_unstable();
                    best.truncate(5 - result.len());
                }
            }
            for rank in best {
                self.push(rank, &prefix, &mut result, &mut seen);
            }
        }
        result
    }
    fn push(&self, rank: u32, prefix: &str, result: &mut Vec<String>, seen: &mut HashSet<String>) {
        let word = &self.words[rank as usize];
        if result.len() < 5
            && usable(&word.text)
            && word.folded.starts_with(prefix)
            && seen.insert(word.folded.clone())
        {
            result.push(word.text.clone());
        }
    }
}
fn usable(word: &str) -> bool {
    word.chars().count() <= 48 && !word.chars().any(char::is_whitespace)
}

#[cfg(test)]
impl Lookup {
    pub fn fixture() -> Self {
        Self::read(&tests::fixture_bytes()[..]).unwrap()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn n(out: &mut Vec<u8>, value: u32) {
        out.extend_from_slice(&value.to_le_bytes());
    }
    fn seal(payload: Vec<u8>) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        out.extend_from_slice(&Sha256::digest(&payload));
        out.extend(payload);
        out
    }
    pub(super) fn fixture_bytes() -> Vec<u8> {
        let mut out = Vec::new();
        n(&mut out, 5);
        for (id, text, freq) in [
            (1, "water", 9),
            (2, "walk", 8),
            (3, "want", 7),
            (4, "I", 6),
            (5, "like", 5),
        ] {
            n(&mut out, id);
            n(&mut out, freq);
            n(&mut out, text.len() as u32);
            out.extend_from_slice(text.as_bytes());
        }
        for _ in 0..3 {
            n(&mut out, 0);
        }
        n(&mut out, 5);
        for rank in [3, 4, 1, 2, 0] {
            n(&mut out, rank);
        }
        seal(out)
    }
    fn change(bytes: &[u8], offset: usize, value: u32) -> Vec<u8> {
        let mut payload = bytes[40..].to_vec();
        payload[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        seal(payload)
    }
    #[test]
    fn malformed_artifacts_fail_closed() {
        let bytes = fixture_bytes();
        assert!(Lookup::read(&bytes[..]).is_ok());
        for size in 0..bytes.len() {
            assert!(Lookup::read(&bytes[..size]).is_err());
        }
        for i in 0..bytes.len() {
            let mut bad = bytes.clone();
            bad[i] ^= 0x80;
            assert!(Lookup::read(&bad[..]).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(Lookup::read(&trailing[..]).is_err());
        for (offset, value) in [
            (0, 0),
            (0, u32::MAX),
            (4, 0),
            (8, 0),
            (12, u32::MAX),
            (21, 1),
        ] {
            assert!(Lookup::read(&change(&bytes, offset, value)[..]).is_err());
        }
        let prefix_count = bytes.len() - 40 - 24;
        assert!(Lookup::read(&change(&bytes, prefix_count, 4)[..]).is_err());
        assert!(Lookup::read(&change(&bytes, prefix_count + 4, 5)[..]).is_err());
        assert!(Lookup::read(&change(&bytes, prefix_count + 8, 3)[..]).is_err());
        assert!(Lookup::read(&change(&bytes, prefix_count + 4, 0)[..]).is_err());
        assert!(Lookup::read(&change(&bytes, prefix_count - 12, u32::MAX)[..]).is_err());
    }
    #[test]
    fn ranking_unicode_deduplication_and_context_backoff() {
        let entries = [
            (1, "I", 100),
            (2, "like", 90),
            (3, "water", 20),
            (4, "walk", 20),
            (5, "Water", 19),
            (6, "café", 15),
            (7, "CAFÉ", 14),
            (8, "café", 13),
            (9, "two words", 12),
            (10, "İstanbul", 11),
            (11, "你好", 10),
        ];
        let mut out = Vec::new();
        n(&mut out, entries.len() as u32);
        for (id, text, freq) in entries {
            n(&mut out, id);
            n(&mut out, freq);
            n(&mut out, text.len() as u32);
            out.extend_from_slice(text.as_bytes());
        }
        n(&mut out, 3);
        for (rank, freq) in [(4, 40), (2, 30), (3, 30)] {
            n(&mut out, 1);
            n(&mut out, rank);
            n(&mut out, freq);
        }
        n(&mut out, 1);
        for value in [0, 1, 2, 1] {
            n(&mut out, value);
        }
        n(&mut out, 1);
        for value in [0, 0, 1, 3, 1] {
            n(&mut out, value);
        }
        let mut prefixes: Vec<_> = (0..entries.len()).collect();
        prefixes.sort_by_key(|&i| (entries[i].1.to_lowercase(), i));
        n(&mut out, prefixes.len() as u32);
        for i in prefixes {
            n(&mut out, i as u32);
        }
        let bytes = seal(out);
        let lookup = Lookup::read(&bytes[..]).unwrap();
        assert_eq!(
            lookup.predict(&["I".into(), "I".into(), "like".into()], "w"),
            ["walk", "water"]
        );
        assert_eq!(lookup.predict(&["like".into()], "w"), ["Water", "walk"]);
        assert_eq!(lookup.predict(&[], "CAF"), ["café", "café"]);
        assert_eq!(lookup.predict(&[], "İ"), ["İstanbul"]);
        assert_eq!(lookup.predict(&[], "你"), ["你好"]);
        assert_eq!(lookup.predict(&["unknown".into()], "w"), ["water", "walk"]);
        assert!(!lookup.predict(&[], "").contains(&"two words".into()));
        assert!(lookup.predict(&[], "unknown").is_empty());
    }
    #[test]
    fn missing_and_oversized_files_fail_closed() {
        let path = std::env::temp_dir().join(format!("switchify-lookup-{}", std::process::id()));
        assert!(Lookup::open(&path).is_err());
        let file = File::create(&path).unwrap();
        file.set_len(MAX_FILE + 1).unwrap();
        drop(file);
        assert!(Lookup::open(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
