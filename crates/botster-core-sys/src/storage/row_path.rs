//! Where a registry row lives: a reversible path, so that every row file that Core wrote names its own key (Core AD-1, AD-2,
//! A10-2; lead ruling on audit A1).
//!
//! A key is `<kind>/<id>`. The kind is a directory of the registry. The id is encoded as lowercase base32 with no padding, and
//! the code is cut into components of [`COMPONENT_CHARS`] characters: every component but the last is a directory, and the
//! last one, with [`ROW_SUFFIX`], is the row file. So any id fits, whatever its length (`CoreLimits.max_session_id_bytes` has
//! no upper bound), and a path that decodes names exactly one key.
//!
//! A name that does not decode this way was not written by Core: it is not a row.

use data_encoding::{Encoding, Specification};
use std::sync::LazyLock;

/// The longest component, in characters. `NAME_MAX` is 255 on Linux and on macOS; 200 leaves room for [`ROW_SUFFIX`] and
/// for the prefix of a temporary file (`TEMP_PREFIX` in the storage) on the last component.
pub(crate) const COMPONENT_CHARS: usize = 200;

/// The suffix of a row file.
pub(crate) const ROW_SUFFIX: &str = ".row";

/// Lowercase base32 (RFC 4648 alphabet, in lower case), no padding. Decoding checks the unused trailing bits, so each id has
/// one code.
static BASE32: LazyLock<Encoding> = LazyLock::new(|| {
    let mut spec = Specification::new();
    spec.symbols.push_str("abcdefghijklmnopqrstuvwxyz234567");
    spec.encoding()
        .expect("a base32 alphabet of 32 distinct symbols")
});

/// The path of one row, relative to the registry directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RowPath {
    pub kind: String,
    /// The directories below the kind, in order.
    pub dirs: Vec<String>,
    /// The row file, with its suffix.
    pub file: String,
}

/// Whether `kind` is a kind directory: lowercase letters and `-`, not empty.
pub(crate) fn valid_kind(kind: &str) -> bool {
    !kind.is_empty() && kind.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
}

/// The path of the row of `key`, or `None` when the key has no valid kind.
pub(crate) fn row_path(key: &str) -> Option<RowPath> {
    let (kind, id) = key.split_once('/')?;
    if !valid_kind(kind) {
        return None;
    }
    let code = BASE32.encode(id.as_bytes());
    let mut chunks: Vec<String> = code
        .as_bytes()
        .chunks(COMPONENT_CHARS)
        .map(|c| String::from_utf8(c.to_vec()).expect("base32 is ASCII"))
        .collect();
    let last = chunks.pop().unwrap_or_default();
    Some(RowPath {
        kind: kind.to_string(),
        dirs: chunks,
        file: format!("{last}{ROW_SUFFIX}"),
    })
}

/// The key that a path names, or `None` when Core did not write it: a kind that is not one, a directory component that is
/// not a full component, a last component that is empty or too long (unless it is the only one), or a code that does not
/// decode to UTF-8 text.
pub(crate) fn key_of(kind: &str, dirs: &[&str], file: &str) -> Option<String> {
    if !valid_kind(kind) {
        return None;
    }
    let last = file.strip_suffix(ROW_SUFFIX)?;
    if dirs.iter().any(|d| d.len() != COMPONENT_CHARS) || last.len() > COMPONENT_CHARS {
        return None;
    }
    if last.is_empty() && !dirs.is_empty() {
        return None;
    }
    let code: String = dirs.iter().copied().chain([last]).collect();
    let id = String::from_utf8(BASE32.decode(code.as_bytes()).ok()?).ok()?;
    Some(format!("{kind}/{id}"))
}

/// Whether `name` may be a directory component of a row path (a full component of the code). A walk descends only into
/// these.
pub(crate) fn is_dir_component(name: &str) -> bool {
    name.len() == COMPONENT_CHARS
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(key: &str) -> RowPath {
        let path = row_path(key).expect("a key with a kind");
        let dirs: Vec<&str> = path.dirs.iter().map(String::as_str).collect();
        assert_eq!(
            key_of(&path.kind, &dirs, &path.file).as_deref(),
            Some(key),
            "{key}"
        );
        path
    }

    /// Lead ruling on A1: a key survives its path. An id of 128 bytes (the default `max_session_id_bytes`) and ids well above
    /// it, which take several components, all round-trip; every component fits the bound, and only the last is a file.
    #[test]
    fn a_key_survives_its_path_at_every_length() {
        for len in [0, 1, 5, 127, 128, 129, 200, 1000, 4096] {
            let key = format!("session/{}", "é".repeat(len / 2) + &"x".repeat(len % 2));
            let path = round_trip(&key);
            let code_len = BASE32.encode_len(key.len() - "session/".len());
            assert_eq!(
                path.dirs.len(),
                code_len.saturating_sub(1) / COMPONENT_CHARS,
                "{len}"
            );
            assert!(path.dirs.iter().all(|d| d.len() == COMPONENT_CHARS));
            assert!(path.file.len() <= COMPONENT_CHARS + ROW_SUFFIX.len());
        }
        round_trip("meta/host-epoch");
    }

    /// Two keys never share a path.
    #[test]
    fn distinct_keys_have_distinct_paths() {
        let keys = [
            "session/a",
            "session/b",
            "session/",
            "session/../x",
            "meta/a",
        ];
        let paths: Vec<RowPath> = keys.iter().map(|k| row_path(k).unwrap()).collect();
        for (i, p) in paths.iter().enumerate() {
            assert!(!paths[..i].contains(p), "{p:?}");
        }
    }

    /// A name that Core did not write is not a row: no suffix, a foreign character, an upper-case code, a short directory
    /// component, an empty last component after directories, or a kind that is not one.
    #[test]
    fn a_name_that_core_did_not_write_is_not_a_row() {
        let full = "a".repeat(COMPONENT_CHARS);
        assert_eq!(key_of("session", &[], "mfrgg"), None, "no suffix");
        assert_eq!(key_of("session", &[], "mfr!g.row"), None);
        assert_eq!(key_of("session", &[], "MFRGG.row"), None);
        assert_eq!(key_of("session", &["abc"], "mfrgg.row"), None);
        assert_eq!(key_of("session", &[&full], ".row"), None);
        assert_eq!(key_of("Session", &[], "mfrgg.row"), None);
        assert_eq!(key_of("session", &[], ".tmp.1.row"), None);
        assert!(row_path("no-kind").is_none());
        assert!(is_dir_component(&full));
        assert!(!is_dir_component("abc"));
    }

    /// The cut is part of the name: a code that decodes is still not a row when it is cut in another place than
    /// [`row_path`] cuts it. A short directory component is refused even when the joined code names a key, and so is a last
    /// component above [`COMPONENT_CHARS`] even when there is no directory.
    #[test]
    fn a_code_cut_in_another_place_is_not_a_row() {
        let short = BASE32.encode(b"ab");
        let (dir, last) = short.split_at(2);
        assert_eq!(
            key_of("session", &[], &format!("{short}{ROW_SUFFIX}")).as_deref(),
            Some("session/ab"),
            "the same code, cut as row_path cuts it"
        );
        assert_eq!(
            key_of("session", &[dir], &format!("{last}{ROW_SUFFIX}")),
            None
        );

        let long = BASE32.encode(&[b'a'; 130]);
        assert!(long.len() > COMPONENT_CHARS);
        assert_eq!(key_of("session", &[], &format!("{long}{ROW_SUFFIX}")), None);
    }
}
