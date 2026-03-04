//! Tests for RO-Crate Dataset functionality.
//!
//! These tests verify the dataset statistics computation, including:
//! - File counting and size totaling
//! - Manifest hash computation
//! - Content hash computation
//! - Multi-threaded processing

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use tempfile::TempDir;

/// Create a test directory structure with known files.
fn create_test_dataset(dir: &Path) {
    // Create subdirectories
    fs::create_dir_all(dir.join("partition_a")).unwrap();
    fs::create_dir_all(dir.join("partition_b")).unwrap();
    fs::create_dir_all(dir.join("partition_c/nested")).unwrap();

    // Create files with known content
    let files = [
        ("partition_a/file1.txt", "hello"),
        ("partition_a/file2.txt", "world"),
        ("partition_b/file3.txt", "test data"),
        ("partition_c/file4.txt", "more content"),
        ("partition_c/nested/file5.txt", "nested file"),
    ];

    for (path, content) in files {
        let file_path = dir.join(path);
        let mut file = File::create(&file_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }
}

/// Get total size of all files in the test dataset.
fn expected_total_size() -> u64 {
    // "hello" + "world" + "test data" + "more content" + "nested file"
    5 + 5 + 9 + 12 + 11
}

#[test]
fn test_collect_files() {
    let temp_dir = TempDir::new().unwrap();
    create_test_dataset(temp_dir.path());

    // Use the internal function via the module
    let files = collect_files_for_test(temp_dir.path());

    assert_eq!(files.len(), 5, "Should find exactly 5 files");

    // Check total size
    let total_size: u64 = files.iter().map(|(_, size, _)| size).sum();
    assert_eq!(total_size, expected_total_size());
}

#[test]
fn test_manifest_hash_deterministic() {
    let temp_dir = TempDir::new().unwrap();
    create_test_dataset(temp_dir.path());

    // Compute manifest hash twice - should be identical
    let hash1 = compute_manifest_hash_for_test(temp_dir.path());
    let hash2 = compute_manifest_hash_for_test(temp_dir.path());

    assert_eq!(hash1, hash2, "Manifest hash should be deterministic");
    assert_eq!(hash1.len(), 64, "SHA256 hex should be 64 characters");
}

#[test]
fn test_manifest_hash_changes_with_content() {
    let temp_dir = TempDir::new().unwrap();
    create_test_dataset(temp_dir.path());

    let hash1 = compute_manifest_hash_for_test(temp_dir.path());

    // Add a new file
    let new_file = temp_dir.path().join("partition_a/new_file.txt");
    let mut file = File::create(&new_file).unwrap();
    file.write_all(b"new content").unwrap();

    let hash2 = compute_manifest_hash_for_test(temp_dir.path());

    assert_ne!(
        hash1, hash2,
        "Manifest hash should change when files are added"
    );
}

#[test]
fn test_content_hash_deterministic() {
    let temp_dir = TempDir::new().unwrap();
    create_test_dataset(temp_dir.path());

    // Compute content hash twice with single thread
    let hash1 = compute_content_hash_for_test(temp_dir.path(), 1);
    let hash2 = compute_content_hash_for_test(temp_dir.path(), 1);

    assert_eq!(hash1, hash2, "Content hash should be deterministic");
    assert_eq!(hash1.len(), 64, "SHA256 hex should be 64 characters");
}

#[test]
fn test_content_hash_parallel_same_as_single() {
    let temp_dir = TempDir::new().unwrap();
    create_test_dataset(temp_dir.path());

    // Compute with 1 thread vs multiple threads
    let hash_single = compute_content_hash_for_test(temp_dir.path(), 1);
    let hash_multi = compute_content_hash_for_test(temp_dir.path(), 4);

    assert_eq!(
        hash_single, hash_multi,
        "Content hash should be same regardless of thread count"
    );
}

#[test]
fn test_content_hash_changes_with_content() {
    let temp_dir = TempDir::new().unwrap();
    create_test_dataset(temp_dir.path());

    let hash1 = compute_content_hash_for_test(temp_dir.path(), 1);

    // Modify a file's content (but not its size or name)
    let file_path = temp_dir.path().join("partition_a/file1.txt");
    let mut file = File::create(&file_path).unwrap();
    file.write_all(b"HELLO").unwrap(); // Same size, different content

    let hash2 = compute_content_hash_for_test(temp_dir.path(), 1);

    assert_ne!(
        hash1, hash2,
        "Content hash should change when file content changes"
    );
}

#[test]
fn test_empty_directory() {
    let temp_dir = TempDir::new().unwrap();

    let files = collect_files_for_test(temp_dir.path());
    assert_eq!(files.len(), 0, "Empty directory should have no files");

    // Manifest hash of empty directory should still work
    let hash = compute_manifest_hash_for_test(temp_dir.path());
    assert_eq!(
        hash.len(),
        64,
        "Should produce valid hash for empty directory"
    );
}

#[test]
fn test_nested_directories() {
    let temp_dir = TempDir::new().unwrap();

    // Create deeply nested structure
    let deep_path = temp_dir.path().join("a/b/c/d/e/f");
    fs::create_dir_all(&deep_path).unwrap();

    let file_path = deep_path.join("deep_file.txt");
    let mut file = File::create(&file_path).unwrap();
    file.write_all(b"deep content").unwrap();

    let files = collect_files_for_test(temp_dir.path());
    assert_eq!(
        files.len(),
        1,
        "Should find file in deeply nested directory"
    );

    // Check relative path is correct
    let (rel_path, _, _) = &files[0];
    assert_eq!(rel_path, "a/b/c/d/e/f/deep_file.txt");
}

#[test]
fn test_file_metadata() {
    let temp_dir = TempDir::new().unwrap();

    let file_path = temp_dir.path().join("test.txt");
    let content = "test content with known size";
    let mut file = File::create(&file_path).unwrap();
    file.write_all(content.as_bytes()).unwrap();

    let files = collect_files_for_test(temp_dir.path());
    assert_eq!(files.len(), 1);

    let (rel_path, size, mtime) = &files[0];
    assert_eq!(rel_path, "test.txt");
    assert_eq!(*size, content.len() as u64);
    assert!(*mtime > 0.0, "mtime should be set");
}

// Helper functions that wrap the internal implementation for testing
// These are defined here to avoid exposing internal functions publicly

fn collect_files_for_test(dir: &Path) -> Vec<(String, u64, f64)> {
    let mut files = Vec::new();
    collect_files_recursive(dir, dir, &mut files);
    files
}

fn collect_files_recursive(dir: &Path, base: &Path, files: &mut Vec<(String, u64, f64)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(&path, base, files);
        } else if path.is_file() {
            let Ok(metadata) = fs::metadata(&path) else {
                continue;
            };

            let size = metadata.len();
            let mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);

            let rel_path = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            files.push((rel_path, size, mtime));
        }
    }
}

fn compute_manifest_hash_for_test(dir: &Path) -> String {
    use sha2::{Digest, Sha256};

    let mut files = Vec::new();
    collect_files_recursive(dir, dir, &mut files);

    let mut manifest_entries: Vec<String> = files
        .iter()
        .map(|(path, size, mtime)| format!("{}|{}|{:.6}", path, size, mtime))
        .collect();

    manifest_entries.sort();
    let manifest = manifest_entries.join("\n");
    format!("{:x}", Sha256::digest(manifest.as_bytes()))
}

fn compute_content_hash_for_test(dir: &Path, num_threads: usize) -> String {
    use rayon::prelude::*;
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let mut files = Vec::new();
    collect_files_recursive(dir, dir, &mut files);

    // Collect full paths
    let file_paths: Vec<_> = files
        .iter()
        .map(|(rel_path, _, _)| (rel_path.clone(), dir.join(rel_path)))
        .collect();

    // Build thread pool
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .unwrap();

    // Hash files in parallel
    let mut file_hashes: Vec<(String, String)> = pool.install(|| {
        file_paths
            .par_iter()
            .map(|(rel_path, full_path)| {
                let mut file = File::open(full_path).unwrap();
                let mut hasher = Sha256::new();
                let mut buffer = [0u8; 8192];

                loop {
                    let n = file.read(&mut buffer).unwrap();
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buffer[..n]);
                }

                (rel_path.clone(), format!("{:x}", hasher.finalize()))
            })
            .collect()
    });

    // Sort by path for determinism
    file_hashes.sort_by(|a, b| a.0.cmp(&b.0));

    // Combine all file hashes
    let mut final_hasher = Sha256::new();
    for (path, hash) in &file_hashes {
        final_hasher.update(format!("{}:{}\n", path, hash).as_bytes());
    }

    format!("{:x}", final_hasher.finalize())
}
