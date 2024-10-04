use crate::env;
use serde::Serialize;
use serde_json::Value;

/// Writes a JSON value to an NDJSON (Newline Delimited JSON) file.
///
/// This function takes a JSON `Value` object and writes it to the specified file in NDJSON format.
/// Each JSON object is serialized to a string and written followed by a newline character (`\n`).
///
/// # Arguments
///
/// * `value` - A JSON `Value` object to write to the NDJSON file.
/// * `filename` - A `&PathBuf` representing the path to the NDJSON file.
/// * `append` - A `bool` indicating whether to append to an existing file (`true`) or overwrite (`false`).
///
/// # Returns
///
/// This function returns `Ok(())` if the operation succeeds. If an error occurs during file operations
/// (e.g., file not found, permission denied), it returns an `std::io::Error`.
///
/// # Errors
///
/// This function will return an error if it encounters an I/O error while opening, writing to,
/// or closing the NDJSON file. Serialization errors may also occur if the JSON value cannot be
/// converted to a string.
///
/// # Examples
///
/// ```rust
/// use serde_json::json;
/// use std::path::PathBuf;
///
/// let data = json!({"key": "value"});
/// let filename = PathBuf::from("output.ndjson");
/// let append = true;
/// match write_ndjson(data, &filename, append) {
///     Ok(_) => println!("Successfully wrote data to {:?}", filename),
///     Err(e) => eprintln!("Failed to write data: {}", e),
/// }
/// ```

pub fn write_ndjson<'a>(value: Value, filename: &PathBuf, append: bool) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(append)
        .open(filename)?;
    let body = serde_json::to_string(&value).expect("Failed to serialize value");
    file.write_all(body.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

/// Conditionally writes a JSON value to an NDJSON file if debug logging is enabled.
///
/// This function takes a JSON `Value` object and writes it to an NDJSON file specified by the
/// `filename`. If debug logging is enabled (`log::Level::Debug`), the JSON value is appended to
/// the file. If debug logging is not enabled, the function returns early without performing any
/// file operations.
///
/// # Arguments
///
/// * `value` - A JSON `Value` object to write to the NDJSON file.
/// * `filename` - A `&str` representing the filename (relative to `ESDIAG_HOME`) of the NDJSON file.
/// * `append` - A `bool` indicating whether to append to an existing file (`true`) or overwrite (`false`).
///
/// # Returns
///
/// This function returns `Ok(())` if debug logging is not enabled or if the operation succeeds.
/// If an error occurs during file operations (e.g., file not found, permission denied),
/// it returns an `std::io::Error`.
///
/// # Errors
///
/// This function will return an error if it fails to retrieve the home directory (`HOME`) or `ESDIAG_HOME`
/// environment variables, or if it encounters an I/O error while writing to the NDJSON file.
///
/// # Examples
///
/// ```rust
/// use serde_json::json;
///
/// let data = json!({"key": "value"});
/// let filename = "output.ndjson";
/// let append = true;
/// match write_ndjson_if_debug(data, filename, append) {
///     Ok(_) => println!("Successfully wrote data to {}", filename),
///     Err(e) => eprintln!("Failed to write data: {}", e),
/// }
/// ```

pub fn write_ndjson_if_debug<'a>(
    value: Value,
    filename: &str,
    append: bool,
) -> std::io::Result<()> {
    let home = PathBuf::from(env::get_string("HOME")?).join(env::get_string("ESDIAG_HOME")?);
    if log::log_enabled!(log::Level::Debug) {
        write_ndjson(value, &home.join(filename), append)
    } else {
        Ok(())
    }
}

/// Appends a vector of JSON documents to a file.
///
/// This function takes a vector of JSON `Value` objects and appends each document to the specified
/// file. Each document is converted to a string and written to the file, followed by a newline.
///
/// # Arguments
///
/// * `docs` - A vector of JSON `Value` objects representing the documents to be appended.
/// * `filename` - A `PathBuf` representing the path to the file where the documents will be appended.
///
/// # Returns
///
/// This function returns a `Result` containing the number of documents appended if successful,
/// or an `std::io::Error` if an error occurs during the file operations.
///
/// # Errors
///
/// This function will return an error if it fails to open the file, write a document, or write a newline.
///
/// # Examples
///
/// ```rust
/// let docs: Vec<Value> = vec![json!({"key": "value"}), json!({"another_key": "another_value"})];
/// let filename = PathBuf::from("output.json");
/// match append_bulk_docs(docs, &filename) {
///     Ok(count) => println!("Successfully appended {} documents", count),
///     Err(e) => eprintln!("Failed to append documents: {}", e),
/// }
/// ```

pub fn append_bulk_docs<'a>(docs: Vec<Value>, filename: &PathBuf) -> std::io::Result<usize> {
    let len = docs.len();
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(filename)?;
    for doc in docs {
        file.write_all(doc.to_string().as_bytes())?;
        file.write_all(b"\n")?;
    }
    Ok(len)
}

pub fn debug_save<T: Serialize>(filename: &str, content: &T) -> std::io::Result<()> {
    if !log::log_enabled!(log::Level::Debug) {
        return Ok(());
    }
    let home_file = PathBuf::from(env::get_string("HOME")?)
        .join(env::get_string("ESDIAG_HOME")?)
        .join(filename);
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(home_file)?;
    let body = serde_json::to_string(&content)?;
    file.write_all(body.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

// *** Exporter ***

use super::Export;
use color_eyre::eyre::Result;
use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::PathBuf,
};

pub struct FileExporter {
    file: File,
    path: PathBuf,
}

impl TryFrom<PathBuf> for FileExporter {
    type Error = color_eyre::eyre::Report;

    fn try_from(path: PathBuf) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&path)?;
        Ok(Self { file, path })
    }
}

impl Export for FileExporter {
    async fn is_connected(&self) -> bool {
        let is_file = self.path.is_file();
        let filename = self.path.to_str().unwrap_or("");
        log::debug!("File {filename} is valid: {is_file}");
        is_file
    }

    async fn write(&self, _index: String, docs: Vec<Value>) -> Result<usize> {
        match &self.path.is_file() {
            false => {
                log::info!("Creating file {}", &self.path.display());
                std::fs::File::create(&self.path)?;
            }
            true => {
                log::debug!("File {} exists", &self.path.display());
            }
        }
        log::debug!("Writing docs to file {}", &self.path.display());
        let mut writer = BufWriter::new(&self.file);
        let mut doc_count = 0;
        for doc in docs {
            serde_json::to_writer(&mut writer, &doc)?;
            writeln!(&mut writer)?;
            doc_count += 1;
        }
        writer.flush()?;
        Ok(doc_count)
    }
}

impl std::fmt::Display for FileExporter {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.path.display())
    }
}
