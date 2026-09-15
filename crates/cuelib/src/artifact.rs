use anyhow::Result;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

const FRONTMATTER_MAX_LINES: usize = 64;

/// Walk `dir` recursively and return all file paths.
///
/// Returns an empty vector if `dir` does not exist or is not a directory.
pub fn collect_files(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    fs::read_dir(dir)?
        .map(|entry| -> Result<Vec<PathBuf>> {
            let path = entry?.path();
            if path.is_dir() {
                collect_files(&path)
            } else {
                Ok(vec![path])
            }
        })
        .collect::<Result<Vec<_>>>()
        .map(|paths| paths.into_iter().flatten().collect())
}

/// Extract the YAML block between the opening and closing frontmatter fences.
pub fn extract_frontmatter_yaml(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();

    reader.read_line(&mut line).ok()?;
    if line.trim() != "---" {
        return None;
    }

    let mut yaml = String::new();
    for _ in 0..FRONTMATTER_MAX_LINES {
        line.clear();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        if line.trim() == "---" {
            return Some(yaml);
        }
        yaml.push_str(&line);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};

    #[test]
    fn collect_files_returns_empty_for_missing_dir() {
        let dir = TempDir::new().unwrap();
        assert!(
            collect_files(&dir.path().join("nonexistent"))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn collect_files_finds_nested_files() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(dir.path().join("a.md"), "").unwrap();
        fs::write(sub.join("b.md"), "").unwrap();

        let files = collect_files(dir.path()).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|path| path.ends_with("a.md")));
        assert!(files.iter().any(|path| path.ends_with("b.md")));
    }

    #[test]
    fn collect_files_returns_empty_for_dir_with_only_subdirs() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("sub1")).unwrap();
        fs::create_dir_all(dir.path().join("sub2")).unwrap();
        assert!(collect_files(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn frontmatter_is_extracted_from_a_valid_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "---\nstatus: open\n---\n# Body").unwrap();

        assert_eq!(
            extract_frontmatter_yaml(file.path()).as_deref(),
            Some("status: open\n")
        );
    }

    #[test]
    fn invalid_frontmatter_returns_none() {
        let mut unfenced = NamedTempFile::new().unwrap();
        writeln!(unfenced, "# Just a header").unwrap();
        assert!(extract_frontmatter_yaml(unfenced.path()).is_none());

        let mut unclosed = NamedTempFile::new().unwrap();
        writeln!(unclosed, "---\nstatus: open").unwrap();
        assert!(extract_frontmatter_yaml(unclosed.path()).is_none());

        let empty = NamedTempFile::new().unwrap();
        assert!(extract_frontmatter_yaml(empty.path()).is_none());
    }
}
