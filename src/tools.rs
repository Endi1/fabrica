use std::{fs, io::Result, path::Path};

pub fn get_directory_contents<P: AsRef<Path>>(path: P) -> Result<Vec<String>> {
    let entries = fs::read_dir(path)?;
    let mut contents = Vec::new();
    for entry in entries {
        let entry = entry?;
        contents.push(entry.file_name().to_string_lossy().to_string());
    }

    Ok(contents)
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::TempDir;

    #[test]
    fn test_get_directory_contents_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let contents = get_directory_contents(temp_dir.path()).unwrap();
        assert_eq!(contents.len(), 0);
    }

    #[test]
    fn test_get_directory_contents_with_files() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Create test files
        File::create(temp_path.join("file1.txt")).unwrap();
        File::create(temp_path.join("file2.rs")).unwrap();
        
        let mut contents = get_directory_contents(temp_path).unwrap();
        contents.sort();
        
        assert_eq!(contents.len(), 2);
        assert!(contents.contains(&"file1.txt".to_string()));
        assert!(contents.contains(&"file2.rs".to_string()));
    }

    #[test]
    fn test_get_directory_contents_with_subdirs() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Create subdirectory
        fs::create_dir(temp_path.join("subdir")).unwrap();
        File::create(temp_path.join("file.txt")).unwrap();
        
        let mut contents = get_directory_contents(temp_path).unwrap();
        contents.sort();
        
        assert_eq!(contents.len(), 2);
        assert!(contents.contains(&"subdir".to_string()));
        assert!(contents.contains(&"file.txt".to_string()));
    }

    #[test]
    fn test_get_directory_contents_nonexistent_path() {
        let result = get_directory_contents("/nonexistent/path");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_directory_contents_file_as_path() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        File::create(&file_path).unwrap();
        
        let result = get_directory_contents(&file_path);
        assert!(result.is_err());
    }
}
