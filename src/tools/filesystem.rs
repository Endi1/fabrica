use std::{fs, path::Path};

use crate::tools::{
    EditInput, EditOutput, ExecutableTool, ReadInput, ReadOutput, ToolRegistry, WriteInput,
    WriteOutput, bash,
};

pub fn get_filesystem_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(read());
    registry.register(write());
    registry.register(edit());
    registry.register(bash());
    registry
}

pub fn read() -> ExecutableTool {
    ExecutableTool::new(
        "read",
        "Reads a file from the local filesystem. You can access any file directly by using this tool.
Assume this tool is able to read all files on the machine. If the User provides a path to a file assume that path is valid. It is okay to read a file that does not exist; an error will be returned.

Usage:
- The file_path parameter must be an absolute path, not a relative path
- By default, it reads up to 2000 lines starting from the beginning of the file
- You can optionally specify a line offset and limit (especially handy for long files), but it's recommended to read the whole file by not providing these parameters
- Any lines longer than 2000 characters will be truncated
- Results are returned using cat -n format, with line numbers starting at 1
- This tool allows Claude Code to read images (eg PNG, JPG, etc). When reading an image file the contents are presented visually as Claude Code is a multimodal LLM.
- This tool can read PDF files (.pdf). PDFs are processed page by page, extracting both text and visual content for analysis.
- This tool can read Jupyter notebooks (.ipynb files) and returns all cells with their outputs, combining code, text, and visualizations.
- You have the capability to call multiple tools in a single response. It is always better to speculatively read multiple files as a batch that are potentially useful.
- You will regularly be asked to read screenshots. If the user provides a path to a screenshot ALWAYS use this tool to view the file at the path. This tool will work with all temporary file paths like /var/folders/123/abc/T/TemporaryItems/NSIRD_screencaptureui_ZfB1tD/Screenshot.png
- If you read a file that exists but has empty contents you will receive a system reminder warning in place of file contents.
",
        |arg: ReadInput| {
            let file_path = Path::new(&arg.filepath);
            let contents = fs::read_to_string(file_path)?;
            Ok(ReadOutput {
                file_contents: contents,
            })
        },
    )
}

pub fn write() -> ExecutableTool {
    ExecutableTool::new(
        "write",
        "Writes a file to the local filesystem.

Usage:
- The filepath parameter must be an absolute path, not a relative path
- If the file already exists, it will be overwritten with the new content
- If the file does not exist, it will be created
- Parent directories will be created if they do not already exist
- Prefer editing existing files over creating new ones when possible
- Only use this tool when you are confident the full file contents should be written
- Returns the number of bytes written on success
",
        |arg: WriteInput| {
            let file_path = Path::new(&arg.filepath);
            if let Some(parent) = file_path.parent()
                && !parent.as_os_str().is_empty()
            {
                fs::create_dir_all(parent)?;
            }
            fs::write(file_path, &arg.content)?;
            Ok(WriteOutput {
                bytes_written: arg.content.len() as u64,
            })
        },
    )
}

pub fn edit() -> ExecutableTool {
    ExecutableTool::new(
        "edit",
        "Edits an existing file by replacing a region of text, identified by line/column coordinates, with new content.

Usage:
- The filepath parameter must be an absolute path to an existing file
- start_line, start_column, end_line, end_column are all 1-indexed
- The start coordinates are inclusive; the end coordinates are exclusive (like a text editor selection)
- Columns count characters (Unicode scalar values), not bytes
- The selected region (from start to end) is removed and replaced with new_content verbatim
- new_content may contain newlines and may be empty (to delete the region)
- Returns the text that was replaced and the new total size in bytes
- Prefer this tool for small, localized edits; use `write` only when rewriting a whole file
",
        |arg: EditInput| {
            let file_path = Path::new(&arg.filepath);
            let original = fs::read_to_string(file_path)?;

            let start_offset = line_col_to_byte_offset(&original, arg.start_line, arg.start_column)
                .ok_or_else(|| {
                    format!(
                        "start coordinates ({}, {}) are out of bounds",
                        arg.start_line, arg.start_column
                    )
                })?;
            let end_offset = line_col_to_byte_offset(&original, arg.end_line, arg.end_column)
                .ok_or_else(|| {
                    format!(
                        "end coordinates ({}, {}) are out of bounds",
                        arg.end_line, arg.end_column
                    )
                })?;

            if end_offset < start_offset {
                return Err("end coordinates must not be before start coordinates".into());
            }

            let replaced_text = original[start_offset..end_offset].to_string();
            let mut new_contents =
                String::with_capacity(original.len() - replaced_text.len() + arg.new_content.len());
            new_contents.push_str(&original[..start_offset]);
            new_contents.push_str(&arg.new_content);
            new_contents.push_str(&original[end_offset..]);

            fs::write(file_path, &new_contents)?;

            Ok(EditOutput {
                bytes_written: new_contents.len() as u64,
                replaced_text,
            })
        },
    )
}

/// Converts a 1-indexed (line, column) pair into a byte offset within `text`.
/// Column counts characters (Unicode scalar values). A column equal to the line
/// length + 1 refers to the position just past the end of the line (i.e. the
/// newline or end of file). Returns `None` if the coordinates are out of range.
fn line_col_to_byte_offset(text: &str, line: u32, column: u32) -> Option<usize> {
    if line == 0 || column == 0 {
        return None;
    }
    let target_line = line as usize;
    let target_col = column as usize;

    let mut cur_line: usize = 1;
    let mut cur_col: usize = 1;

    for (i, ch) in text.char_indices() {
        if cur_line == target_line && cur_col == target_col {
            return Some(i);
        }
        if ch == '\n' {
            // Allow addressing the position of/just past the newline on this line.
            if cur_line == target_line && cur_col + 1 == target_col {
                return Some(i);
            }
            cur_line += 1;
            cur_col = 1;
        } else {
            cur_col += 1;
        }
    }

    if cur_line == target_line && cur_col == target_col {
        return Some(text.len());
    }
    None
}
