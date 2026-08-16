/// The main client used to interact with the REST API
pub struct Client {
    host: String,
    is_secure: bool,
}

impl Client {
    /// Creates a new instance of the Client.
    /// It automatically detects if the host uses https.
    pub fn new(host: impl Into<String>) -> Self {
        let host_str = host.into();

        // Check if the host starts with https://
        let is_secure = host_str.starts_with("https://");

        // Optional: Strip the protocol from the host string so
        // the stored host is just the domain/ip (e.g., "google.com" instead of "https://google.com")
        // This makes path concatenation easier later.
        let cleaned_host = if is_secure {
            host_str
                .strip_prefix("https://")
                .unwrap_or(&host_str)
                .to_string()
        } else if host_str.starts_with("http://") {
            host_str
                .strip_prefix("http://")
                .unwrap_or(&host_str)
                .to_string()
        } else {
            host_str
        };

        Self {
            host: cleaned_host,
            is_secure,
        }
    }

    pub async fn get(&self, path: &str) -> Result<String, String> {
        println!("GET request to: {}/{}", self.host, path);
        // Implementation goes here
        Ok("Success".to_string())
    }

    pub async fn post(&self, path: &str, body: String) -> Result<String, String> {
        println!("POST request to: {}/{}", self.host, path);
        // Implementation goes here
        Ok("Created".to_string())
    }

    pub async fn put(&self, path: &str, body: String) -> Result<String, String> {
        println!("PUT request to: {}/{}", self.host, path);
        Ok("Updated".to_string())
    }

    pub async fn patch(&self, path: &str, body: String) -> Result<String, String> {
        println!("PATCH request to: {}/{}", self.host, path);
        Ok("Patched".to_string())
    }

    pub async fn delete(&self, path: &str) -> Result<String, String> {
        println!("DELETE request to: {}/{}", self.host, path);
        Ok("Deleted".to_string())
    }
}
