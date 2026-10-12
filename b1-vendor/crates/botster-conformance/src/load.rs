//! Loading transcripts embedded with `include_dir` (design 6.1).

use crate::transcript::Transcript;
use include_dir::{Dir, DirEntry};

/// A transcript file that cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError(pub String);

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LoadError {}

/// Parses one transcript. The file stem must equal the id (design 5.1).
pub fn parse_transcript(file_name: &str, text: &str) -> Result<Transcript, LoadError> {
    let transcript: Transcript =
        serde_json::from_str(text).map_err(|e| LoadError(format!("{file_name}: {e}")))?;
    let stem = file_name.strip_suffix(".json").unwrap_or(file_name);
    if stem != transcript.id {
        return Err(LoadError(format!(
            "{file_name}: the file name must equal the id '{}'",
            transcript.id
        )));
    }
    if transcript.seeds.as_ref().is_some_and(Vec::is_empty) {
        return Err(LoadError(format!(
            "{file_name}: 'seeds' is empty. Omit it to run every seed."
        )));
    }
    Ok(transcript)
}

/// Loads every `*.json` file under `dir`, except files under a directory named `fixtures`. The result is sorted by id.
pub fn load_dir(dir: &Dir<'_>) -> Result<Vec<Transcript>, LoadError> {
    let mut out = Vec::new();
    collect(dir, &mut out)?;
    out.sort_by(|a, b| a.id.cmp(&b.id));
    for pair in out.windows(2) {
        if pair[0].id == pair[1].id {
            return Err(LoadError(format!("duplicate id {}", pair[0].id)));
        }
    }
    Ok(out)
}

fn collect(dir: &Dir<'_>, out: &mut Vec<Transcript>) -> Result<(), LoadError> {
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(sub) => {
                if sub.path().file_name().is_some_and(|n| n == "fixtures") {
                    continue;
                }
                collect(sub, out)?;
            }
            DirEntry::File(file) => {
                let path = file.path();
                if path.extension().is_none_or(|e| e != "json") {
                    continue;
                }
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                let text = file
                    .contents_utf8()
                    .ok_or_else(|| LoadError(format!("{name}: not UTF-8")))?;
                out.push(parse_transcript(name, text)?);
            }
        }
    }
    Ok(())
}

/// Loads the one transcript whose file stem is `id`, and parses no other file. A generated test runs in its own process (nextest), so
/// parsing the whole directory there costs every test the cost of all of them. The file stem equals the id (design 5.1), so two files
/// cannot carry one id under one name; two files with that stem in different directories are an error, as a duplicate id is in [`load_dir`].
pub fn load_one(dir: &Dir<'_>, id: &str) -> Result<Option<Transcript>, LoadError> {
    let mut found = Vec::new();
    find(dir, &format!("{id}.json"), &mut found);
    match found.as_slice() {
        [] => Ok(None),
        [file] => {
            let name = id.to_string() + ".json";
            let text = file
                .contents_utf8()
                .ok_or_else(|| LoadError(format!("{name}: not UTF-8")))?;
            parse_transcript(&name, text).map(Some)
        }
        _ => Err(LoadError(format!("duplicate id {id}"))),
    }
}

fn find<'a>(dir: &'a Dir<'a>, name: &str, out: &mut Vec<&'a include_dir::File<'a>>) {
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(sub) => {
                if sub.path().file_name().is_none_or(|n| n != "fixtures") {
                    find(sub, name, out);
                }
            }
            DirEntry::File(file) => {
                if file.path().file_name().is_some_and(|n| n == name) {
                    out.push(file);
                }
            }
        }
    }
}
