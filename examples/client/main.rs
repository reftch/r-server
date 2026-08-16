use r_server::{
    client::Client,
    core::http::Server,
    debug,
    response::{ContentType, Status::BadRequest},
    router::Method,
};

fn main() -> std::io::Result<()> {
    Server::new("0.0.0.0:8082")?
        .route(Method::GET, "/api/v1/temperature", |req, res| {
            let latitude = match req.query("latitude") {
                Some(v) => v,
                None => {
                    res.status(BadRequest).body("Missing latitude");
                    return;
                }
            };

            let longtitude = match req.query("longtitude") {
                Some(v) => v,
                None => {
                    res.status(BadRequest).body("Missing longtitude");
                    return;
                }
            };

            debug!("Latitude: {}, Longtitude: {}", latitude, longtitude);

            let client = Client::new("https://api.open-meteo.com");
            let path =
                "/v1/forecast?latitude=48.78&longitude=9.18&current=temperature_2m,wind_speed_10m";

            let body = client.get(path).unwrap();
            res.content_type(ContentType::JSON).body(body);
        })
        .run()?;

    Ok(())
}
