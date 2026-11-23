//! In-place edits of a markdown artifact's frontmatter block.
//!
//! Edits work on the text of the block, never on a parsed and reserialized
//! form, so every byte outside the edited line is preserved: key order,
//! quoting, comments and the body are left exactly as written.
//!
//! [`insert_if_absent`] and [`remove`] edit top-level keys holding a
//! single-line scalar only, refusing a key whose value continues onto
//! further lines rather than risk leaving part of its value behind.
//!
//! [`set`] and [`unset`] edit a whole top-level entry, the key line and every
//! continuation line of its value, so block lists and other multi-line values
//! are replaced cleanly. They first check with [`validate`] that the text
//! view of the block agrees with its parsed view, and refuse to edit
//! frontmatter they cannot split into entries safely.

use anyhow::{Context, Result, bail};
use serde_yaml::{Mapping, Value};

/// Append `key: value` as the last frontmatter line when `key` is absent.
///
/// Returns the edited text, or `None` when `key` is already present, in which
/// case its existing value is kept.
pub fn insert_if_absent(text: &str, key: &str, value: &str) -> Result<Option<String>> {
    if value.contains(['\n', '\r']) {
        bail!("frontmatter value for '{key}' must be a single line");
    }
    let block = Block::parse(text)?;
    if block.find(key)?.is_some() {
        return Ok(None);
    }
    let at = block.closing_start;
    let line = format!("{key}: {value}{}", block.newline);
    Ok(Some(format!("{}{line}{}", &text[..at], &text[at..])))
}

/// Remove the line holding `key`.
///
/// Returns the edited text, or `None` when `key` is absent.
pub fn remove(text: &str, key: &str) -> Result<Option<String>> {
    let block = Block::parse(text)?;
    let Some(line) = block.find(key)? else {
        return Ok(None);
    };
    Ok(Some(format!(
        "{}{}",
        &text[..line.start],
        &text[line.end..]
    )))
}

/// Parse the frontmatter and check that it can be edited entry by entry.
///
/// Fails when the block is missing or unterminated, holds duplicate keys,
/// is not a mapping, or contains a line that cannot be attributed to a
/// top-level entry, or when parsing an entry on its own does not give the
/// value the whole block gives it. Returns the parsed mapping.
pub fn validate(text: &str) -> Result<Mapping> {
    let block = Block::parse(text)?;
    validated(text, &block)
}

/// Set top-level `key` to `value`, replacing its entry including every
/// continuation line, or appending it as the last entry when absent.
///
/// The value is rendered by serde_yaml, so quoting follows YAML rules.
pub fn set(text: &str, key: &str, value: &Value) -> Result<String> {
    let block = Block::parse(text)?;
    validated(text, &block)?;
    let mut entry = Mapping::new();
    entry.insert(Value::String(key.to_string()), value.clone());
    let mut rendered = serde_yaml::to_string(&entry)?;
    if block.newline == "\r\n" {
        rendered = rendered.replace('\n', "\r\n");
    }
    let (start, end) = match block.entries()?.iter().find(|entry| entry.key == key) {
        Some(entry) => block.span(entry),
        None => (block.closing_start, block.closing_start),
    };
    Ok(format!("{}{rendered}{}", &text[..start], &text[end..]))
}

/// Remove top-level `key` with every continuation line of its value.
///
/// Returns the edited text, or `None` when `key` is absent.
pub fn unset(text: &str, key: &str) -> Result<Option<String>> {
    let block = Block::parse(text)?;
    validated(text, &block)?;
    let entries = block.entries()?;
    let Some(entry) = entries.iter().find(|entry| entry.key == key) else {
        return Ok(None);
    };
    let (start, end) = block.span(entry);
    Ok(Some(format!("{}{}", &text[..start], &text[end..])))
}

fn validated(text: &str, block: &Block<'_>) -> Result<Mapping> {
    let yaml = &text[block.body_start..block.closing_start];
    let parsed: Value = serde_yaml::from_str(yaml).context("frontmatter cannot be parsed")?;
    let mapping = match parsed {
        Value::Null => Mapping::new(),
        Value::Mapping(mapping) => mapping,
        _ => bail!("frontmatter is not a mapping"),
    };

    let entries = block.entries()?;
    let mut seen = std::collections::HashSet::new();
    for entry in &entries {
        if !seen.insert(entry.key) {
            bail!("frontmatter key '{}' appears more than once", entry.key);
        }
        let (start, end) = block.span(entry);
        let unsafe_key_message =
            || format!("frontmatter key '{}' cannot be edited safely", entry.key);
        let alone: Mapping =
            serde_yaml::from_str(&text[start..end]).with_context(unsafe_key_message)?;
        let key = Value::String(entry.key.to_string());
        if alone.len() != 1 || alone.get(&key) != mapping.get(&key) {
            bail!(unsafe_key_message());
        }
    }
    if entries.len() != mapping.len() {
        bail!("frontmatter holds keys that cannot be edited safely");
    }
    Ok(mapping)
}

/// A fence line, recognised as `artifact::extract_frontmatter_yaml` does so
/// the editor and the reader agree on where the block ends.
fn is_fence(content: &str) -> bool {
    content.trim() == "---"
}

/// The frontmatter lines of a document, as byte ranges into it.
struct Block<'a> {
    lines: Vec<Line<'a>>,
    /// Byte offset of the first line after the opening fence.
    body_start: usize,
    /// Byte offset of the closing fence line.
    closing_start: usize,
    /// Line ending of the opening fence, reused for inserted lines.
    newline: &'static str,
}

struct Line<'a> {
    start: usize,
    end: usize,
    /// Content without its line ending.
    content: &'a str,
}

impl<'a> Block<'a> {
    fn parse(text: &'a str) -> Result<Self> {
        let mut offset = 0;
        let mut raw = text.split_inclusive('\n').map(|line| {
            let start = offset;
            offset += line.len();
            Line {
                start,
                end: offset,
                content: line.trim_end_matches(['\n', '\r']),
            }
        });

        let opening = raw.next().filter(|line| is_fence(line.content));
        let Some(opening) = opening else {
            bail!("document has no frontmatter");
        };
        let newline = if text[..opening.end].ends_with("\r\n") {
            "\r\n"
        } else {
            "\n"
        };

        let mut lines = Vec::new();
        for line in raw {
            if is_fence(line.content) {
                return Ok(Self {
                    lines,
                    body_start: opening.end,
                    closing_start: line.start,
                    newline,
                });
            }
            lines.push(line);
        }
        bail!("frontmatter is not terminated")
    }

    /// The top-level entries of the block, each a key line followed by the
    /// continuation lines of its value. Column-zero comment lines end the
    /// current entry; they and trailing blank lines belong to no entry.
    fn entries(&self) -> Result<Vec<Entry<'a>>> {
        let mut entries: Vec<Entry<'a>> = Vec::new();
        let mut open = false;
        for (index, line) in self.lines.iter().enumerate() {
            let content = line.content;
            if content.trim().is_empty() {
                continue;
            }
            // A column-zero '-' continues the entry above: YAML allows block
            // list items at their key's indentation, as cue writes refs.
            if content.starts_with([' ', '\t', '-']) {
                match entries.last_mut() {
                    Some(entry) if open => entry.last = index,
                    _ => bail!("frontmatter line '{content}' belongs to no key"),
                }
                continue;
            }
            if content.starts_with('#') {
                open = false;
                continue;
            }
            let Some(key) = plain_key(content) else {
                bail!("frontmatter line '{content}' is not a plain 'key:' entry");
            };
            entries.push(Entry {
                key,
                first: index,
                last: index,
            });
            open = true;
        }
        Ok(entries)
    }

    /// Byte range of an entry, from its key line to the end of its last
    /// continuation line.
    fn span(&self, entry: &Entry<'_>) -> (usize, usize) {
        (self.lines[entry.first].start, self.lines[entry.last].end)
    }

    /// The line holding top-level `key`, refusing a value that spans lines.
    fn find(&self, key: &str) -> Result<Option<&Line<'a>>> {
        let Some(index) = self.lines.iter().position(|line| {
            line.content
                .strip_prefix(key)
                .and_then(|rest| rest.strip_prefix(':'))
                .is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', '\t']))
        }) else {
            return Ok(None);
        };

        let line = &self.lines[index];
        let value = line.content[key.len() + 1..].trim();
        let continued = self.lines.get(index + 1).is_some_and(|next| {
            next.content.starts_with([' ', '\t']) || next.content.starts_with('-')
        });
        if value.is_empty() || value.starts_with(['|', '>']) || continued {
            bail!("frontmatter key '{key}' does not hold a single-line value");
        }
        Ok(Some(line))
    }
}

struct Entry<'a> {
    key: &'a str,
    first: usize,
    last: usize,
}

/// The key of a top-level `key: value` line, when written as a plain
/// unquoted scalar.
fn plain_key(content: &str) -> Option<&str> {
    let (key, rest) = content.split_once(':')?;
    let plain = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    (plain && (rest.is_empty() || rest.starts_with([' ', '\t']))).then_some(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "---\n\
title: 'Quoted: title'\n\
kind: work\n\
refs:\n\
- a/b/c\n\
created_at: 1792913657 # stamped\n\
---\n\
Body with\n---\nfence-like lines.\n";

    #[test]
    fn insert_appends_last_and_preserves_every_other_byte() {
        let edited = insert_if_absent(DOC, "archived_at", "1800000000")
            .unwrap()
            .unwrap();
        let expected = DOC.replacen(
            "# stamped\n---\n",
            "# stamped\narchived_at: 1800000000\n---\n",
            1,
        );
        assert_eq!(edited, expected);
    }

    #[test]
    fn insert_keeps_an_existing_value() {
        let archived = DOC.replacen("kind: work\n", "kind: work\narchived_at: 1\n", 1);
        assert_eq!(
            insert_if_absent(&archived, "archived_at", "2").unwrap(),
            None
        );
    }

    #[test]
    fn insert_matches_whole_top_level_keys_only() {
        let doc = DOC.replacen(
            "kind: work\n",
            "kind: work\narchived_at_note: x\nmeta:\n  archived_at: 1\n",
            1,
        );
        let edited = insert_if_absent(&doc, "archived_at", "2").unwrap().unwrap();
        assert!(edited.contains("# stamped\narchived_at: 2\n---\n"));
    }

    #[test]
    fn remove_deletes_only_that_line() {
        let archived = DOC.replacen("kind: work\n", "kind: work\narchived_at: 1\n", 1);
        assert_eq!(remove(&archived, "archived_at").unwrap().unwrap(), DOC);
    }

    #[test]
    fn remove_of_an_absent_key_is_unchanged() {
        assert_eq!(remove(DOC, "archived_at").unwrap(), None);
    }

    #[test]
    fn remove_ignores_body_lines() {
        let doc = format!("{DOC}archived_at: 1\n");
        assert_eq!(remove(&doc, "archived_at").unwrap(), None);
    }

    #[test]
    fn multi_line_values_are_refused() {
        assert!(remove(DOC, "refs").is_err());
        let block = DOC.replacen("kind: work\n", "kind: work\narchived_at: >\n  1\n", 1);
        assert!(insert_if_absent(&block, "archived_at", "2").is_err());
    }

    #[test]
    fn values_spanning_lines_are_refused() {
        assert!(insert_if_absent(DOC, "archived_at", "1\nkind: x").is_err());
    }

    #[test]
    fn missing_or_unterminated_frontmatter_is_an_error() {
        assert!(insert_if_absent("no fence\n", "k", "v").is_err());
        assert!(insert_if_absent("---\nkind: work\n", "k", "v").is_err());
        assert!(remove("---\nkind: work\n", "k").is_err());
    }

    #[test]
    fn fences_with_trailing_whitespace_match_the_reader() {
        let doc = "--- \nkind: work\n---\t\nbody\n---\nmore\n";
        let edited = insert_if_absent(doc, "archived_at", "1").unwrap().unwrap();
        assert_eq!(
            edited,
            "--- \nkind: work\narchived_at: 1\n---\t\nbody\n---\nmore\n"
        );
        assert_eq!(remove(&edited, "archived_at").unwrap().unwrap(), doc);
    }

    #[test]
    fn crlf_files_keep_their_line_endings() {
        let doc = "---\r\nkind: work\r\n---\r\nbody\r\n";
        let edited = insert_if_absent(doc, "archived_at", "1").unwrap().unwrap();
        assert_eq!(
            edited,
            "---\r\nkind: work\r\narchived_at: 1\r\n---\r\nbody\r\n"
        );
        assert_eq!(remove(&edited, "archived_at").unwrap().unwrap(), doc);
    }

    fn list(items: &[&str]) -> Value {
        Value::Sequence(items.iter().map(|item| Value::from(*item)).collect())
    }

    const BODY: &str = "---\nBody with\n---\nfence-like lines.\n";

    #[test]
    fn set_replaces_a_scalar_in_place() {
        let edited = set(DOC, "kind", &Value::from("coord")).unwrap();
        assert_eq!(edited, DOC.replacen("kind: work\n", "kind: coord\n", 1));
    }

    #[test]
    fn set_quotes_values_that_need_it() {
        let edited = set(DOC, "title", &Value::from("New: title")).unwrap();
        assert!(edited.contains("\ntitle: 'New: title'\nkind: work\n"));
        assert_eq!(
            validate(&edited).unwrap()["title"],
            Value::from("New: title")
        );
    }

    #[test]
    fn set_replaces_a_block_list_with_all_its_items() {
        let edited = set(DOC, "refs", &list(&["x/y/z", "x/y/w"])).unwrap();
        assert_eq!(
            edited,
            DOC.replacen("refs:\n- a/b/c\n", "refs:\n- x/y/z\n- x/y/w\n", 1)
        );
    }

    #[test]
    fn set_replaces_an_indented_block_list() {
        let doc = "---\nrefs:\n  - a/b/c\n  - a/b/d\nkind: work\n---\nbody\n";
        let edited = set(doc, "refs", &list(&["x/y/z"])).unwrap();
        assert_eq!(edited, "---\nrefs:\n- x/y/z\nkind: work\n---\nbody\n");
    }

    #[test]
    fn set_replaces_a_flow_list() {
        let doc = "---\nrefs: [a/b/c, a/b/d]\nkind: work\n---\nbody\n";
        let edited = set(doc, "refs", &list(&["x/y/z"])).unwrap();
        assert_eq!(edited, "---\nrefs:\n- x/y/z\nkind: work\n---\nbody\n");
    }

    #[test]
    fn set_appends_an_absent_key_last() {
        let edited = set(DOC, "parent", &Value::from("a/b/c")).unwrap();
        assert_eq!(
            edited,
            DOC.replacen("# stamped\n---\n", "# stamped\nparent: a/b/c\n---\n", 1)
        );
    }

    #[test]
    fn edits_leave_the_body_byte_for_byte() {
        let edited = set(DOC, "refs", &list(&["x/y/z"])).unwrap();
        assert!(edited.ends_with(BODY));
        let edited = unset(&edited, "refs").unwrap().unwrap();
        assert!(edited.ends_with(BODY));
        assert!(DOC.ends_with(BODY));
    }

    #[test]
    fn unset_removes_a_block_list_entirely() {
        let edited = unset(DOC, "refs").unwrap().unwrap();
        assert_eq!(edited, DOC.replacen("refs:\n- a/b/c\n", "", 1));
        assert_eq!(unset(&edited, "refs").unwrap(), None);
    }

    #[test]
    fn set_keeps_crlf_line_endings() {
        let doc = "---\r\nrefs:\r\n- a/b/c\r\nkind: work\r\n---\r\nbody\r\n";
        let edited = set(doc, "refs", &list(&["x/y/z", "x/y/w"])).unwrap();
        assert_eq!(
            edited,
            "---\r\nrefs:\r\n- x/y/z\r\n- x/y/w\r\nkind: work\r\n---\r\nbody\r\n"
        );
    }

    #[test]
    fn duplicate_keys_are_refused() {
        let doc = "---\nkind: work\nkind: coord\n---\n";
        assert!(validate(doc).is_err());
        assert!(set(doc, "kind", &Value::from("x")).is_err());
    }

    #[test]
    fn frontmatter_the_editor_cannot_split_is_refused() {
        for doc in [
            "---\n  kind: work\n---\n",
            "---\n'quoted key': 1\n---\n",
            "---\n? complex\n: 1\n---\n",
            "---\n- a\n---\n",
            "---\nbase: &a 1\nother: *a\n---\n",
        ] {
            assert!(validate(doc).is_err(), "accepted: {doc:?}");
            assert!(
                set(doc, "kind", &Value::from("x")).is_err(),
                "edited: {doc:?}"
            );
        }
    }

    #[test]
    fn comments_and_blank_lines_between_entries_are_kept() {
        let doc = "---\n# heading\nkind: work\n\nrefs:\n- a/b/c\n---\n";
        let edited = set(doc, "kind", &Value::from("coord")).unwrap();
        assert_eq!(
            edited,
            "---\n# heading\nkind: coord\n\nrefs:\n- a/b/c\n---\n"
        );
    }
}
