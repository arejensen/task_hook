extern crate task_hook;
use task_hook::*;

use std::env;
use std::error;
use std::process;

fn main() -> Result<(), Box<dyn error::Error>> {
    let commit_filename = env::args().nth(1);
    let commit_source = env::args().nth(2);
    let current_branch = get_current_branch();

    match (
        current_branch,
        commit_filename.clone(),
        commit_source.clone(),
    ) {
        (Ok(branch), Some(filename), None) => {
            if let Err(e) = process_commit(&branch, &filename) {
                eprintln!("Failed to add task number to message: {}", e);
                process::exit(2);
            }
            if let Err(e) = delegate_to_local_git_hook() {
                eprintln!("Failed to run local git hook: {}", e);
                process::exit(3);
            }
        }
        (Ok(branch), Some(filename), Some(source)) => {
            if source == "message" {
                if let Err(e) = process_commit(&branch, &filename) {
                    eprintln!("Failed to append task number to commit message: {}", e);
                    process::exit(2);
                }
                if let Err(e) = delegate_to_local_git_hook() {
                    eprintln!("Failed to run local git hook: {}", e);
                    process::exit(3);
                }
            }
        }
        (Err(e), _, _) => {
            eprintln!("Failed to find current branch: {}", e);
            process::exit(1);
        }
        (_, None, _) => {
            eprintln!("Commit file was not provided or could not be found or read");
            process::exit(2);
        }
    }

    Ok(())
}
