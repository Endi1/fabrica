use super::*;
use std::fs::{self, File};
use std::io::Write;
use tempfile::TempDir;

/// Helper to execute a tool with typed args and parse the result back.
fn run_tool<A: serde::Serialize, R: serde::de::DeserializeOwned>(
    tool: &ExecutableTool,
    args: A,
) -> Result<R, Box<dyn std::error::Error + Send + Sync>> {
    let args_value = serde_json::to_value(args)?;
    let result_value = tool.execute(args_value)?;
    let result: R = serde_json::from_value(result_value)?;
    Ok(result)
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
