use super::context::Context;
use rusqlite::{Connection, OpenFlags};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

pub struct Database {
    connection: Connection,
    words: Vec<(i64, String, String)>,
    ids: HashMap<String, i64>,
}
impl Database {
    #[cfg(test)]
    pub fn fixture() -> Self {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE WORDS(ID INTEGER,WORD TEXT,BASE_FREQUENCY INTEGER); INSERT INTO WORDS VALUES(1,'water',9),(2,'walk',8),(3,'want',7),(4,'I',6),(5,'like',5); CREATE TABLE BIGRAMS(ID1 INTEGER,ID2 INTEGER,BASE_FREQUENCY INTEGER); CREATE TABLE TRIGRAMS(ID1 INTEGER,ID2 INTEGER,ID3 INTEGER,BASE_FREQUENCY INTEGER); CREATE TABLE QUADGRAMS(ID1 INTEGER,ID2 INTEGER,ID3 INTEGER,ID4 INTEGER,BASE_FREQUENCY INTEGER);").unwrap();
        Self::from_connection(c).unwrap()
    }
    pub fn open(path: &Path) -> Result<Self, ()> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|_| ())?;
        Self::from_connection(connection)
    }
    fn from_connection(connection: Connection) -> Result<Self, ()> {
        connection
            .execute_batch("PRAGMA query_only=ON; PRAGMA cache_size=-8192;")
            .map_err(|_| ())?;
        let words = {
            let mut q = connection
                .prepare("SELECT ID, WORD FROM WORDS ORDER BY BASE_FREQUENCY DESC, ID ASC")
                .map_err(|_| ())?;
            let result = q
                .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
                .map_err(|_| ())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| ())?;
            result
        };
        let words: Vec<_> = words
            .into_iter()
            .map(|(id, word)| {
                let folded = word.to_lowercase();
                (id, word, folded)
            })
            .collect();
        let mut ids = HashMap::new();
        for (id, _, folded) in &words {
            ids.entry(folded.clone()).or_insert(*id);
        }
        Ok(Self {
            connection,
            words,
            ids,
        })
    }
    pub fn predict(&self, context: &Context) -> Result<Vec<String>, ()> {
        let prefix = context.prefix.to_lowercase();
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        let mut push = |word: String| {
            let folded = word.to_lowercase();
            if result.len() < 5
                && word.chars().count() <= 48
                && !word.chars().any(char::is_whitespace)
                && folded.starts_with(&prefix)
                && seen.insert(folded)
            {
                result.push(word);
            }
            result.len() == 5
        };
        for count in (1..=context.words.len().min(3)).rev() {
            let ids: Option<Vec<i64>> = context.words[context.words.len() - count..]
                .iter()
                .map(|w| self.ids.get(&w.to_lowercase()).copied())
                .collect();
            let Some(ids) = ids else {
                continue;
            };
            let table = ["", "BIGRAMS", "TRIGRAMS", "QUADGRAMS"][count];
            let conditions = (1..=count)
                .map(|i| format!("g.ID{i}=?{i}"))
                .collect::<Vec<_>>()
                .join(" AND ");
            let sql = format!("SELECT w.WORD FROM {table} g JOIN WORDS w ON w.ID=g.ID{} WHERE {conditions} ORDER BY g.BASE_FREQUENCY DESC, w.ID ASC", count+1);
            let mut statement = self.connection.prepare_cached(&sql).map_err(|_| ())?;
            let mut rows = statement
                .query(rusqlite::params_from_iter(ids))
                .map_err(|_| ())?;
            // Context indexes bound this candidate set; the entire query is also
            // contained by the parent process's two-second deadline.
            while let Some(row) = rows.next().map_err(|_| ())? {
                if push(row.get(0).map_err(|_| ())?) {
                    break;
                }
            }
        }
        for (_, word, _) in self
            .words
            .iter()
            .filter(|(_, _, folded)| folded.starts_with(&prefix))
        {
            if push(word.clone()) {
                break;
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn longest_context_wins_and_backoff_deduplicates() {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE WORDS(ID INTEGER,WORD TEXT,BASE_FREQUENCY INTEGER); INSERT INTO WORDS VALUES(1,'I',9),(2,'would',8),(3,'like',7),(4,'water',1),(5,'wine',20),(6,'walk',30); CREATE TABLE BIGRAMS(ID1 INTEGER,ID2 INTEGER,BASE_FREQUENCY INTEGER); INSERT INTO BIGRAMS VALUES(3,5,9); CREATE TABLE TRIGRAMS(ID1 INTEGER,ID2 INTEGER,ID3 INTEGER,BASE_FREQUENCY INTEGER); CREATE TABLE QUADGRAMS(ID1 INTEGER,ID2 INTEGER,ID3 INTEGER,ID4 INTEGER,BASE_FREQUENCY INTEGER); INSERT INTO QUADGRAMS VALUES(1,2,3,4,1);").unwrap();
        let db = Database::from_connection(c).unwrap();
        let result = db
            .predict(&super::super::context::extract("I would like w", false))
            .unwrap();
        assert_eq!(&result[..3], ["water", "wine", "walk"]);
        assert!(db.connection.execute("DELETE FROM WORDS", []).is_err());
    }
}
