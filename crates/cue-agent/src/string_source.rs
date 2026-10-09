//! String values that may be written inline or sourced from a file.
//!
//! Wherever a schema accepts a string it also accepts `{"file": PATH}`. The
//! reference is located against the directory of the document that declared
//! it, at parse time, so the base survives any later layering. Reading is a
//! separate step: a value that a later layer overrides is validated but its
//! file is never opened.

use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// One string-valued setting, before any file is read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StringSource {
    Inline(String),
    /// A path already resolved against the declaring document's directory.
    File(PathBuf),
}

impl StringSource {
    /// Validate one JSON value as a string or a single-key file reference.
    pub fn parse(value: &Value, base: &Path) -> Result<Self> {
        match value {
            Value::String(text) => Ok(Self::Inline(text.clone())),
            Value::Object(map) => {
                let (Some(Value::String(file)), 1) = (map.get("file"), map.len()) else {
                    bail!(
                        "expected a string or a file reference {{\"file\": PATH}} with no other keys"
                    );
                };
                if file.is_empty() {
                    bail!("the file reference names an empty path");
                }
                Ok(Self::File(base.join(file)))
            }
            other => bail!(
                "expected a string or a file reference {{\"file\": PATH}}, found {}",
                kind(other)
            ),
        }
    }

    /// The literal string: inline text as written, or file contents verbatim.
    pub fn resolve(&self) -> Result<String> {
        match self {
            Self::Inline(text) => Ok(text.clone()),
            Self::File(path) => std::fs::read_to_string(path)
                .with_context(|| format!("could not read referenced file {}", path.display())),
        }
    }
}

/// The JSON type of a value, for error messages.
pub fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// An optional field in one layer: absent, explicitly cleared, or set.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Patch<T> {
    #[default]
    Absent,
    Clear,
    Set(T),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_inline_string_is_kept_as_written() {
        let source = StringSource::parse(&json!("  text\n"), Path::new("/base")).unwrap();
        assert_eq!(source, StringSource::Inline("  text\n".into()));
        assert_eq!(source.resolve().unwrap(), "  text\n");
    }

    #[test]
    fn a_relative_file_reference_resolves_against_the_declaring_directory() {
        let source =
            StringSource::parse(&json!({"file": "prompts/a.md"}), Path::new("/base")).unwrap();
        assert_eq!(source, StringSource::File("/base/prompts/a.md".into()));
        let source =
            StringSource::parse(&json!({"file": "/abs/a.md"}), Path::new("/base")).unwrap();
        assert_eq!(source, StringSource::File("/abs/a.md".into()));
    }

    #[test]
    fn parsing_never_opens_the_file() {
        let source =
            StringSource::parse(&json!({"file": "missing.md"}), Path::new("/nonexistent")).unwrap();
        let error = format!("{:#}", source.resolve().unwrap_err());
        assert!(error.contains("/nonexistent/missing.md"), "{error}");
    }

    #[test]
    fn file_contents_are_verbatim_and_never_expanded() {
        let dir = tempfile::tempdir().unwrap();
        let text = "# Title\n\n> \"quoted\" \\ back\\slash\n{\"file\": \"other.md\"}\n\n";
        std::fs::write(dir.path().join("a.md"), text).unwrap();
        let source = StringSource::parse(&json!({"file": "a.md"}), dir.path()).unwrap();
        assert_eq!(source.resolve().unwrap(), text);
    }

    #[test]
    fn malformed_values_are_rejected() {
        for value in [
            json!(null),
            json!(1),
            json!(true),
            json!(["a"]),
            json!({}),
            json!({"file": 1}),
            json!({"file": ""}),
            json!({"file": {"file": "a.md"}}),
            json!({"file": "a.md", "extra": 1}),
            json!({"path": "a.md"}),
        ] {
            assert!(
                StringSource::parse(&value, Path::new("/base")).is_err(),
                "{value} must be rejected"
            );
        }
    }
}
