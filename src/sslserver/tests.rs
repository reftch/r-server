use super::*;
use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::tempdir;
    use crate::response::ContentType;

    #[test]
    fn test_get_content_type() {
        // Test various extensions
        assert!(matches!(
            Server::get_content_type(Path::new("index.html")),
            ContentType::HTML
        ));
        assert!(matches!(
            Server::get_content_type(Path::new("style.css")),
            ContentType::CSS
        ));
        assert!(matches!(
            Server::get_content_type(Path::new("image.png")),
            ContentType::PNG
        ));
        assert!(matches!(
            Server::get_content_type(Path::new("data.json")),
            ContentType::JSON
        ));
        assert!(matches!(
            Server::get_content_type(Path::new("no_extension")),
            ContentType::UNKNOWN
        ));

        // Extra coverage for more types
        assert!(matches!(
            Server::get_content_type(Path::new("script.js")),
            ContentType::JAVASCRIPT
        ));
        assert!(matches!(
            Server::get_content_type(Path::new("text.txt")),
            ContentType::TEXT
        ));
        assert!(matches!(
            Server::get_content_type(Path::new("archive.zip")),
            ContentType::UNKNOWN
        )); // Testing unknown/unmapped extension
    }

    #[test]
    fn test_handle_static() {
        // Create a temporary directory to act as our assets folder
        let dir = tempdir().expect("Failed to create temp dir");
        let assets_path = dir.path();

        // 1. Setup: Create a dummy file in the asset directory
        let file_name = "test.txt";
        let full_path = assets_path.join(file_name);
        let mut file = File::create(&full_path).unwrap();
        writeln!(file, "hello world").unwrap();

        // 2. Test: Valid path access
        let resp = Server::handle_static(file_name, assets_path);
        assert!(resp.is_some(), "Should find the file");
        let content = String::from_utf8(resp.unwrap().body).unwrap();
        assert_eq!(content.trim(), "hello world");

        // 3. Test: Directory Traversal Attack (CRITICAL)
        // Attempting to go up one level to access system files
        let attack_path = "../../../etc/passwd";
        let traversal_resp = Server::handle_static(attack_path, assets_path);
        assert!(
            traversal_resp.is_none(),
            "Security failure: Directory traversal allowed!"
        );

        // 4. Test: Non-existent file
        let missing_resp = Server::handle_static("ghost.html", assets_path);
        assert!(
            missing_resp.is_none(),
            "Should return None for non-existent files"
        );

        // 5. Test: Directory access (should look for index.html)
        let sub_dir = assets_path.join("subdir");
        fs::create_dir(&sub_dir).unwrap();
        let index_file = sub_dir.join("index.html");
        fs::write(&index_file, "<html>index</html>").unwrap();

        let dir_resp = Server::handle_static("subdir/", assets_path);
        assert!(dir_resp.is_some(), "Should resolve directory to index.html");
        assert_eq!(
            String::from_utf8(dir_resp.unwrap().body).unwrap(),
            "<html>index</html>"
        );
    }

    #[test]
    fn test_would_block() {
        let error = std::io::Error::new(std::io::ErrorKind::WouldBlock, "would block");
        assert!(Server::would_block(&error));

        let interrupt = std::io::Error::new(std::io::ErrorKind::Interrupted, "interrupted");
        assert!(Server::would_block(&interrupt));

        let real_error = std::io::Error::new(std::io::ErrorKind::ConnectionReset, "reset");
        assert!(!Server::would_block(&real_error));
    }
}
