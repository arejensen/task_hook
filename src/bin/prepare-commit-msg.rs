extern crate task_hook;
use task_hook::*;

use std::error;
use std::process;
use std::env;

use regex::Regex;

fn main() -> Result<(), Box<dyn error::Error>> {
    let commit_filename = env::args().nth(1);
    let commit_source = env::args().nth(2);
    
    let current_branch = get_current_branch();

    let task_number_regex = Regex::new(r#"^task/([0-9]+).*$"#)?;

    match (current_branch, commit_filename, commit_source) {
        (Ok(branch), Some(filename), None) => {
            let write_result = ordinary_commit(branch, filename, task_number_regex);
            match write_result {
                Ok(_) => {},
                Err(e) => {
                    eprintln!("Failed to add task number to message. {}", e);
                    process::exit(2);
                }
            };
        },
        (Ok(branch), Some(filename), Some(commit_source)) => {
            // We only care about message commits (e.g., git commit -m "message")
            // not amends, merges, etc. 
            if commit_source == "message" {
                let write_result = message_commit(branch, filename, task_number_regex);
                match write_result {
                    Ok(_) => {},
                    Err(e) => {
                        eprintln!("Failed to append task number to commit message. {}", e);
                        process::exit(2);
                    }
                };
            } 
        }
        (Err(e), _, _) => {
            eprintln!("Failed to find current branch. {}", e);
            process::exit(1);
        },
        (_, None, _) => {
            eprintln!("Commit file was not provided or could not be found or read");
            process::exit(2);
        }
    }

    Ok(())
}