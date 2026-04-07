use napi_derive::napi;
use riri_semver_range::{ParsedRange, VersionPrecision};

fn parse(input: &str) -> napi::Result<ParsedRange> {
    ParsedRange::parse(input).map_err(napi::Error::from_reason)
}

fn to_precision(input: Option<&str>) -> VersionPrecision {
    match input {
        Some("major") => VersionPrecision::Major,
        Some("minor") => VersionPrecision::MajorMinor,
        _ => VersionPrecision::Full,
    }
}

#[napi]
pub fn humanize_range(input: String, precision: Option<String>) -> napi::Result<String> {
    let range = parse(&input)?;
    Ok(range.humanize_with(to_precision(precision.as_deref())))
}

#[napi]
pub fn restrictive_range(range1: String, range2: String) -> napi::Result<String> {
    let parsed_range1 = parse(&range1)?;
    let parsed_range2 = parse(&range2)?;
    let result = riri_semver_range::restrictive_range(&parsed_range1, &parsed_range2);
    Ok(result.humanize())
}

#[napi]
pub fn satisfies(range: String, version: String) -> napi::Result<bool> {
    let parsed_range = parse(&range)?;
    let parsed_version = ::semver::Version::parse(&version)
        .map_err(|error| napi::Error::from_reason(format!("invalid version: {error}")))?;
    Ok(parsed_range.satisfies(&parsed_version))
}

#[napi]
pub fn is_subset_of(range1: String, range2: String) -> napi::Result<bool> {
    let parsed_range1 = parse(&range1)?;
    let parsed_range2 = parse(&range2)?;
    Ok(riri_semver_range::is_subset_of(
        &parsed_range1,
        &parsed_range2,
    ))
}

#[napi]
pub fn intersects(range1: String, range2: String) -> napi::Result<bool> {
    let parsed_range1 = parse(&range1)?;
    let parsed_range2 = parse(&range2)?;
    Ok(riri_semver_range::intersects(
        &parsed_range1,
        &parsed_range2,
    ))
}
