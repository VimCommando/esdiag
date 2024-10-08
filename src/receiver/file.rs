use super::Source;
use crate::{data::diagnostic::Product, exporter::Exporter, setup::Asset};
use color_eyre::eyre::{eyre, Result};
use include_dir::{include_dir, Dir};
use serde_yaml;
use std::collections::BTreeMap;

// Subdirectory for templates and configs files
pub static ASSETS_DIR: Dir = include_dir!("assets");
pub static ELASTICSEARCH_ASSETS: &str = "elasticsearch/assets.yml";
pub static ELASTICSEARCH_SOURCES: &str = "elasticsearch/sources.yml";

/// Parses the `sources.yml` file for a given product and returns its contents as a `HashMap`.
///
/// # Arguments
///
/// * `product` - A reference to the `Product` for which the `sources.yml` file should be parsed.
///
/// # Returns
///
/// A `Result` containing a `HashMap` with `String` keys and `Source` values if successful,
/// or a boxed `Error` if an error occurs.
///
/// # Errors
///
/// This function will return an error if:
/// - The `sources.yml` file is not found for the specified product.
/// - The `sources.yml` file cannot be parsed.
/// - The specified product is not implemented.
///
/// # Example
///
/// ```rust
/// match parse_sources_yml(&Product::Elasticsearch) {
///     Ok(sources) => println!("Parsed sources: {:?}", sources),
///     Err(e) => eprintln!("Error parsing sources.yml: {}", e),
/// }
/// ```

pub fn parse_sources_yml(product: &Product) -> Result<BTreeMap<String, Source>> {
    log::debug!("Parsing sources.yml");
    let file = match product {
        Product::Elasticsearch => ASSETS_DIR
            .get_file(ELASTICSEARCH_SOURCES)
            .ok_or(eyre!("Error reading {ELASTICSEARCH_SOURCES}"))?,
        _ => return Err(eyre!("{} not yet implemented", product)),
    };
    let sources = serde_yaml::from_slice(file.contents())?;
    Ok(sources)
}

/// Parses the `assets.yml` file for a given target and returns its contents as a `Vec` of `Asset`.
///
/// # Arguments
///
/// * `target` - A reference to the `Target` for which the `assets.yml` file should be parsed.
///
/// # Returns
///
/// A `Result` containing a `Vec` of `Asset` if successful,
/// or a boxed `Error` if an error occurs.
///
/// # Errors
///
/// This function will return an error if:
/// - The `assets.yml` file is not found for the specified target.
/// - The `assets.yml` file cannot be parsed.
/// - The specified target is not implemented.
///
/// # Example
///
/// ```rust
/// match parse_assets_yml(&Target::Elasticsearch(SomeVersion)) {
///     Ok(assets) => println!("Parsed assets: {:?}", assets),
///     Err(e) => eprintln!("Error parsing assets.yml: {}", e),
/// }
/// ```

pub fn parse_assets_yml(exporter: &Exporter) -> Result<Vec<Asset>> {
    let file = match exporter {
        Exporter::Elasticsearch(_) => ASSETS_DIR
            .get_file(ELASTICSEARCH_ASSETS)
            .ok_or(eyre!("Error reading {ELASTICSEARCH_ASSETS}"))?,
        _ => return Err(eyre!("Application not implemented")),
    };
    let assets = serde_yaml::from_slice(file.contents())?;
    Ok(assets)
}
