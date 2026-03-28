use std::collections::{HashSet, VecDeque};

use crate::ops::symbol_ops;
use crate::server::state::Project;
use crate::symbols::symbol::{Symbol, SymbolKind};

use super::{
    CallerWithRisk, CoveredSymbol, ImpactResult, ReviewDiff, RiskLevel, SafetyIssue, SafetyReport,
    SymbolChange, TestCoverageReport, TestWithStatus, UncoveredSymbol,
};

/// Maximum callers expanded per symbol per BFS level.
const MAX_CALLERS_PER_LEVEL: usize = 20;
/// Hard cap on total callers returned across all depths.
const MAX_TOTAL_CALLERS: usize = 500;
/// Maximum allowed depth (server-side guard).
const DEPTH_LIMIT: usize = 3;

/// Build the set of file paths that are modified in the PR.
fn modified_files(review_diff: &ReviewDiff) -> HashSet<String> {
    review_diff
        .file_diffs
        .iter()
        .map(|f| f.path.clone())
        .collect()
}

/// Find the innermost function or method symbol that contains `line` in `file`.
fn enclosing_function(project: &Project, file: &str, line: usize) -> Option<Symbol> {
    project
        .symbol_table
        .list_by_file(file)
        .into_iter()
        .filter(|s| {
            matches!(s.kind, SymbolKind::Function | SymbolKind::Method)
                && s.line_range.0 <= line
                && line <= s.line_range.1
        })
        // Innermost = smallest line range
        .min_by_key(|s| s.line_range.1.saturating_sub(s.line_range.0))
}

/// BFS expansion of callers up to `max_depth`.
///
/// Returns a flat vec of `CallerWithRisk` with `depth` and `via` fields
/// populated. Direct callers have `depth=1, via=None`; callers of callers
/// have `depth=2, via=Some(intermediate_function_name)`, and so on.
fn collect_callers_bfs(
    base_project: &Project,
    start_symbol: &str,
    start_file: &str,
    max_depth: usize,
    modified: &HashSet<String>,
) -> Vec<CallerWithRisk> {
    let max_depth = max_depth.clamp(1, DEPTH_LIMIT);

    // Queue: (sym_file, sym_name, caller_depth, via_for_callers)
    let mut queue: VecDeque<(String, String, usize, Option<String>)> = VecDeque::new();
    queue.push_back((start_file.to_string(), start_symbol.to_string(), 1, None));

    let mut visited: HashSet<(String, String)> = HashSet::new();
    visited.insert((start_file.to_string(), start_symbol.to_string()));

    let mut all_callers: Vec<CallerWithRisk> = Vec::new();

    while let Some((sym_file, sym_name, caller_depth, via)) = queue.pop_front() {
        if all_callers.len() >= MAX_TOTAL_CALLERS {
            break;
        }

        let callers = match symbol_ops::find_callers(
            &base_project.root,
            &base_project.file_tree,
            &base_project.symbol_table,
            &sym_name,
            &sym_file,
            MAX_CALLERS_PER_LEVEL,
        ) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for caller in callers {
            if all_callers.len() >= MAX_TOTAL_CALLERS {
                break;
            }

            all_callers.push(CallerWithRisk {
                file: caller.file.clone(),
                line: caller.line,
                text: caller.text.clone(),
                also_modified_in_pr: modified.contains(&caller.file),
                depth: caller_depth,
                via: via.clone(),
            });

            // Queue the enclosing function for the next depth level.
            if caller_depth < max_depth {
                if let Some(enc) = enclosing_function(base_project, &caller.file, caller.line) {
                    let enc_name = enc.name;
                    let enc_file = enc.file;
                    let key = (enc_file.clone(), enc_name.clone());
                    if !visited.contains(&key) {
                        visited.insert(key);
                        queue.push_back((
                            enc_file,
                            enc_name.clone(),
                            caller_depth + 1,
                            Some(enc_name),
                        ));
                    }
                }
            }
        }
    }

    all_callers
}

/// Find all deleted or signature-changed symbols that have callers in files
/// NOT modified by this PR — potential breakage.
///
/// `depth` controls how many hops of transitive callers to include (1 = direct
/// only, up to 3). When `depth > 1`, the result may include indirect callers
/// (callers of callers) with `depth` and `via` fields set on each entry.
pub fn compute_safety(
    base_project: &Project,
    review_diff: &ReviewDiff,
    depth: usize,
) -> SafetyReport {
    let modified = modified_files(review_diff);
    let mut issues: Vec<SafetyIssue> = Vec::new();
    let mut safe_count = 0usize;

    for sym_diff in &review_diff.symbol_diffs {
        let is_risky = matches!(
            &sym_diff.change,
            SymbolChange::Deleted
                | SymbolChange::Modified { signature_changed: true, .. }
                | SymbolChange::Moved { signature_changed: true, .. }
        );
        if !is_risky {
            continue;
        }

        let callers =
            collect_callers_bfs(base_project, &sym_diff.name, &sym_diff.file, depth, &modified);

        let unmodified: Vec<CallerWithRisk> =
            callers.into_iter().filter(|c| !c.also_modified_in_pr).collect();

        if unmodified.is_empty() {
            safe_count += 1;
        } else {
            let risk = match &sym_diff.change {
                SymbolChange::Deleted
                | SymbolChange::Modified { signature_changed: true, .. }
                | SymbolChange::Moved { signature_changed: true, .. } => RiskLevel::High,
                SymbolChange::Modified { body_changed: true, .. } => RiskLevel::Medium,
                _ => RiskLevel::Low,
            };
            issues.push(SafetyIssue {
                symbol: sym_diff.name.clone(),
                file: sym_diff.file.clone(),
                change: sym_diff.change.clone(),
                unmodified_callers: unmodified,
                risk,
            });
        }
    }

    SafetyReport { issues, safe_changes_count: safe_count }
}

/// Find symbols whose body changed (but signature did NOT) and that have
/// callers in files NOT modified by this PR.
///
/// These are silent behavioral changes — the code compiles but callers may
/// observe different behavior at runtime. Risk level is `Medium`.
pub fn compute_body_safety(
    base_project: &Project,
    review_diff: &ReviewDiff,
    depth: usize,
) -> SafetyReport {
    let modified = modified_files(review_diff);
    let mut issues: Vec<SafetyIssue> = Vec::new();
    let mut safe_count = 0usize;

    for sym_diff in &review_diff.symbol_diffs {
        let is_body_only_change = matches!(
            &sym_diff.change,
            SymbolChange::Modified { signature_changed: false, body_changed: true, .. }
                | SymbolChange::Moved { signature_changed: false, body_changed: true, .. }
        );
        if !is_body_only_change {
            continue;
        }

        let callers =
            collect_callers_bfs(base_project, &sym_diff.name, &sym_diff.file, depth, &modified);

        let unmodified: Vec<CallerWithRisk> =
            callers.into_iter().filter(|c| !c.also_modified_in_pr).collect();

        if unmodified.is_empty() {
            safe_count += 1;
        } else {
            issues.push(SafetyIssue {
                symbol: sym_diff.name.clone(),
                file: sym_diff.file.clone(),
                change: sym_diff.change.clone(),
                unmodified_callers: unmodified,
                risk: RiskLevel::Medium,
            });
        }
    }

    SafetyReport { issues, safe_changes_count: safe_count }
}

/// Compute the blast radius for a specific changed symbol: all callers in the
/// base branch, annotated with whether each also appears in modified PR files.
///
/// `depth` controls transitive expansion (1 = direct only, up to 3).
pub fn compute_impact(
    base_project: &Project,
    head_project: &Project,
    review_diff: &ReviewDiff,
    symbol_name: &str,
    file: &str,
    depth: usize,
) -> Option<ImpactResult> {
    let modified = modified_files(review_diff);

    let sym_diff = review_diff
        .symbol_diffs
        .iter()
        .find(|s| s.name == symbol_name && s.file == file)?;

    let base_callers =
        collect_callers_bfs(base_project, symbol_name, file, depth, &modified);

    let unmodified_count = base_callers.iter().filter(|c| !c.also_modified_in_pr).count();

    let tests: Vec<TestWithStatus> =
        match symbol_ops::find_tests(
            &head_project.root,
            &head_project.file_tree,
            &head_project.symbol_table,
            symbol_name,
            file,
            50,
        ) {
            Ok(tests) => tests
                .iter()
                .map(|t| TestWithStatus {
                    name: t.name.clone(),
                    file: t.file.clone(),
                    line: t.line,
                    also_modified_in_pr: modified.contains(&t.file),
                })
                .collect(),
            Err(_) => vec![],
        };

    let risk = if unmodified_count > 0 {
        match &sym_diff.change {
            SymbolChange::Deleted
            | SymbolChange::Modified { signature_changed: true, .. }
            | SymbolChange::Moved { signature_changed: true, .. } => RiskLevel::High,
            SymbolChange::Modified { body_changed: true, .. }
            | SymbolChange::Moved { body_changed: true, .. } => RiskLevel::Medium,
            _ => RiskLevel::Low,
        }
    } else {
        RiskLevel::Low
    };

    Some(ImpactResult {
        symbol: sym_diff.name.clone(),
        file: sym_diff.file.clone(),
        change: sym_diff.change.clone(),
        base_callers,
        unmodified_callers_count: unmodified_count,
        tests,
        risk,
    })
}

/// For each added or modified symbol in the PR, determine whether tests exist
/// for it in the head project.
pub fn compute_test_coverage(
    head_project: &Project,
    review_diff: &ReviewDiff,
) -> TestCoverageReport {
    let mut covered: Vec<CoveredSymbol> = Vec::new();
    let mut uncovered: Vec<UncoveredSymbol> = Vec::new();

    for sym_diff in &review_diff.symbol_diffs {
        // Imports don't have meaningful test coverage — skip them.
        if sym_diff.kind == SymbolKind::Import {
            continue;
        }

        let change_label = match &sym_diff.change {
            SymbolChange::Added => "added",
            SymbolChange::Modified { .. } => "modified",
            _ => continue,
        };

        let tests = match symbol_ops::find_tests(
            &head_project.root,
            &head_project.file_tree,
            &head_project.symbol_table,
            &sym_diff.name,
            &sym_diff.file,
            20,
        ) {
            Ok(t) => t,
            Err(_) => vec![],
        };

        // Deduplicate by test name before counting.
        let mut seen = std::collections::HashSet::new();
        let unique_count = tests.into_iter().filter(|t| seen.insert(t.name.clone())).count();

        if unique_count == 0 {
            uncovered.push(UncoveredSymbol {
                symbol: sym_diff.name.clone(),
                file: sym_diff.file.clone(),
                kind: sym_diff.kind,
                change: change_label.to_string(),
            });
        } else {
            covered.push(CoveredSymbol {
                symbol: sym_diff.name.clone(),
                file: sym_diff.file.clone(),
                kind: sym_diff.kind,
                test_count: unique_count,
            });
        }
    }

    let total = covered.len() + uncovered.len();
    let coverage_ratio = format!("{}/{}", covered.len(), total);

    TestCoverageReport { covered, uncovered, coverage_ratio }
}
