use super::*;

#[test]
fn empty_request_uses_the_global_folder() {
    assert_eq!(validate_working_dir(None).unwrap(), None);
    assert_eq!(validate_working_dir(Some("   ")).unwrap(), None);
}

#[test]
fn existing_absolute_directory_is_canonicalized() {
    let temp = tempfile::tempdir().unwrap();
    let nested = temp.path().join("project");
    std::fs::create_dir(&nested).unwrap();
    let raw = format!("{}/./", nested.display());
    let validated = validate_working_dir(Some(&raw)).unwrap().unwrap();
    assert_eq!(PathBuf::from(validated), nested.canonicalize().unwrap());
}

#[test]
fn relative_missing_file_and_forbidden_paths_are_rejected() {
    assert!(validate_working_dir(Some("relative/dir")).is_err());

    let temp = tempfile::tempdir().unwrap();
    assert!(
        validate_working_dir(Some(&temp.path().join("missing").display().to_string())).is_err()
    );

    let file = temp.path().join("file.txt");
    std::fs::write(&file, "x").unwrap();
    assert!(validate_working_dir(Some(&file.display().to_string())).is_err());

    let ssh = temp.path().join(".ssh");
    std::fs::create_dir(&ssh).unwrap();
    assert!(validate_working_dir(Some(&ssh.display().to_string())).is_err());
}
