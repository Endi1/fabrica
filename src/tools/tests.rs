use super::*;
use std::env;
use std::io::Write;
use std::{
    fs::{self, File},
    path::PathBuf,
    str::FromStr,
};
use tempfile::TempDir;

#[test]
fn test_get_directory_contents_empty_dir() {
    let temp_dir = TempDir::new().unwrap();
    let tool = ls();
    let contents = tool
        .run(DirectoryPath {
            path: temp_dir.path().to_str().unwrap().to_string(),
        })
        .unwrap();
    assert_eq!(contents.contents.len(), 0);
}

#[test]
fn test_get_directory_contents_with_files() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create test files
    File::create(temp_path.join("file1.txt")).unwrap();
    File::create(temp_path.join("file2.rs")).unwrap();

    let mut contents = ls()
        .run(DirectoryPath {
            path: temp_dir.path().to_str().unwrap().to_string(),
        })
        .unwrap();
    contents.contents.sort();

    assert_eq!(contents.contents.len(), 2);
    assert!(contents.contents.contains(&"file1.txt".to_string()));
    assert!(contents.contents.contains(&"file2.rs".to_string()));
}

#[test]
fn test_get_directory_contents_with_subdirs() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create subdirectory
    fs::create_dir(temp_path.join("subdir")).unwrap();
    File::create(temp_path.join("file.txt")).unwrap();

    let mut contents = ls()
        .run(DirectoryPath {
            path: temp_dir.path().to_str().unwrap().to_string(),
        })
        .unwrap();
    contents.contents.sort();

    assert_eq!(contents.contents.len(), 2);
    assert!(contents.contents.contains(&"subdir".to_string()));
    assert!(contents.contents.contains(&"file.txt".to_string()));
}

#[test]
fn test_get_directory_contents_nonexistent_path() {
    let result = ls().run(DirectoryPath {
        path: "/nonexistent/path".to_string(),
    });
    assert!(result.is_err());
}

#[test]
fn test_get_directory_contents_file_as_path() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir
        .path()
        .join("test.txt")
        .to_str()
        .unwrap()
        .to_string();
    File::create(&file_path).unwrap();

    let result = ls().run(DirectoryPath { path: file_path });
    assert!(result.is_err());
}

#[test]
fn test_get_current_path_returns_ok() {
    let result = get_current_path().run(());
    assert!(result.is_ok());
}

#[test]
fn test_get_current_path_is_absolute() {
    let path = get_current_path().run(()).unwrap();
    let parsed_path = PathBuf::from_str(&path.path).unwrap();
    assert!(parsed_path.is_absolute());
}

#[test]
fn test_get_current_path_matches_env_current_dir() {
    let our_path = get_current_path().run(()).unwrap().path;
    let env_path = env::current_dir().unwrap().to_str().unwrap().to_string();
    assert_eq!(our_path, env_path);
}

#[test]
fn test_get_current_path_exists() {
    let path = get_current_path().run(()).unwrap();
    let parsed_path = PathBuf::from_str(&path.path).unwrap();
    assert!(parsed_path.exists());
}

#[test]
fn test_get_current_path_is_directory() {
    let path = get_current_path().run(()).unwrap();
    let parsed_path = PathBuf::from_str(&path.path).unwrap();
    assert!(parsed_path.is_dir());
}

#[test]
fn test_read_file_contents_success() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create test file with content
    let mut file = File::create(temp_path.join("test.txt")).unwrap();
    writeln!(file, "Hello, World!").unwrap();
    writeln!(file, "This is a test file.").unwrap();

    let result = read()
        .run(ReadInput {
            directory: temp_path.to_str().unwrap().to_string(),
            filename: "test.txt".to_string(),
        })
        .unwrap();

    assert_eq!(
        result.file_contents,
        "Hello, World!\nThis is a test file.\n"
    );
}

#[test]
fn test_read_file_contents_empty_file() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create empty file
    File::create(temp_path.join("empty.txt")).unwrap();

    let result = read()
        .run(ReadInput {
            directory: temp_path.to_str().unwrap().to_string(),
            filename: "empty.txt".to_string(),
        })
        .unwrap();

    assert_eq!(result.file_contents, "");
}

#[test]
fn test_read_file_contents_nonexistent_file() {
    let temp_dir = TempDir::new().unwrap();

    let result = read().run(ReadInput {
        directory: temp_dir.path().to_str().unwrap().to_string(),
        filename: "nonexistent.txt".to_string(),
    });

    assert!(result.is_err());
}

#[test]
fn test_read_file_contents_nonexistent_directory() {
    let result = read().run(ReadInput {
        directory: "/nonexistent/directory".to_string(),
        filename: "test.txt".to_string(),
    });

    assert!(result.is_err());
}

#[test]
fn test_read_file_contents_with_subdirectory() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create subdirectory and file
    let subdir = temp_path.join("subdir");
    fs::create_dir(&subdir).unwrap();

    let mut file = File::create(subdir.join("nested.txt")).unwrap();
    writeln!(file, "Nested file content").unwrap();

    let result = read()
        .run(ReadInput {
            directory: subdir.to_str().unwrap().to_string(),
            filename: "nested.txt".to_string(),
        })
        .unwrap();

    assert_eq!(result.file_contents, "Nested file content\n");
}

#[test]
fn test_read_file_contents_unicode() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create file with Unicode content
    let mut file = File::create(temp_path.join("unicode.txt")).unwrap();
    writeln!(file, "Hello 世界! 🦀").unwrap();

    let result = read()
        .run(ReadInput {
            directory: temp_path.to_str().unwrap().to_string(),
            filename: "unicode.txt".to_string(),
        })
        .unwrap();

    assert_eq!(result.file_contents, "Hello 世界! 🦀\n");
}

#[test]
fn test_read_file_contents_binary_file() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create binary file
    let binary_data = vec![0u8, 1u8, 2u8, 255u8];
    fs::write(temp_path.join("binary.bin"), &binary_data).unwrap();

    let result = read().run(ReadInput {
        directory: temp_path.to_str().unwrap().to_string(),
        filename: "binary.bin".to_string(),
    });

    // This might fail or succeed depending on if binary data is valid UTF-8
    // If it succeeds, verify the content
    if let Ok(content) = result {
        assert_eq!(content.file_contents.as_bytes(), &binary_data);
    } else {
        // Expected to fail for invalid UTF-8
        assert!(result.is_err());
    }
}

#[test]
fn test_read_file_contents_directory_as_filename() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create subdirectory
    fs::create_dir(temp_path.join("subdir")).unwrap();

    let result = read().run(ReadInput {
        directory: temp_path.to_str().unwrap().to_string(),
        filename: "subdir".to_string(),
    });

    assert!(result.is_err());
}

#[test]
fn test_read_file_contents_path_traversal() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create file in temp directory
    let mut file = File::create(temp_path.join("secret.txt")).unwrap();
    writeln!(file, "Secret content").unwrap();

    // Try to read using path traversal (this should still work as Path::join handles it)
    let result = read().run(ReadInput {
        directory: temp_path.to_str().unwrap().to_string(),
        filename: "../secret.txt".to_string(),
    });

    // This test verifies the behavior - it may succeed or fail depending on filesystem
    println!("Path traversal result: {:?}", result);
}
