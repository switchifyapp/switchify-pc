use super::{context::Context, lookup::Lookup};
use std::path::Path;

pub struct Database(Lookup);
impl Database {
    pub fn open(path: &Path) -> Result<Self, ()> {
        Lookup::open(path).map(Self)
    }
    pub fn predict(&self, context: &Context) -> Result<Vec<String>, ()> {
        Ok(self.0.predict(&context.words, &context.prefix))
    }
    #[cfg(test)]
    pub fn fixture() -> Self {
        Self(Lookup::fixture())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_lookup_matches_sqlite_corpus_and_expanded_samples() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources");
        let source = root.join("WordData2017051601.db");
        let reference = super::super::database_reference::Database::open(&source).unwrap();
        let lookup = Database::open(&root.join("word-predictions.lookup")).unwrap();
        let c = rusqlite::Connection::open_with_flags(
            source,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let mut texts: Vec<String> = [
            "",
            "a",
            "w",
            "wa",
            "water",
            "zzzzzz",
            "I ",
            "I would ",
            "I would like ",
            "I would like wa",
            "the ",
            "the a",
            "unknownword wa",
            "café",
            "cafe\u{301}",
            "İ",
            "你好",
            "I can’t find ",
            "Done! ",
            "hello\nwa",
            "W",
            "WATER",
            "🙂 ",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let ids: std::collections::HashMap<u32, String> = c
            .prepare("SELECT ID,WORD FROM WORDS")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        for offset in [0, 499] {
            for (n, table) in [(2, "BIGRAMS"), (3, "TRIGRAMS"), (4, "QUADGRAMS")] {
                let fields = (1..=n)
                    .map(|i| format!("ID{i}"))
                    .collect::<Vec<_>>()
                    .join(",");
                let mut q = c
                    .prepare(&format!(
                        "SELECT {fields} FROM {table} WHERE ID%997={offset} ORDER BY ID LIMIT 80"
                    ))
                    .unwrap();
                let rows = q
                    .query_map([], |r| {
                        (0..n)
                            .map(|i| r.get::<_, u32>(i))
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .unwrap();
                for row in rows {
                    let row = row.unwrap();
                    let context = row[..n - 1]
                        .iter()
                        .map(|id| ids[id].as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    for size in [0, 1, 3] {
                        texts.push(format!(
                            "{context} {}",
                            ids[&row[n - 1]].chars().take(size).collect::<String>()
                        ));
                    }
                }
            }
        }
        assert_eq!(texts.len(), 1463);
        for (i, text) in texts.iter().enumerate() {
            let context = super::super::context::extract(text, i % 17 == 0);
            assert_eq!(
                lookup.predict(&context),
                reference.predict(&context),
                "case {i}"
            );
        }
    }
}
