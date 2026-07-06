use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::process::{self, Command};

mod validator;

fn main() {
    // GitHub Actions sets INPUT_<UPPERCASED_NAME> for every declared input.
    let bundle_path_input = env::var("INPUT_BUNDLE_PATH").unwrap_or_else(|_| ".".to_string());
    let repo_input = env::var("INPUT_REPO").unwrap_or_default();

    // The workspace root inside the Docker container.
    let github_workspace = env::var("GITHUB_WORKSPACE").unwrap_or_else(|_| ".".to_string());
    let github_output = env::var("GITHUB_OUTPUT").unwrap_or_default();

    let validate_path: PathBuf = if !repo_input.trim().is_empty() {
        // Clone the external repository at shallow depth, then validate.
        let clone_dir = "/tmp/okf-repo-clone";
        println!("Cloning repository: {}", repo_input);
        let status = Command::new("git")
            .args(["clone", "--depth=1", repo_input.trim(), clone_dir])
            .status();
        match status {
            Ok(s) if s.success() => {}
            Ok(_) => {
                eprintln!("::error::Failed to clone repository: {}", repo_input);
                process::exit(1);
            }
            Err(e) => {
                eprintln!("::error::Could not run git: {}", e);
                process::exit(1);
            }
        }
        let base = PathBuf::from(clone_dir);
        if bundle_path_input.trim() == "." || bundle_path_input.trim().is_empty() {
            base
        } else {
            base.join(bundle_path_input.trim())
        }
    } else {
        // Validate a path inside the current workspace.
        let workspace = PathBuf::from(&github_workspace);
        if bundle_path_input.trim() == "." || bundle_path_input.trim().is_empty() {
            workspace
        } else {
            workspace.join(bundle_path_input.trim())
        }
    };

    println!("Validating OKF v0.1 bundle at: {}", validate_path.display());

    let report = match validator::validate_bundle(&validate_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("::error::{}", e);
            process::exit(1);
        }
    };

    // Print a human-readable summary.
    println!("=== OKF v0.1 Validation Report ===");
    println!("Files checked : {}", report.files_checked);
    println!("Errors        : {}", report.error_count);
    println!("Warnings      : {}", report.warning_count);

    // Emit GitHub Actions workflow annotations.
    for issue in &report.errors {
        println!("::error file={}::{}", issue.file, issue.message);
    }
    for issue in &report.warnings {
        println!("::warning file={}::{}", issue.file, issue.message);
    }

    // Write step outputs to $GITHUB_OUTPUT (Actions v2+ protocol).
    if !github_output.is_empty() {
        if let Ok(mut f) = OpenOptions::new().append(true).open(&github_output) {
            let _ = writeln!(f, "files_checked={}", report.files_checked);
            let _ = writeln!(f, "error_count={}", report.error_count);
        }
    }

    if report.error_count > 0 {
        eprintln!(
            "Bundle validation FAILED with {} error(s).",
            report.error_count
        );
        process::exit(1);
    } else {
        println!("✓ Bundle is conformant with OKF v0.1");
    }
}
