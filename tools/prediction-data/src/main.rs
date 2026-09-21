#[allow(dead_code)]
#[path = "../../../src-tauri/src/prediction/lookup.rs"]
mod lookup;
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufWriter, Read, Seek, SeekFrom, Write},
    path::Path,
};
type Error = Box<dyn std::error::Error>;
fn number(out: &mut impl Write, n: u32) -> Result<(), Error> {
    out.write_all(&n.to_le_bytes())?;
    Ok(())
}
fn convert(source: &Path, output: &Path) -> Result<(), Error> {
    let c = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut q =
        c.prepare("SELECT ID,WORD,BASE_FREQUENCY FROM WORDS ORDER BY BASE_FREQUENCY DESC,ID ASC")?;
    let words = q
        .query_map([], |r| {
            Ok((
                r.get::<_, u32>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, u32>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let ranks: HashMap<_, _> = words
        .iter()
        .enumerate()
        .map(|(rank, w)| (w.0, rank as u32))
        .collect();
    let temporary = output.with_extension("pending");
    let mut out = BufWriter::new(File::create(&temporary)?);
    out.write_all(lookup::MAGIC)?;
    out.write_all(&[0; 32])?;
    number(&mut out, u32::try_from(words.len())?)?;
    for (id, text, frequency) in &words {
        number(&mut out, *id)?;
        number(&mut out, *frequency)?;
        number(&mut out, u32::try_from(text.len())?)?;
        out.write_all(text.as_bytes())?;
    }
    for (n, table) in [(2, "BIGRAMS"), (3, "TRIGRAMS"), (4, "QUADGRAMS")] {
        let count: u32 = c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))?;
        number(&mut out, count)?;
        let fields = (1..=n)
            .map(|i| format!("ID{i}"))
            .collect::<Vec<_>>()
            .join(",");
        let context = (1..n)
            .map(|i| format!("ID{i}"))
            .collect::<Vec<_>>()
            .join(",");
        let mut q=c.prepare(&format!("SELECT {fields},BASE_FREQUENCY FROM {table} ORDER BY BASE_FREQUENCY DESC,ID{n} ASC,{context}"))?;
        let mut rows = q.query([])?;
        while let Some(row) = rows.next()? {
            for i in 0..n {
                let id: u32 = row.get(i)?;
                number(&mut out, *ranks.get(&id).ok_or("Missing word reference")?)?;
            }
            number(&mut out, row.get(n)?)?;
        }
    }
    let folded: Vec<_> = words.iter().map(|w| w.1.to_lowercase()).collect();
    let mut prefixes: Vec<_> = (0..words.len()).collect();
    prefixes.sort_unstable_by(|&a, &b| folded[a].cmp(&folded[b]).then(a.cmp(&b)));
    number(&mut out, u32::try_from(prefixes.len())?)?;
    for rank in prefixes {
        number(&mut out, u32::try_from(rank)?)?;
    }
    out.flush()?;
    drop(out);
    let mut input = File::open(&temporary)?;
    input.seek(SeekFrom::Start(40))?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    drop(input);
    let mut file = fs::OpenOptions::new().write(true).open(&temporary)?;
    file.seek(SeekFrom::Start(8))?;
    file.write_all(&hash.finalize())?;
    file.sync_all()?;
    drop(file);
    lookup::Lookup::open(&temporary).map_err(|_| "Generated lookup failed validation")?;
    fs::rename(&temporary, output)?;
    println!("Generated {} bytes", fs::metadata(output)?.len());
    Ok(())
}
fn checksum(path: &Path) -> Result<Vec<u8>, Error> {
    let mut input = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(hash.finalize().to_vec())
}
fn main() -> Result<(), Error> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() == 4 && args[1] == "--check" {
        let generated = std::env::temp_dir().join(format!(
            "switchify-predictions-{}.lookup",
            std::process::id()
        ));
        let result = (|| {
            convert(Path::new(&args[2]), &generated)?;
            if checksum(&generated)? != checksum(Path::new(&args[3]))? {
                return Err("Generated lookup differs from packaged artifact".into());
            }
            Ok(())
        })();
        let _ = fs::remove_file(&generated);
        let _ = fs::remove_file(generated.with_extension("pending"));
        return result;
    }
    if args.len() != 3 {
        return Err(
            "Usage: switchify-prediction-data [--check] <source.db> <output.lookup>".into(),
        );
    }
    convert(Path::new(&args[1]), Path::new(&args[2]))
}
