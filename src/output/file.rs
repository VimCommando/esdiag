use serde_json::Value;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

pub fn write_ndjson<'a>(input: String, value: Value, filename: &PathBuf) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(filename)?;
    let body = serde_json::to_string(&value).unwrap();
    file.write_all(body.as_bytes())?;
    file.write_all(b"\n")?;
    log::info!("{}: appended {input}", filename.display());
    Ok(())
}

pub fn write_bulk_docs<'a>(docs: Vec<Value>, filename: &PathBuf) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(filename)?;
    let len = docs.len();
    for doc in docs {
        file.write_all(doc.to_string().as_bytes())?;
        file.write_all(b"\n")?;
    }
    log::info!("{}: appended {} docs", filename.display(), len);
    Ok(())
}
