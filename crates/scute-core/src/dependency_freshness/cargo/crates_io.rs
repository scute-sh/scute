use super::super::FetchError;

const INDEX_BASE: &str = "https://index.crates.io";
const USER_AGENT: &str = concat!(
    "scute/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/noma-to/scute)"
);
const TIMEOUT_SECS: u64 = 10;

pub(super) fn fetch_latest_version(name: &str) -> Result<Option<semver::Version>, FetchError> {
    let path = sparse_index_path(name);
    let url = format!("{INDEX_BASE}/{path}");

    let response = minreq::get(&url)
        .with_header("User-Agent", USER_AGENT)
        .with_timeout(TIMEOUT_SECS)
        .send()
        .map_err(|e| FetchError::Failed(format!("crates.io lookup for {name} failed: {e}")))?;

    if response.status_code == 404 {
        return Ok(None);
    }

    if response.status_code != 200 {
        return Err(FetchError::Failed(format!(
            "crates.io returned {} for {name}",
            response.status_code
        )));
    }

    let body = response
        .as_str()
        .map_err(|e| FetchError::Failed(format!("invalid UTF-8 from crates.io for {name}: {e}")))?;

    Ok(parse_latest_stable(body))
}

fn sparse_index_path(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    match lower.len() {
        1 => format!("1/{lower}"),
        2 => format!("2/{lower}"),
        3 => format!("3/{}/{lower}", &lower[..1]),
        _ => format!("{}/{}/{lower}", &lower[..2], &lower[2..4]),
    }
}

fn parse_latest_stable(ndjson: &str) -> Option<semver::Version> {
    ndjson
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|entry| !entry["yanked"].as_bool().unwrap_or(true))
        .filter_map(|entry| {
            let vers = entry["vers"].as_str()?;
            let v = vers.parse::<semver::Version>().ok()?;
            if !v.pre.is_empty() {
                return None;
            }
            Some(v)
        })
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    #[test_case("a", "1/a" ; "single char")]
    #[test_case("cc", "2/cc" ; "two chars")]
    #[test_case("syn", "3/s/syn" ; "three chars")]
    #[test_case("serde", "se/rd/serde" ; "four plus chars")]
    #[test_case("Serde", "se/rd/serde" ; "lowercases name")]
    fn sparse_index_path_follows_length_based_layout(name: &str, expected: &str) {
        assert_eq!(sparse_index_path(name), expected);
    }

    fn ndjson(entries: &[(&str, bool)]) -> String {
        entries
            .iter()
            .map(|(vers, yanked)| format!(r#"{{"vers":"{vers}","yanked":{yanked}}}"#))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn picks_highest_stable_non_yanked_version() {
        let input = ndjson(&[
            ("1.0.0", false),
            ("1.1.0", false),
            ("2.0.0-rc.1", false),
            ("1.2.0", true),
            ("1.0.5", false),
        ]);

        let latest = parse_latest_stable(&input).unwrap();

        assert_eq!(latest.to_string(), "1.1.0");
    }

    #[test]
    fn all_yanked_returns_none() {
        let input = ndjson(&[("1.0.0", true), ("2.0.0", true)]);

        assert!(parse_latest_stable(&input).is_none());
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(parse_latest_stable("").is_none());
    }

    #[test]
    fn skips_pre_release_versions() {
        let input = ndjson(&[("1.0.0", false), ("2.0.0-beta.1", false)]);

        let latest = parse_latest_stable(&input).unwrap();

        assert_eq!(latest.to_string(), "1.0.0");
    }
}
