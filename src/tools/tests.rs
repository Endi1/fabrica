use super::*;
use std::env;
use std::io::Write;
use std::{
    fs::{self, File},
    path::PathBuf,
    str::FromStr,
};
use tempfile::TempDir;

/// Helper to execute a tool with typed args and parse the result back.
fn run_tool<A: serde::Serialize, R: serde::de::DeserializeOwned>(
    tool: &ExecutableTool,
    args: A,
) -> Result<R, Box<dyn std::error::Error>> {
    let args_value = serde_json::to_value(args)?;
    let result_value = tool.execute(args_value)?;
    let result: R = serde_json::from_value(result_value)?;
    Ok(result)
}

#[test]
fn test_get_directory_contents_empty_dir() {
    let temp_dir = TempDir::new().unwrap();
    let tool = ls();
    let contents: DirectoryContentsResult = run_tool(
        &tool,
        DirectoryPath {
            path: temp_dir.path().to_str().unwrap().to_string(),
        },
    )
    .unwrap();
    assert_eq!(contents.contents.len(), 0);
}

#[test]
fn test_get_directory_contents_with_files() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    File::create(temp_path.join("file1.txt")).unwrap();
    File::create(temp_path.join("file2.rs")).unwrap();

    let mut contents: DirectoryContentsResult = run_tool(
        &ls(),
        DirectoryPath {
            path: temp_dir.path().to_str().unwrap().to_string(),
        },
    )
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

    fs::create_dir(temp_path.join("subdir")).unwrap();
    File::create(temp_path.join("file.txt")).unwrap();

    let mut contents: DirectoryContentsResult = run_tool(
        &ls(),
        DirectoryPath {
            path: temp_dir.path().to_str().unwrap().to_string(),
        },
    )
    .unwrap();
    contents.contents.sort();

    assert_eq!(contents.contents.len(), 2);
    assert!(contents.contents.contains(&"subdir".to_string()));
    assert!(contents.contents.contains(&"file.txt".to_string()));
}

#[test]
fn test_get_directory_contents_nonexistent_path() {
    let result: Result<DirectoryContentsResult, _> = run_tool(
        &ls(),
        DirectoryPath {
            path: "/nonexistent/path".to_string(),
        },
    );
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

    let result: Result<DirectoryContentsResult, _> =
        run_tool(&ls(), DirectoryPath { path: file_path });
    assert!(result.is_err());
}

#[test]
fn test_get_current_path_returns_ok() {
    let result: Result<CurrentPathResult, _> = run_tool(&get_current_path(), serde_json::json!({}));
    assert!(result.is_ok());
}

#[test]
fn test_get_current_path_is_absolute() {
    let path: CurrentPathResult = run_tool(&get_current_path(), serde_json::json!({})).unwrap();
    let parsed_path = PathBuf::from_str(&path.path).unwrap();
    assert!(parsed_path.is_absolute());
}

#[test]
fn test_get_current_path_matches_env_current_dir() {
    let result: CurrentPathResult = run_tool(&get_current_path(), serde_json::json!({})).unwrap();
    let env_path = env::current_dir().unwrap().to_str().unwrap().to_string();
    assert_eq!(result.path, env_path);
}

#[test]
fn test_get_current_path_exists() {
    let result: CurrentPathResult = run_tool(&get_current_path(), serde_json::json!({})).unwrap();
    let parsed_path = PathBuf::from_str(&result.path).unwrap();
    assert!(parsed_path.exists());
}

#[test]
fn test_get_current_path_is_directory() {
    let result: CurrentPathResult = run_tool(&get_current_path(), serde_json::json!({})).unwrap();
    let parsed_path = PathBuf::from_str(&result.path).unwrap();
    assert!(parsed_path.is_dir());
}

#[test]
fn test_read_file_contents_success() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    let mut file = File::create(temp_path.join("test.txt")).unwrap();
    writeln!(file, "Hello, World!").unwrap();
    writeln!(file, "This is a test file.").unwrap();

    let mut fp = temp_path.to_str().unwrap().to_string();
    fp.push_str("/test.txt");
    let result: ReadOutput = run_tool(
        &read(),
        ReadInput {
            filepath: fp,
            offset: None,
            limit: None,
        },
    )
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

    File::create(temp_path.join("empty.txt")).unwrap();

    let mut fp = temp_path.to_str().unwrap().to_string();
    fp.push_str("/empty.txt");
    let result: ReadOutput = run_tool(
        &read(),
        ReadInput {
            filepath: fp,
            offset: None,
            limit: None,
        },
    )
    .unwrap();

    assert_eq!(result.file_contents, "");
}

#[test]
fn test_read_file_contents_nonexistent_file() {
    let temp_dir = TempDir::new().unwrap();

    let mut fp = temp_dir.path().to_str().unwrap().to_string();
    fp.push_str("/nonexistent.txt");
    let result: Result<ReadOutput, _> = run_tool(
        &read(),
        ReadInput {
            filepath: fp,
            offset: None,
            limit: None,
        },
    );

    assert!(result.is_err());
}

#[test]
fn test_read_file_contents_nonexistent_directory() {
    let result: Result<ReadOutput, _> = run_tool(
        &read(),
        ReadInput {
            filepath: "/nonexistent/directory/test.txt".to_string(),
            offset: None,
            limit: None,
        },
    );

    assert!(result.is_err());
}

#[test]
fn test_read_file_contents_with_subdirectory() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    let subdir = temp_path.join("subdir");
    fs::create_dir(&subdir).unwrap();

    let mut file = File::create(subdir.join("nested.txt")).unwrap();
    writeln!(file, "Nested file content").unwrap();

    let mut fp = subdir.to_str().unwrap().to_string();
    fp.push_str("/nested.txt");
    let result: ReadOutput = run_tool(
        &read(),
        ReadInput {
            filepath: fp,
            offset: None,
            limit: None,
        },
    )
    .unwrap();

    assert_eq!(result.file_contents, "Nested file content\n");
}

#[test]
fn test_read_file_contents_unicode() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    let mut file = File::create(temp_path.join("unicode.txt")).unwrap();
    writeln!(file, "Hello 世界! 🦀").unwrap();

    let mut fp = temp_path.to_str().unwrap().to_string();
    fp.push_str("/unicode.txt");
    let result: ReadOutput = run_tool(
        &read(),
        ReadInput {
            filepath: fp,
            offset: None,
            limit: None,
        },
    )
    .unwrap();

    assert_eq!(result.file_contents, "Hello 世界! 🦀\n");
}

#[test]
fn test_read_file_contents_binary_file() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    let binary_data = vec![0u8, 1u8, 2u8, 255u8];
    fs::write(temp_path.join("binary.bin"), &binary_data).unwrap();

    let mut fp = temp_path.to_str().unwrap().to_string();
    fp.push_str("/binary.bin");
    let result: Result<ReadOutput, _> = run_tool(
        &read(),
        ReadInput {
            filepath: fp,
            offset: None,
            limit: None,
        },
    );

    if let Ok(content) = &result {
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

    fs::create_dir(temp_path.join("subdir")).unwrap();

    let mut fp = temp_path.to_str().unwrap().to_string();
    fp.push_str("/subdir");
    let result: Result<ReadOutput, _> = run_tool(
        &read(),
        ReadInput {
            filepath: fp,
            offset: None,
            limit: None,
        },
    );

    assert!(result.is_err());
}

#[test]
fn test_read_file_contents_path_traversal() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    let mut file = File::create(temp_path.join("secret.txt")).unwrap();
    writeln!(file, "Secret content").unwrap();

    let mut fp = temp_path.to_str().unwrap().to_string();
    fp.push_str("/../secret.txt");
    let result: Result<ReadOutput, _> = run_tool(
        &read(),
        ReadInput {
            filepath: fp,
            offset: None,
            limit: None,
        },
    );

    println!("Path traversal result: {:?}", result);
}
