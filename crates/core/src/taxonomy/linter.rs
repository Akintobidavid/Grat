use crate::error::{GratError, GratResult};
use crate::taxonomy::loader::TaxonomyParser;
use crate::taxonomy::schema::ErrorCategory;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// A single issue found by the linter.
#[derive(Debug, Clone)]
pub struct LintIssue {
    /// Source file where the issue was found.
    pub file: String,
    /// The `id` of the taxonomy entry the issue relates to, if applicable.
    pub entry_id: Option<String>,
    /// Human-readable description of the issue.
    pub message: String,
}

/// Lint all `*.toml` taxonomy files in `dir`.
///
/// Returns a list of [`LintIssue`]s. The function does **not** short-circuit on
/// the first error – it processes every file and collects all issues.
#[allow(clippy::too_many_lines)]
pub fn lint_dir(dir: &Path) -> GratResult<Vec<LintIssue>> {
    let mut issues: Vec<LintIssue> = Vec::new();

    // ------------------------------------------------------------------
    // 1. Gather all *.toml files in the directory
    // ------------------------------------------------------------------
    let mut toml_files: Vec<std::path::PathBuf> = Vec::new();
    let dir_reader = std::fs::read_dir(dir)
        .map_err(|e| GratError::TaxonomyError(format!("Cannot read taxonomy dir: {e}")))?;

    for entry in dir_reader {
        let entry = entry.map_err(|e| GratError::TaxonomyError(e.to_string()))?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "toml") {
            toml_files.push(path);
        }
    }

    // All successfully-parsed entries, annotated with their source file name.
    let mut all_entries: Vec<(String, crate::taxonomy::schema::TaxonomyEntry)> = Vec::new();

    // ------------------------------------------------------------------
    // 2. Parse every file and validate per-entry rules
    // ------------------------------------------------------------------
    for path in &toml_files {
        let file_name = path
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or("unknown")
            .to_string();

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                issues.push(LintIssue {
                    file: file_name,
                    entry_id: None,
                    message: format!("Cannot read file: {e}"),
                });
                continue;
            }
        };

        let schema = match TaxonomyParser::parse(&content) {
            Ok(s) => s,
            Err(e) => {
                issues.push(LintIssue {
                    file: file_name,
                    entry_id: None,
                    message: format!("TOML parse error: {e}"),
                });
                continue;
            }
        };

        for entry in schema.errors {
            let entry_id = entry.id.clone();

            // ── id must be non-empty ─────────────────────────────────
            if entry.id.trim().is_empty() {
                issues.push(LintIssue {
                    file: file_name.clone(),
                    entry_id: Some(entry_id.clone()),
                    message: "id is empty".to_string(),
                });
            }

            // ── name must be non-empty ───────────────────────────────
            if entry.name.trim().is_empty() {
                issues.push(LintIssue {
                    file: file_name.clone(),
                    entry_id: Some(entry_id.clone()),
                    message: "name is empty".to_string(),
                });
            }

            // ── summary must be non-empty ────────────────────────────
            if entry.summary.trim().is_empty() {
                issues.push(LintIssue {
                    file: file_name.clone(),
                    entry_id: Some(entry_id.clone()),
                    message: "summary is empty".to_string(),
                });
            }

            // ── detailed_explanation must be non-empty ───────────────
            if entry.detailed_explanation.trim().is_empty() {
                issues.push(LintIssue {
                    file: file_name.clone(),
                    entry_id: Some(entry_id.clone()),
                    message: "detailed_explanation is empty".to_string(),
                });
            }

            // ── severity must be a known value ───────────────────────
            // Checked against actual data files: Error, Warning, Info, Fatal
            const VALID_SEVERITIES: &[&str] = &["Error", "Warning", "Info", "Fatal"];
            if !VALID_SEVERITIES.contains(&entry.severity.as_str()) {
                issues.push(LintIssue {
                    file: file_name.clone(),
                    entry_id: Some(entry_id.clone()),
                    message: format!(
                        "invalid severity '{}': must be one of {}",
                        entry.severity,
                        VALID_SEVERITIES.join(", "),
                    ),
                });
            }

            // ── since_protocol > 0 when present ──────────────────────
            if let Some(sp) = entry.since_protocol {
                if sp == 0 {
                    issues.push(LintIssue {
                        file: file_name.clone(),
                        entry_id: Some(entry_id.clone()),
                        message: "since_protocol must be > 0".to_string(),
                    });
                }
            }

            // ── deprecated_protocol >= since_protocol ────────────────
            if let (Some(dp), Some(sp)) = (entry.deprecated_protocol, entry.since_protocol) {
                if dp < sp {
                    issues.push(LintIssue {
                        file: file_name.clone(),
                        entry_id: Some(entry_id.clone()),
                        message: format!(
                            "deprecated_protocol ({dp}) must be >= since_protocol ({sp})"
                        ),
                    });
                }
            }

            // ── documentation_url must parse as a URL when present ───
            // Structural validation only – no live HTTP requests.
            if let Some(ref doc_url) = entry.documentation_url {
                if url::Url::parse(doc_url).is_err() {
                    issues.push(LintIssue {
                        file: file_name.clone(),
                        entry_id: Some(entry_id.clone()),
                        message: format!("documentation_url '{doc_url}' is not a valid URL"),
                    });
                }
            }

            all_entries.push((file_name.clone(), entry));
        }
    }

    // ------------------------------------------------------------------
    // 3. Cross-entry checks
    // ------------------------------------------------------------------

    // ── Duplicate (category, code) pairs ──────────────────────────────
    let mut seen: HashMap<(ErrorCategory, u32), String> = HashMap::new();
    for (file_name, entry) in &all_entries {
        let key = (entry.category.clone(), entry.code);
        if let Some(prev_file) = seen.get(&key) {
            issues.push(LintIssue {
                file: file_name.clone(),
                entry_id: Some(entry.id.clone()),
                message: format!(
                    "duplicate (category, code) pair ({}, {}) already defined in {}",
                    entry.category, entry.code, prev_file,
                ),
            });
        } else {
            seen.insert(key, file_name.clone());
        }
    }

    // ── related_errors should reference existing ids ──────────────────
    let all_ids: HashSet<&str> = all_entries.iter().map(|(_, e)| e.id.as_str()).collect();
    for (file_name, entry) in &all_entries {
        for rel in &entry.related_errors {
            if !all_ids.contains(rel.as_str()) {
                issues.push(LintIssue {
                    file: file_name.clone(),
                    entry_id: Some(entry.id.clone()),
                    message: format!(
                        "related_errors references '{rel}' which does not exist in any loaded file",
                    ),
                });
            }
        }
    }

    Ok(issues)
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::taxonomy::schema::{CategoryMeta, ErrorCategory, TaxonomyEntry, TaxonomySchema};

    /// Helper: create a minimal valid entry for testing.
    fn valid_entry(id: &str, category: ErrorCategory, code: u32) -> TaxonomyEntry {
        TaxonomyEntry {
            id: id.to_string(),
            category,
            code,
            name: format!("TestName{code}"),
            severity: "Error".to_string(),
            since_protocol: Some(20),
            deprecated_protocol: None,
            summary: "Test summary.".to_string(),
            detailed_explanation: "Test detailed explanation.".to_string(),
            common_causes: vec![],
            suggested_fixes: vec![],
            related_errors: vec![],
            source_file: None,
            source_line: None,
            documentation_url: None,
        }
    }

    /// Helper: write a taxonomy file to a temp dir and return its path.
    fn write_taxonomy_file(
        dir: &std::path::Path,
        name: &str,
        entries: Vec<TaxonomyEntry>,
    ) -> std::path::PathBuf {
        let schema = TaxonomySchema {
            category: CategoryMeta {
                name: "Test".to_string(),
                description: "Test data".to_string(),
                source_module: "test".to_string(),
            },
            errors: entries,
        };
        let toml_str = toml::to_string(&schema).expect("serialize schema");
        let path = dir.join(name);
        std::fs::write(&path, toml_str).expect("write taxonomy file");
        path
    }

    #[test]
    fn valid_entry_passes() {
        let dir = tempfile::tempdir().expect("temp dir");
        write_taxonomy_file(
            dir.path(),
            "test.toml",
            vec![
                valid_entry("test.entry.1", ErrorCategory::Budget, 1),
                valid_entry("test.entry.2", ErrorCategory::Storage, 2),
            ],
        );

        let issues = lint_dir(dir.path()).expect("lint_dir");
        assert!(issues.is_empty(), "expected no issues, got: {issues:?}");
    }

    #[test]
    fn missing_required_field_fails() {
        let dir = tempfile::tempdir().expect("temp dir");

        // Manually craft via toml string so we can omit fields that serde
        // treats as required (but may still serialize as empty).
        let toml_str = r#"
[category]
name = "Test"
description = "Test data"
source_module = "test"

[[errors]]
id = ""
category = "budget"
code = 1
name = ""
severity = "Error"
summary = ""
detailed_explanation = ""
"#;
        std::fs::write(dir.path().join("test.toml"), toml_str).expect("write");

        let issues = lint_dir(dir.path()).expect("lint_dir");
        assert!(
            !issues.is_empty(),
            "expected issues for empty required fields"
        );
        // Expect at least id empty, name empty, summary empty, detailed_explanation empty
        let messages: Vec<&str> = issues.iter().map(|i| i.message.as_str()).collect();
        assert!(
            messages.contains(&"id is empty"),
            "missing 'id is empty': {messages:?}"
        );
        assert!(
            messages.contains(&"name is empty"),
            "missing 'name is empty': {messages:?}"
        );
        assert!(
            messages.contains(&"summary is empty"),
            "missing 'summary is empty': {messages:?}"
        );
        assert!(
            messages.contains(&"detailed_explanation is empty"),
            "missing 'detailed_explanation is empty': {messages:?}"
        );
    }

    #[test]
    fn duplicate_category_code_fails() {
        let dir = tempfile::tempdir().expect("temp dir");
        write_taxonomy_file(
            dir.path(),
            "test.toml",
            vec![
                valid_entry("test.dup.a", ErrorCategory::Budget, 1),
                valid_entry("test.dup.b", ErrorCategory::Budget, 1), // duplicate!
            ],
        );

        let issues = lint_dir(dir.path()).expect("lint_dir");
        let dup_issues: Vec<&LintIssue> = issues
            .iter()
            .filter(|i| i.message.contains("duplicate"))
            .collect();
        assert_eq!(
            dup_issues.len(),
            1,
            "expected 1 duplicate issue, got {dup_issues:?}",
        );
    }

    #[test]
    fn malformed_documentation_url_fails() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut entry = valid_entry("test.bad.url", ErrorCategory::Budget, 42);
        entry.documentation_url = Some("not a url".to_string());
        write_taxonomy_file(dir.path(), "test.toml", vec![entry]);

        let issues = lint_dir(dir.path()).expect("lint_dir");
        let url_issues: Vec<&LintIssue> = issues
            .iter()
            .filter(|i| i.message.contains("documentation_url"))
            .collect();
        assert_eq!(
            url_issues.len(),
            1,
            "expected 1 url issue, got {url_issues:?}",
        );
    }

    #[test]
    fn valid_documentation_url_passes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut entry = valid_entry("test.good.url", ErrorCategory::Budget, 43);
        entry.documentation_url = Some("https://example.com/docs".to_string());
        write_taxonomy_file(dir.path(), "test.toml", vec![entry]);

        let issues = lint_dir(dir.path()).expect("lint_dir");
        let url_issues: Vec<&LintIssue> = issues
            .iter()
            .filter(|i| i.message.contains("documentation_url"))
            .collect();
        assert!(
            url_issues.is_empty(),
            "expected no url issues, got {url_issues:?}",
        );
    }

    #[test]
    fn bad_severity_fails() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut entry = valid_entry("test.bad.severity", ErrorCategory::Budget, 44);
        entry.severity = "BadSeverity".to_string();
        write_taxonomy_file(dir.path(), "test.toml", vec![entry]);

        let issues = lint_dir(dir.path()).expect("lint_dir");
        let sev_issues: Vec<&LintIssue> = issues
            .iter()
            .filter(|i| i.message.contains("severity"))
            .collect();
        assert_eq!(
            sev_issues.len(),
            1,
            "expected 1 severity issue, got {sev_issues:?}",
        );
    }

    #[test]
    fn since_protocol_zero_fails() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut entry = valid_entry("test.bad.sp", ErrorCategory::Budget, 45);
        entry.since_protocol = Some(0);
        write_taxonomy_file(dir.path(), "test.toml", vec![entry]);

        let issues = lint_dir(dir.path()).expect("lint_dir");
        let sp_issues: Vec<&LintIssue> = issues
            .iter()
            .filter(|i| i.message.contains("since_protocol"))
            .collect();
        assert_eq!(
            sp_issues.len(),
            1,
            "expected 1 since_protocol issue, got {sp_issues:?}",
        );
    }

    #[test]
    fn deprecated_before_since_fails() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut entry = valid_entry("test.bad.depr", ErrorCategory::Budget, 46);
        entry.since_protocol = Some(20);
        entry.deprecated_protocol = Some(15); // < since_protocol
        write_taxonomy_file(dir.path(), "test.toml", vec![entry]);

        let issues = lint_dir(dir.path()).expect("lint_dir");
        let dep_issues: Vec<&LintIssue> = issues
            .iter()
            .filter(|i| i.message.contains("deprecated_protocol"))
            .collect();
        assert_eq!(
            dep_issues.len(),
            1,
            "expected 1 deprecated_protocol issue, got {dep_issues:?}",
        );
    }

    #[test]
    fn unresolved_related_error_fails() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut entry = valid_entry("test.rel", ErrorCategory::Budget, 47);
        entry.related_errors = vec!["nonexistent.id".to_string()];
        write_taxonomy_file(dir.path(), "test.toml", vec![entry]);

        let issues = lint_dir(dir.path()).expect("lint_dir");
        let rel_issues: Vec<&LintIssue> = issues
            .iter()
            .filter(|i| i.message.contains("related_errors"))
            .collect();
        assert_eq!(
            rel_issues.len(),
            1,
            "expected 1 related_errors issue, got {rel_issues:?}",
        );
    }
}
