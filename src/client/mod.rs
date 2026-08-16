/// The main client used to interact with the REST API
pub struct Client {
    host: String,
}

impl Client {
    /// Creates a new instance of the ApiClient.
    /// This is the standard way to instantiate objects in Rust.
    pub fn new(host: impl Into<String>) -> Self {
        Self { host: host.into() }
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
