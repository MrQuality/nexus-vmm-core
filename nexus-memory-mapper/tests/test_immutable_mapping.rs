use nexus_memory_mapper::map_secret_read_only;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

#[test]
fn test_mmap_secret_is_zero_copy_and_readonly() {
    // a. Create a temporary file on the host containing a dummy Kubernetes Secret string
    let secret_content = b"db-password-123";
    let mut temp_path = std::env::temp_dir();
    temp_path.push("nexus_test_secret");

    let mut file = fs::File::create(&temp_path).expect("failed to create temp secret file");
    file.write_all(secret_content)
        .expect("failed to write secret content");
    file.sync_all().expect("failed to sync secret file");

    // b. It must pass the file's path to map_secret_read_only
    // c. It must assert that the returned memory slice matches the bytes
    let mapped_slice = map_secret_read_only(&temp_path);

    assert_eq!(
        mapped_slice, secret_content,
        "Mapped memory content does not match dummy secret"
    );

    // Cleanup
    let _ = fs::remove_file(&temp_path);
}
