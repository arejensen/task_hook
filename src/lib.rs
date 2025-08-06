use regex::Regex;
use std::env;
use std::error::Error;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{self, Command};
use std::sync::OnceLock;
use std::{fs::File, io::Read};

// Default regex pattern for task/pbi/bug branches
const WORK_ITEM_REGEX_PATTERN: &str = r#"^(?:task|pbi|bug|feature|feat)/([0-9]+).*$"#;

// Static regex compiled once for performance
static WORK_ITEM_REGEX: OnceLock<Regex> = OnceLock::new();

fn get_work_item_regex() -> &'static Regex {
    WORK_ITEM_REGEX.get_or_init(|| {
        Regex::new(WORK_ITEM_REGEX_PATTERN).expect("Work item regex pattern should be valid")
    })
}

/// Get the current git branch name using git command
/// Uses 'git rev-parse --abbrev-ref HEAD' which is reliable and widely supported
pub fn get_current_branch() -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(&["rev-parse", "--abbrev-ref", "HEAD"])
        .output()?;

    if output.status.success() {
        let branch = String::from_utf8(output.stdout)?.trim().to_string();
        // Handle special case where detached HEAD returns "HEAD"
        if branch == "HEAD" {
            // Get short commit hash for detached HEAD
            let hash_output = Command::new("git")
                .args(&["rev-parse", "--short", "HEAD"])
                .output()?;
            if hash_output.status.success() {
                let hash = String::from_utf8(hash_output.stdout)?.trim().to_string();
                return Ok(format!("HEAD-{}", hash));
            }
        }
        Ok(branch)
    } else {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        Err(format!("Failed to get current branch: {}", error_msg).into())
    }
}

/// Process a commit by appending the task number to the commit message
pub fn process_commit(branch_name: &str, commit_filename: &str) -> Result<(), std::io::Error> {
    // Read the current commit message
    let mut current_message = String::new();
    {
        let mut read_file = File::open(commit_filename)?;
        read_file.read_to_string(&mut current_message)?;
    }

    // Generate the task number string
    let task_number_string = create_task_number_string(branch_name);

    // Write back the modified message
    let mut write_file = File::create(commit_filename)?;
    write!(write_file, "{}", current_message)?;
    write!(write_file, "{}", task_number_string)?;

    Ok(())
}

fn create_task_number_string(branch_name: &str) -> String {
    let regex = get_work_item_regex();
    if let Some(captures) = regex.captures(branch_name) {
        if let Some(task_number) = captures.get(1) {
            return format!("#{}", task_number.as_str());
        }
    }
    String::new()
}

// Look for the .git/hooks/prepare-commit-msg hook and call it if found
pub fn delegate_to_local_git_hook() -> Result<(), Box<dyn Error>> {
    let git_dir_output = Command::new("git")
        .args(&["rev-parse", "--git-dir"])
        .output()?
        .stdout;
    let git_dir = String::from_utf8(git_dir_output)?.trim().to_string();
    let repo_hook_path = format!("{}/hooks/prepare-commit-msg", git_dir);
    let repo_hook = Path::new(&repo_hook_path);
    Ok(if repo_hook.exists() {
        // Check it is executable
        let mode = fs::metadata(&repo_hook)?.permissions().mode();
        if mode & 0o111 != 0 {
            let args: Vec<String> = env::args().skip(1).collect();
            let status = Command::new(&repo_hook_path).args(&args).status()?;

            if !status.success() {
                process::exit(status.code().unwrap_or(1));
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use tempfile::NamedTempFile;

    /// Helper struct for managing test git repositories
    struct TestGitRepo {
        temp_dir: tempfile::TempDir,
    }

    impl TestGitRepo {
        /// Create a new test git repository with initial setup
        fn new() -> Result<Self, Box<dyn std::error::Error>> {
            let temp_dir = tempfile::tempdir()?;
            let repo = TestGitRepo { temp_dir };
            repo.init_git_repo()?;
            repo.configure_git()?;
            repo.create_initial_commit()?;
            Ok(repo)
        }

        /// Get the path to the repository
        fn path(&self) -> &Path {
            self.temp_dir.path()
        }

        /// Initialize git repository
        fn init_git_repo(&self) -> Result<(), Box<dyn std::error::Error>> {
            let init_result = Command::new("git")
                .args(&["init", "--initial-branch=main"])
                .current_dir(self.path())
                .env("GIT_CONFIG_GLOBAL", "/dev/null") // disable config as it can interfere with tests
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
                .output()?;

            if !init_result.status.success() {
                // Try without initial-branch flag for older git versions
                let init_result2 = Command::new("git")
                    .args(&["init"])
                    .current_dir(self.path())
                    .env("GIT_CONFIG_GLOBAL", "/dev/null")
                    .env("GIT_CONFIG_SYSTEM", "/dev/null")
                    .output()?;
                if !init_result2.status.success() {
                    let stderr = String::from_utf8_lossy(&init_result2.stderr);
                    return Err(format!("Failed to initialize git repository: {}", stderr).into());
                }
            }
            Ok(())
        }

        /// Configure git user and settings
        fn configure_git(&self) -> Result<(), Box<dyn std::error::Error>> {
            self.run_git_command(&["config", "user.name", "Test User"])?;
            self.run_git_command(&["config", "user.email", "test@example.com"])?;
            self.run_git_command(&["config", "core.hooksPath", "/dev/null"])?;
            Ok(())
        }

        /// Create initial commit
        fn create_initial_commit(&self) -> Result<(), Box<dyn std::error::Error>> {
            std::fs::write(self.path().join("test.txt"), "initial content")?;
            self.run_git_command(&["add", "test.txt"])?;
            self.run_git_command(&["commit", "-m", "Initial commit"])?;
            Ok(())
        }

        /// Run a git command and return the output
        fn run_git_command(
            &self,
            args: &[&str],
        ) -> Result<std::process::Output, Box<dyn std::error::Error>> {
            let output = Command::new("git")
                .args(args)
                .current_dir(self.path())
                .output()?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!(
                    "Git command failed: git {}\nError: {}",
                    args.join(" "),
                    stderr
                )
                .into());
            }
            Ok(output)
        }

        /// Get the current branch name
        fn get_current_branch(&self) -> Result<String, Box<dyn std::error::Error>> {
            let output = self.run_git_command(&["rev-parse", "--abbrev-ref", "HEAD"])?;
            Ok(String::from_utf8(output.stdout)?.trim().to_string())
        }

        /// Create and switch to a new branch
        fn create_and_switch_branch(
            &self,
            branch_name: &str,
        ) -> Result<(), Box<dyn std::error::Error>> {
            self.run_git_command(&["checkout", "-b", branch_name])?;
            Ok(())
        }

        /// Switch to an existing branch
        fn switch_branch(&self, branch_name: &str) -> Result<(), Box<dyn std::error::Error>> {
            self.run_git_command(&["checkout", branch_name])?;
            Ok(())
        }

        /// Get the current commit hash
        fn get_commit_hash(&self) -> Result<String, Box<dyn std::error::Error>> {
            let output = self.run_git_command(&["rev-parse", "HEAD"])?;
            Ok(String::from_utf8(output.stdout)?.trim().to_string())
        }

        /// Create detached HEAD by checking out a commit hash
        fn create_detached_head(&self) -> Result<String, Box<dyn std::error::Error>> {
            let commit_hash = self.get_commit_hash()?;
            self.run_git_command(&["checkout", &commit_hash])?;
            Ok(commit_hash)
        }

        /// Get detached HEAD representation
        fn get_detached_head_info(&self) -> Result<String, Box<dyn std::error::Error>> {
            let branch_output = self.run_git_command(&["rev-parse", "--abbrev-ref", "HEAD"])?;
            let branch = String::from_utf8(branch_output.stdout)?.trim().to_string();

            if branch == "HEAD" {
                let hash_output = self.run_git_command(&["rev-parse", "--short", "HEAD"])?;
                let hash = String::from_utf8(hash_output.stdout)?.trim().to_string();
                Ok(format!("HEAD-{}", hash))
            } else {
                Err(format!("Not in detached HEAD state, current branch: {}", branch).into())
            }
        }
    }

    #[test]
    fn test_create_task_number_string_comprehensive() {
        let test_cases = vec![
            // Task branches
            ("task/123-some-feature", "#123"),
            ("task/456", "#456"),
            ("task/0001-feature", "#0001"),
            // PBI branches
            ("pbi/456-user-story", "#456"),
            ("pbi/789", "#789"),
            // Bug branches
            ("bug/789-fix-issue", "#789"),
            ("bug/123-crash-fix", "#123"),
            // Feature branches
            ("feature/012-cool-feature", "#012"),
            ("feat/012-cool-feature", "#012"),
            // Non-matching branches
            ("feature/some-feature", ""),
            ("main", ""),
            ("develop", ""),
            // Invalid formats
            ("task/feature-name", ""),
            ("task/abc", ""),
            ("pbi/xyz", ""),
            ("bug/abc", ""),
            ("task/", ""),
            ("pbi/", ""),
            ("bug/", ""),
        ];

        for (branch_name, expected) in test_cases {
            let result = create_task_number_string(branch_name);
            assert_eq!(result, expected, "Failed for branch: {}", branch_name);
        }
    }

    #[test]
    fn test_commit_scenarios() -> Result<(), Box<dyn std::error::Error>> {
        let test_cases = vec![
            // Should change commit messages
            (
                "task/123-feature",
                "Original commit message\n",
                "Original commit message\n#123",
            ),
            ("pbi/456-story", "Fix the bug\n", "Fix the bug\n#456"),
            (
                "bug/789-crash-fix",
                "Fix critical bug\n",
                "Fix critical bug\n#789",
            ),
            (
                "feature/012-cool-feature",
                "Implement cool feature\n",
                "Implement cool feature\n#012",
            ),
            (
                "feat/012-cool-feature",
                "Implement cool feature\n",
                "Implement cool feature\n#012",
            ),
            // Should not not change commit messages
            (
                "main",
                "Original commit message\n",
                "Original commit message\n",
            ),
            (
                "feature/cool-stuff",
                "Add new feature\n",
                "Add new feature\n",
            ),
        ];

        for (branch_name, original_message, expected_result) in test_cases {
            let mut temp_file = NamedTempFile::new()?;
            write!(temp_file, "{}", original_message)?;

            let temp_path = temp_file.path().to_string_lossy();
            process_commit(branch_name, &temp_path)?;

            let result = fs::read_to_string(temp_file.path())?;
            assert_eq!(
                result, expected_result,
                "Failed for branch: {}",
                branch_name
            );
        }
        Ok(())
    }

    /// Test git command functionality by creating a temporary git repository
    #[test]
    fn test_get_current_branch_with_git() -> Result<(), Box<dyn std::error::Error>> {
        let repo = TestGitRepo::new()?;

        let branch = repo.get_current_branch()?;
        println!("Current branch detected: {}", branch);

        assert!(!branch.is_empty(), "Branch name should not be empty");
        assert!(
            !branch.contains('\n'),
            "Branch name should not contain newlines"
        );
        assert!(
            branch == "main" || branch == "master",
            "Expected main or master, got: {}",
            branch
        );

        Ok(())
    }

    /// Test detached HEAD scenario by creating a temporary git repository
    #[test]
    fn test_detached_head_scenario() -> Result<(), Box<dyn std::error::Error>> {
        let repo = TestGitRepo::new()?;

        repo.create_detached_head()?;

        let detached_info = repo.get_detached_head_info()?;
        assert!(
            detached_info.starts_with("HEAD-"),
            "Detached HEAD should start with 'HEAD-', got: {}",
            detached_info
        );
        println!("Successfully tested detached HEAD: {}", detached_info);

        Ok(())
    }

    /// Test error handling when git commands fail
    #[test]
    fn test_git_error_handling() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let non_git_path = temp_dir.path();

        // Test git command in directory that's not a git repo
        let result = Command::new("git")
            .args(&["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(non_git_path)
            .output()?;

        // Should return an error
        assert!(
            !result.status.success(),
            "Should return error when not in git repository"
        );

        let stderr = String::from_utf8_lossy(&result.stderr);
        println!("Expected error when not in git repo: {}", stderr);
        // Error message should mention git failure
        assert!(
            stderr.contains("not a git repository") || stderr.contains("fatal:"),
            "Error message should mention git failure, got: {}",
            stderr
        );

        Ok(())
    }

    /// Test branch creation and switching in a temporary repository
    #[test]
    fn test_branch_switching() -> Result<(), Box<dyn std::error::Error>> {
        let repo = TestGitRepo::new()?;

        let initial_branch = repo.get_current_branch()?;
        assert!(initial_branch == "main" || initial_branch == "master");

        let task_branch = "task/123-test-feature";
        repo.create_and_switch_branch(task_branch)?;

        let current_branch = repo.get_current_branch()?;
        assert_eq!(current_branch, task_branch, "Should detect task branch");

        let task_number = create_task_number_string(&current_branch);
        assert_eq!(
            task_number, "#123",
            "Should extract task number from branch"
        );

        repo.switch_branch(&initial_branch)?;

        // Verify we're back on the initial branch
        let final_branch = repo.get_current_branch()?;
        assert_eq!(
            final_branch, initial_branch,
            "Should be back on initial branch"
        );

        Ok(())
    }
}
