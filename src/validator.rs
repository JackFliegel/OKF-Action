//! OKF v0.1 conformance validator.
//!
//! Conformance rules (spec §9):
//!   1. Every non-reserved `.md` file must contain a parseable YAML frontmatter block.
//!   2. Every such frontmatter block must contain a non-empty `type` field.
//!   3. Reserved filenames (`index.md`, `log.md`) must follow their respective structures (§6, §7).

use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// A single validation issue (error or warning).
#[derive(Debug)]
pub struct Issue {
    pub file: String,
    pub message: String,
}

/// The result of validating an OKF bundle.
#[derive(Debug, Default)]
pub struct ValidationReport {
    pub files_checked: usize,
    pub error_count: usize,
    pub warning_count: usize,
    pub errors: Vec<Issue>,
    pub warnings: Vec<Issue>,
}

impl ValidationReport {
    fn add_error(&mut self, file: &str, message: impl Into<String>) {
        self.error_count += 1;
        self.errors.push(Issue {
            file: file.to_string(),
            message: message.into(),
        });
    }

    fn add_warning(&mut self, file: &str, message: impl Into<String>) {
        self.warning_count += 1;
        self.warnings.push(Issue {
            file: file.to_string(),
            message: message.into(),
        });
    }
}

/// Validate all `.md` files in `bundle_root` for OKF v0.1 conformance.
pub fn validate_bundle(bundle_root: &Path) -> Result<ValidationReport, String> {
    if !bundle_root.exists() {
        return Err(format!(
            "Bundle path does not exist: {}",
            bundle_root.display()
        ));
    }
    if !bundle_root.is_dir() {
        return Err(format!(
            "Bundle path is not a directory: {}",
            bundle_root.display()
        ));
    }

    let mut report = ValidationReport::default();

    for entry in WalkDir::new(bundle_root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "md")
                .unwrap_or(false)
        })
    {
        let path = entry.path();
        let rel_path = path.strip_prefix(bundle_root).unwrap_or(path);
        let rel_str = rel_path.to_string_lossy().to_string();
        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        report.files_checked += 1;

        match filename.as_str() {
            "index.md" => validate_index_md(path, rel_path, &mut report),
            "log.md" => validate_log_md(path, &rel_str, &mut report),
            _ => validate_concept(path, &rel_str, &mut report),
        }
    }

    Ok(report)
}

/// Parse YAML frontmatter from markdown content.
///
/// Returns:
/// - `None`             — no frontmatter delimiter found.
/// - `Some(Err(msg))`   — delimiter found but YAML is invalid or block unclosed.
/// - `Some(Ok(value))`  — successfully parsed frontmatter.
fn parse_frontmatter(content: &str) -> Option<Result<serde_yaml::Value, String>> {
    // Strip optional UTF-8 BOM.
    let content = content.strip_prefix('\u{FEFF}').unwrap_or(content);

    // Frontmatter must begin at column 0 with "---" followed immediately by a newline.
    if !content.starts_with("---\n") && !content.starts_with("---\r\n") {
        return None;
    }

    let after_open = if content.starts_with("---\r\n") {
        &content[5..]
    } else {
        &content[4..]
    };

    // Locate the closing "---" on its own line.
    let (yaml_str, _body) = if let Some(pos) = after_open.find("\n---\n") {
        (&after_open[..pos], &after_open[pos + 5..])
    } else if let Some(pos) = after_open.find("\n---\r\n") {
        (&after_open[..pos], &after_open[pos + 6..])
    } else if after_open.ends_with("\n---") {
        let end = after_open.len() - 4;
        (&after_open[..end], "")
    } else {
        return Some(Err(
            "Frontmatter block is not closed with '---'".to_string()
        ));
    };

    Some(
        serde_yaml::from_str(yaml_str)
            .map_err(|e| format!("Invalid YAML frontmatter: {}", e)),
    )
}

// ---------------------------------------------------------------------------
// Per-file validators
// ---------------------------------------------------------------------------

/// Validate a concept document (any `.md` file that is not reserved).
fn validate_concept(path: &Path, rel_str: &str, report: &mut ValidationReport) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            report.add_error(rel_str, format!("Failed to read file: {}", e));
            return;
        }
    };

    match parse_frontmatter(&content) {
        None => {
            report.add_error(
                rel_str,
                "Missing YAML frontmatter block (OKF v0.1 §9 conformance rule 1)",
            );
        }
        Some(Err(e)) => {
            report.add_error(
                rel_str,
                format!(
                    "Unparseable YAML frontmatter: {} (OKF v0.1 §9 conformance rule 1)",
                    e
                ),
            );
        }
        Some(Ok(fm)) => {
            validate_type_field(&fm, rel_str, report);
        }
    }
}

/// Check that the frontmatter contains a non-empty `type` field (§9 rule 2).
fn validate_type_field(
    fm: &serde_yaml::Value,
    rel_str: &str,
    report: &mut ValidationReport,
) {
    match fm.get("type") {
        None => {
            report.add_error(
                rel_str,
                "Missing required 'type' field in frontmatter (OKF v0.1 §9 conformance rule 2)",
            );
        }
        Some(v) => {
            let is_empty = match v {
                serde_yaml::Value::Null => true,
                serde_yaml::Value::String(s) => s.trim().is_empty(),
                _ => false,
            };
            if is_empty {
                report.add_error(
                    rel_str,
                    "The 'type' field must not be empty (OKF v0.1 §9 conformance rule 2)",
                );
            }
        }
    }
}

/// Validate an `index.md` file (§6 and §11).
///
/// Rules:
/// - Only the bundle-root `index.md` may carry frontmatter (for `okf_version`).
/// - Non-root `index.md` files must NOT have frontmatter.
fn validate_index_md(path: &Path, rel_path: &Path, report: &mut ValidationReport) {
    let rel_str = rel_path.to_string_lossy().to_string();
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            report.add_error(&rel_str, format!("Failed to read file: {}", e));
            return;
        }
    };

    let is_root_index = rel_path
        .parent()
        .map(|p| p == Path::new(""))
        .unwrap_or(false);

    let has_frontmatter_marker = content
        .strip_prefix('\u{FEFF}')
        .unwrap_or(&content)
        .starts_with("---");

    if has_frontmatter_marker {
        if !is_root_index {
            // Only the root index.md may have frontmatter (§11).
            report.add_error(
                &rel_str,
                "Non-root index.md must not have frontmatter (OKF v0.1 §6 and §11)",
            );
        } else {
            // Root index.md: frontmatter must be parseable.
            match parse_frontmatter(&content) {
                Some(Err(e)) => {
                    report.add_error(
                        &rel_str,
                        format!("Root index.md has invalid frontmatter: {}", e),
                    );
                }
                Some(Ok(fm)) => {
                    // okf_version, if present, should be a string.
                    if let Some(v) = fm.get("okf_version") {
                        if v.as_str().is_none() {
                            report.add_warning(
                                &rel_str,
                                "okf_version in root index.md should be a string (e.g. \"0.1\")",
                            );
                        }
                    }
                }
                None => {}
            }
        }
    }
}

/// Validate a `log.md` file (§7).
///
/// Rules:
/// - Must not contain frontmatter.
/// - Level-2 headings (`## …`) must use ISO 8601 `YYYY-MM-DD` date format.
fn validate_log_md(path: &Path, rel_str: &str, report: &mut ValidationReport) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            report.add_error(rel_str, format!("Failed to read file: {}", e));
            return;
        }
    };

    let has_frontmatter_marker = content
        .strip_prefix('\u{FEFF}')
        .unwrap_or(&content)
        .starts_with("---");

    if has_frontmatter_marker {
        report.add_warning(rel_str, "log.md should not have frontmatter (OKF v0.1 §7)");
    }

    // Every level-2 heading must be an ISO 8601 date (YYYY-MM-DD).
    for line in content.lines() {
        if let Some(heading) = line.trim().strip_prefix("## ") {
            let date_str = heading.trim();
            if !is_iso_date(date_str) {
                report.add_error(
                    rel_str,
                    format!(
                        "log.md date heading '{}' must be in ISO 8601 YYYY-MM-DD format (OKF v0.1 §7)",
                        date_str
                    ),
                );
            }
        }
    }
}

/// Returns `true` if `s` matches the pattern `YYYY-MM-DD` using only ASCII digits.
fn is_iso_date(s: &str) -> bool {
    if s.len() != 10 {
        return false;
    }
    let b = s.as_bytes();
    b[4] == b'-'
        && b[7] == b'-'
        && b[..4].iter().all(|c| c.is_ascii_digit())
        && b[5..7].iter().all(|c| c.is_ascii_digit())
        && b[8..10].iter().all(|c| c.is_ascii_digit())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(dir: &Path, name: &str, content: &str) {
        fs::write(dir.join(name), content).unwrap();
    }

    // --- is_iso_date ---------------------------------------------------------

    #[test]
    fn iso_date_valid() {
        assert!(is_iso_date("2026-05-28"));
        assert!(is_iso_date("2000-01-01"));
        assert!(is_iso_date("9999-12-31"));
    }

    #[test]
    fn iso_date_invalid() {
        assert!(!is_iso_date("2026/05/28"));
        assert!(!is_iso_date("26-05-28"));
        assert!(!is_iso_date("2026-5-28"));
        assert!(!is_iso_date("not-a-date"));
        assert!(!is_iso_date("May 28 2026"));
        assert!(!is_iso_date(""));
    }

    // --- parse_frontmatter ---------------------------------------------------

    #[test]
    fn frontmatter_valid() {
        let content = "---\ntype: BigQuery Table\ntitle: Orders\n---\n# Body";
        let result = parse_frontmatter(content).unwrap().unwrap();
        assert_eq!(result.get("type").and_then(|v| v.as_str()), Some("BigQuery Table"));
    }

    #[test]
    fn frontmatter_none_when_absent() {
        assert!(parse_frontmatter("# No frontmatter\n").is_none());
    }

    #[test]
    fn frontmatter_unclosed() {
        let content = "---\ntype: Table\n";
        let result = parse_frontmatter(content).unwrap();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not closed"));
    }

    #[test]
    fn frontmatter_invalid_yaml() {
        // Unmatched bracket makes serde_yaml fail.
        let content = "---\ntype: [\nbroken\n---\n";
        let result = parse_frontmatter(content).unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn frontmatter_strips_bom() {
        let content = "\u{FEFF}---\ntype: Table\n---\n";
        let result = parse_frontmatter(content).unwrap().unwrap();
        assert_eq!(result.get("type").and_then(|v| v.as_str()), Some("Table"));
    }

    // --- validate_bundle errors on bad path ----------------------------------

    #[test]
    fn bundle_path_not_found() {
        let err = validate_bundle(Path::new("/tmp/__does_not_exist__")).unwrap_err();
        assert!(err.contains("does not exist"));
    }

    #[test]
    fn bundle_path_is_file() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("file.md");
        fs::write(&f, "").unwrap();
        let err = validate_bundle(&f).unwrap_err();
        assert!(err.contains("not a directory"));
    }

    // --- concept validation --------------------------------------------------

    #[test]
    fn concept_valid() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "concept.md", "---\ntype: BigQuery Table\n---\n# Body\n");
        let report = validate_bundle(dir.path()).unwrap();
        assert_eq!(report.files_checked, 1);
        assert_eq!(report.error_count, 0);
    }

    #[test]
    fn concept_missing_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "concept.md", "# Just a heading\n\nNo frontmatter.");
        let report = validate_bundle(dir.path()).unwrap();
        assert_eq!(report.error_count, 1);
        assert!(report.errors[0].message.contains("Missing YAML frontmatter"));
    }

    #[test]
    fn concept_missing_type() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "concept.md", "---\ntitle: No Type\n---\n# Body\n");
        let report = validate_bundle(dir.path()).unwrap();
        assert_eq!(report.error_count, 1);
        assert!(report.errors[0].message.contains("'type'"));
    }

    #[test]
    fn concept_null_type() {
        let dir = tempfile::tempdir().unwrap();
        // YAML null value for type
        write(dir.path(), "concept.md", "---\ntype: ~\n---\n# Body\n");
        let report = validate_bundle(dir.path()).unwrap();
        assert_eq!(report.error_count, 1);
        assert!(report.errors[0].message.contains("must not be empty"));
    }

    #[test]
    fn concept_empty_string_type() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "concept.md", "---\ntype: \"\"\n---\n# Body\n");
        let report = validate_bundle(dir.path()).unwrap();
        assert_eq!(report.error_count, 1);
    }

    #[test]
    fn concept_unparseable_yaml() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "concept.md", "---\n: bad: yaml:\n---\n");
        let report = validate_bundle(dir.path()).unwrap();
        // Either a parse error or missing type error is acceptable.
        assert!(report.error_count >= 1);
    }

    // --- index.md validation -------------------------------------------------

    #[test]
    fn root_index_without_frontmatter_ok() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "index.md", "# Index\n\n* [Concept](concept.md)\n");
        let report = validate_bundle(dir.path()).unwrap();
        assert_eq!(report.error_count, 0);
    }

    #[test]
    fn root_index_with_okf_version_ok() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "index.md",
            "---\nokf_version: \"0.1\"\n---\n# Index\n",
        );
        let report = validate_bundle(dir.path()).unwrap();
        assert_eq!(report.error_count, 0);
        assert_eq!(report.warning_count, 0);
    }

    #[test]
    fn non_root_index_with_frontmatter_errors() {
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("tables");
        fs::create_dir_all(&subdir).unwrap();
        write(&subdir, "index.md", "---\ntype: Index\n---\n# Tables\n");
        let report = validate_bundle(dir.path()).unwrap();
        assert_eq!(report.error_count, 1);
        assert!(report.errors[0].message.contains("Non-root index.md"));
    }

    #[test]
    fn non_root_index_without_frontmatter_ok() {
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("tables");
        fs::create_dir_all(&subdir).unwrap();
        write(&subdir, "index.md", "# Tables\n\n* [Orders](orders.md)\n");
        let report = validate_bundle(dir.path()).unwrap();
        assert_eq!(report.error_count, 0);
    }

    // --- log.md validation ---------------------------------------------------

    #[test]
    fn log_md_valid() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "log.md",
            "# Log\n\n## 2026-05-28\n\n* **Update**: something changed.\n",
        );
        let report = validate_bundle(dir.path()).unwrap();
        assert_eq!(report.error_count, 0);
        assert_eq!(report.warning_count, 0);
    }

    #[test]
    fn log_md_bad_date_format() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "log.md",
            "# Log\n\n## May 28 2026\n\n* **Update**: something.\n",
        );
        let report = validate_bundle(dir.path()).unwrap();
        assert_eq!(report.error_count, 1);
        assert!(report.errors[0].message.contains("ISO 8601"));
    }

    #[test]
    fn log_md_with_frontmatter_warns() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "log.md",
            "---\ntitle: log\n---\n## 2026-05-28\n\n* entry\n",
        );
        let report = validate_bundle(dir.path()).unwrap();
        assert_eq!(report.warning_count, 1);
        assert!(report.warnings[0].message.contains("frontmatter"));
    }

    // --- mixed bundle --------------------------------------------------------

    #[test]
    fn full_bundle_valid() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "index.md",
            "---\nokf_version: \"0.1\"\n---\n# My Bundle\n",
        );
        write(dir.path(), "log.md", "# Log\n\n## 2026-01-01\n\n* **Init**: bundle created.\n");
        let tables = dir.path().join("tables");
        fs::create_dir_all(&tables).unwrap();
        write(&tables, "index.md", "# Tables\n\n* [Orders](orders.md)\n");
        write(
            &tables,
            "orders.md",
            "---\ntype: BigQuery Table\ntitle: Orders\ntags: [sales]\n---\n# Orders\n",
        );
        let report = validate_bundle(dir.path()).unwrap();
        assert_eq!(report.files_checked, 4);
        assert_eq!(report.error_count, 0);
        assert_eq!(report.warning_count, 0);
    }
}
