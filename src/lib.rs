use git2::Repository;
use regex::Regex;
use std::fs::File;
use std::io::Write;
use std::io::Read;

pub fn get_current_branch() -> Result<String, git2::Error> {
    let git_repo = Repository::discover("./")?;
    let head = git_repo.head()?;
    let head_name =  head.shorthand();
    match head_name {
        Some(name) => Ok(name.to_string()),
        None => Err(git2::Error::from_str("No branch name found"))
    }
}

pub fn ordinary_commit(branch_name: String, commit_filename: String, branch_name_regex: Regex) -> Result<(), std::io::Error> {
    let (current_message, mut commit_file) = prepare_output(commit_filename)?;
    let task_number_string = create_task_number_string(branch_name_regex, branch_name);

    write!(commit_file, "{}", task_number_string)?;
    write!(commit_file, "{}", current_message)?;

    Ok(())
}

pub fn message_commit(branch_name: String, commit_filename: String, branch_name_regex: Regex) -> Result<(), std::io::Error> {
    let (current_message, mut commit_file) = prepare_output(commit_filename)?;
    let task_number_string = create_task_number_string(branch_name_regex, branch_name);

    write!(commit_file, "{}", current_message)?;
    write!(commit_file, "{}", task_number_string)?;

    Ok(())
}

fn prepare_output(commit_filename: String) -> Result<(String, File), std::io::Error> {
    let mut read_commit_file = File::open(commit_filename.clone())?;
    let mut current_message = String::new();

    read_commit_file.read_to_string(&mut current_message)?;
    let commit_file = File::create(commit_filename)?;

    Ok((current_message, commit_file))
}

pub fn create_task_number_string(branch_name_regex: Regex, branch_name: String) -> String {
    let task_number = if let Some(task_number) = branch_name_regex.captures_iter(&branch_name).next() {
        let task_number = task_number.get(1);
        match task_number {
            Some(task_number) => format!("#{}", task_number.as_str()),
            None => "".to_owned(),
        }
    } else {
        "".to_owned()
    };
    task_number
}