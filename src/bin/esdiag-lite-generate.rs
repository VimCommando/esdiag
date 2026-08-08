// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

//! Maintainer utility for the generated region in `bin/esdiag-lite.sh`.

use esdiag::processor::diagnostic::data_source::{VersionSource, get_product_sources, parse_npm_version_requirement};
use eyre::{Result, bail, eyre};
use std::cmp::Ordering;
use std::fmt::Write;
use std::path::PathBuf;

const BEGIN_MARKER: &str = "# BEGIN GENERATED LITE APIS";
const END_MARKER: &str = "# END GENERATED LITE APIS";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct VersionParts {
    major: u64,
    minor: u64,
    patch: u64,
}

#[derive(Clone, Copy, Debug)]
enum Bound {
    Inclusive(VersionParts),
    Exclusive(VersionParts),
}

#[derive(Clone, Debug)]
struct Rule {
    expression: String,
    url: String,
    lower: Option<Bound>,
    upper: Option<Bound>,
}

impl Rule {
    fn condition(&self) -> String {
        let mut conditions = Vec::new();
        if let Some(bound) = self.lower {
            conditions.push(bound_condition(bound, true));
        }
        if let Some(bound) = self.upper {
            conditions.push(bound_condition(bound, false));
        }
        conditions.join(" && ")
    }

    #[cfg(test)]
    fn matches(&self, version: VersionParts) -> bool {
        let lower_matches = match self.lower {
            Some(Bound::Inclusive(bound)) => version >= bound,
            Some(Bound::Exclusive(bound)) => version > bound,
            None => true,
        };
        let upper_matches = match self.upper {
            Some(Bound::Inclusive(bound)) => version <= bound,
            Some(Bound::Exclusive(bound)) => version < bound,
            None => true,
        };
        lower_matches && upper_matches
    }
}

fn bound_condition(bound: Bound, lower: bool) -> String {
    let (function, version) = match (lower, bound) {
        (true, Bound::Inclusive(version)) => ("version_at_least", version),
        (true, Bound::Exclusive(version)) => ("version_greater_than", version),
        (false, Bound::Inclusive(version)) => ("version_at_most", version),
        (false, Bound::Exclusive(version)) => ("version_less_than", version),
    };
    format!("{} {} {} {}", function, version.major, version.minor, version.patch)
}

fn parse_version(value: &str, source_name: &str, expression: &str) -> Result<VersionParts> {
    let parts: Vec<&str> = value.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|part| part.is_empty()) {
        bail!(
            "lite source '{}' uses unsupported version '{}' in rule '{}'",
            source_name,
            value,
            expression
        );
    }
    let parse_component = |component: &str| {
        component.parse::<u64>().map_err(|_| {
            eyre!(
                "lite source '{}' uses unsupported version '{}' in rule '{}'",
                source_name,
                value,
                expression
            )
        })
    };
    Ok(VersionParts {
        major: parse_component(parts[0])?,
        minor: parse_component(parts[1])?,
        patch: parse_component(parts[2])?,
    })
}

fn parse_rule(source_name: &str, expression: &str, url: &str) -> Result<Rule> {
    // This also verifies the generated subset and runtime source resolution use
    // the same NPM-style parsing rules.
    parse_npm_version_requirement(expression)?;

    let mut tokens = expression.split_whitespace().peekable();
    let mut lower = None;
    let mut upper = None;
    while let Some(token) = tokens.next() {
        let (operator, version) = match token {
            ">=" | ">" | "<=" | "<" | "=" => (
                token,
                tokens.next().ok_or_else(|| {
                    eyre!(
                        "lite source '{}' has incomplete version rule '{}'",
                        source_name,
                        expression
                    )
                })?,
            ),
            _ => {
                let operator = [">=", "<=", ">", "<", "="]
                    .into_iter()
                    .find(|operator| token.starts_with(operator))
                    .ok_or_else(|| {
                        eyre!(
                            "lite source '{}' uses unsupported version rule '{}'",
                            source_name,
                            expression
                        )
                    })?;
                (operator, &token[operator.len()..])
            }
        };
        let version = parse_version(version, source_name, expression)?;
        match operator {
            ">=" => {
                if lower.replace(Bound::Inclusive(version)).is_some() {
                    bail!("lite source '{}' has ambiguous rule '{}'", source_name, expression);
                }
            }
            ">" => {
                if lower.replace(Bound::Exclusive(version)).is_some() {
                    bail!("lite source '{}' has ambiguous rule '{}'", source_name, expression);
                }
            }
            "<=" => {
                if upper.replace(Bound::Inclusive(version)).is_some() {
                    bail!("lite source '{}' has ambiguous rule '{}'", source_name, expression);
                }
            }
            "<" => {
                if upper.replace(Bound::Exclusive(version)).is_some() {
                    bail!("lite source '{}' has ambiguous rule '{}'", source_name, expression);
                }
            }
            "=" => {
                if lower.replace(Bound::Inclusive(version)).is_some()
                    || upper.replace(Bound::Inclusive(version)).is_some()
                {
                    bail!("lite source '{}' has ambiguous rule '{}'", source_name, expression);
                }
            }
            _ => unreachable!(),
        }
    }

    if lower.is_none() && upper.is_none() {
        bail!(
            "lite source '{}' uses unsupported version rule '{}'",
            source_name,
            expression
        );
    }
    Ok(Rule {
        expression: expression.to_string(),
        url: url.to_string(),
        lower,
        upper,
    })
}

fn bash_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    matches!(characters.next(), Some('_' | 'a'..='z' | 'A'..='Z'))
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn bash_string(value: &str, context: &str) -> Result<String> {
    if value.chars().any(|character| character.is_control()) {
        bail!(
            "{} contains a control character that cannot be rendered in Bash",
            context
        );
    }
    Ok(value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`"))
}

fn lower_bound(rule: &Rule) -> Option<(VersionParts, bool)> {
    rule.lower.map(|bound| match bound {
        Bound::Inclusive(version) => (version, true),
        Bound::Exclusive(version) => (version, false),
    })
}

fn upper_bound(rule: &Rule) -> Option<(VersionParts, bool)> {
    rule.upper.map(|bound| match bound {
        Bound::Inclusive(version) => (version, true),
        Bound::Exclusive(version) => (version, false),
    })
}

fn rules_overlap(left: &Rule, right: &Rule) -> bool {
    let lower = match (lower_bound(left), lower_bound(right)) {
        (Some(left), Some(right)) => match left.0.cmp(&right.0) {
            Ordering::Less => right,
            Ordering::Greater => left,
            Ordering::Equal => (left.0, left.1 && right.1),
        },
        (Some(bound), None) | (None, Some(bound)) => bound,
        (None, None) => return true,
    };
    let upper = match (upper_bound(left), upper_bound(right)) {
        (Some(left), Some(right)) => match left.0.cmp(&right.0) {
            Ordering::Less => left,
            Ordering::Greater => right,
            Ordering::Equal => (left.0, left.1 && right.1),
        },
        (Some(bound), None) | (None, Some(bound)) => bound,
        (None, None) => return true,
    };
    lower.0 < upper.0 || (lower.0 == upper.0 && lower.1 && upper.1)
}

fn render_source(name: &str, output_path: &str, rules: &[Rule]) -> Result<String> {
    if !bash_identifier(name) {
        bail!("lite source '{}' is not a valid Bash function identifier", name);
    }
    if rules.is_empty() {
        bail!("lite source '{}' has no version rules", name);
    }
    for (index, rule) in rules.iter().enumerate() {
        for previous in &rules[..index] {
            if rules_overlap(previous, rule) {
                bail!(
                    "lite source '{}' has ambiguous overlapping rules '{}' and '{}'",
                    name,
                    previous.expression,
                    rule.expression
                );
            }
        }
    }

    let output_path = bash_string(output_path, &format!("lite source '{}' output path", name))?;
    let mut result = String::new();
    writeln!(result, "get_api_{}() {{", name)?;
    for (index, rule) in rules.iter().enumerate() {
        let prefix = if index == 0 { "if" } else { "elif" };
        writeln!(result, "  {} {}; then", prefix, rule.condition())?;
        writeln!(
            result,
            "    get_api \"{}\" \"{}\"",
            bash_string(&rule.url, &format!("lite source '{}' URL", name))?,
            output_path
        )?;
    }
    writeln!(result, "  else")?;
    writeln!(result, "    skip_api \"{}\"", name)?;
    writeln!(result, "  fi")?;
    writeln!(result, "}}")?;
    Ok(result)
}

fn render() -> Result<String> {
    let sources =
        get_product_sources("elasticsearch").ok_or_else(|| eyre!("embedded Elasticsearch sources are unavailable"))?;
    let mut names: Vec<&String> = sources
        .iter()
        .filter_map(|(name, source)| source.has_tag("lite").then_some(name))
        .collect();
    names.sort();

    if !names.iter().any(|name| name.as_str() == "version") {
        bail!("lite source catalog must include the version bootstrap source");
    }

    let mut output = String::new();
    writeln!(
        output,
        "# This region is generated by `cargo run --bin esdiag-lite-generate`. Do not edit."
    )?;
    writeln!(output)?;
    for name in names.iter().filter(|name| name.as_str() != "version") {
        let source = sources.get(name.as_str()).expect("source key from map");
        let rules = source
            .versions
            .iter()
            .map(|(expression, source)| match source {
                VersionSource::Url(url) => parse_rule(name, expression, url),
                VersionSource::Structured(_) => bail!(
                    "lite source '{}' uses structured request details unsupported by esdiag-lite",
                    name
                ),
            })
            .collect::<Result<Vec<_>>>()?;
        output.push_str(&render_source(name, &source.get_file_path(name), &rules)?);
        writeln!(output)?;
    }

    let version = sources.get("version").expect("validated version source");
    let bootstrap_urls: Vec<&str> = version
        .versions
        .values()
        .map(|source| match source {
            VersionSource::Url(url) => Ok(url.as_str()),
            VersionSource::Structured(_) => bail!("lite version source must use a URL"),
        })
        .collect::<Result<_>>()?;
    if bootstrap_urls.len() != 1 || bootstrap_urls[0] != "/" {
        bail!("lite version source must contain exactly one root (/) request");
    }
    writeln!(output, "get_api_version() {{")?;
    writeln!(output, "  get_api \"/\" \"version.json\"")?;
    writeln!(output, "}}")?;
    writeln!(output)?;
    writeln!(output, "collect_lite_apis() {{")?;
    for name in names.iter().filter(|name| name.as_str() != "version") {
        writeln!(output, "  get_api_{}", name)?;
    }
    writeln!(output, "}}")?;
    Ok(output)
}

fn replace_generated_region(script: &str, generated: &str) -> Result<String> {
    let begin = script
        .find(BEGIN_MARKER)
        .ok_or_else(|| eyre!("{} is missing from bin/esdiag-lite.sh", BEGIN_MARKER))?;
    let generated_start = begin + BEGIN_MARKER.len();
    let end = script[generated_start..]
        .find(END_MARKER)
        .map(|offset| generated_start + offset)
        .ok_or_else(|| eyre!("{} is missing from bin/esdiag-lite.sh", END_MARKER))?;
    if script[generated_start..end].contains(BEGIN_MARKER) {
        bail!("bin/esdiag-lite.sh has nested generated-region markers");
    }
    Ok(format!(
        "{}\n{}\n{}",
        &script[..generated_start],
        generated.trim_end(),
        &script[end..]
    ))
}

fn script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/esdiag-lite.sh")
}

fn main() -> Result<()> {
    let check = match std::env::args().nth(1).as_deref() {
        None | Some("--write") => false,
        Some("--check") => true,
        Some(argument) => bail!("usage: esdiag-lite-generate [--write|--check]; unknown argument {argument}"),
    };
    let path = script_path();
    let script =
        std::fs::read_to_string(&path).map_err(|error| eyre!("failed to read {}: {}", path.display(), error))?;
    let updated = replace_generated_region(&script, &render()?)?;
    if check {
        if script != updated {
            bail!("{} is stale; run cargo run --bin esdiag-lite-generate", path.display());
        }
    } else if script != updated {
        std::fs::write(&path, updated).map_err(|error| eyre!("failed to write {}: {}", path.display(), error))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{VersionParts, parse_rule, render, replace_generated_region, rules_overlap, script_path};
    use esdiag::processor::diagnostic::data_source::{VersionSource, get_product_sources};
    use semver::Version;

    #[test]
    fn detects_overlapping_rules() {
        let first = parse_rule("test", ">= 7.0.0", "/first").unwrap();
        let second = parse_rule("test", ">= 7.7.0", "/second").unwrap();
        assert!(rules_overlap(&first, &second));
    }

    #[test]
    fn accepts_bounded_rules() {
        let first = parse_rule("test", ">= 6.6.0 < 7.7.0", "/first").unwrap();
        let second = parse_rule("test", ">= 7.7.0", "/second").unwrap();
        assert!(!rules_overlap(&first, &second));
        assert_eq!(first.condition(), "version_at_least 6 6 0 && version_less_than 7 7 0");
    }

    #[test]
    fn generated_rules_match_source_resolution_at_boundaries() {
        let samples = [
            "0.9.0", "2.0.0", "6.4.0", "6.6.0", "7.4.0", "7.6.2", "7.7.0", "7.10.0", "7.11.0", "7.13.0", "8.0.0",
        ];
        let sources = get_product_sources("elasticsearch").unwrap();

        for (name, source) in sources.iter().filter(|(_, source)| source.has_tag("lite")) {
            if name == "version" {
                continue;
            }
            let rules = source
                .versions
                .iter()
                .map(|(expression, source)| match source {
                    VersionSource::Url(url) => parse_rule(name, expression, url),
                    VersionSource::Structured(_) => unreachable!("lite sources use URL requests"),
                })
                .collect::<eyre::Result<Vec<_>>>()
                .unwrap();

            for sample in samples {
                let version = Version::parse(sample).unwrap();
                let parts = VersionParts {
                    major: version.major,
                    minor: version.minor,
                    patch: version.patch,
                };
                let selected: Vec<&str> = rules
                    .iter()
                    .filter(|rule| rule.matches(parts))
                    .map(|rule| rule.url.as_str())
                    .collect();
                let expected = source.get_url(&version).ok();
                assert!(
                    selected.len() <= 1,
                    "{} selected multiple generated rules for {}",
                    name,
                    sample
                );
                assert_eq!(
                    selected.first().copied(),
                    expected.as_deref(),
                    "generated rule mismatch for {} at {}",
                    name,
                    sample
                );
            }
        }
    }

    #[test]
    fn checked_in_generated_region_is_current() {
        let path = script_path();
        let script = std::fs::read_to_string(&path).unwrap();
        assert_eq!(script, replace_generated_region(&script, &render().unwrap()).unwrap());
    }
}
