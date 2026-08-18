use r_server::{
    client::Client,
    debug,
    response::{ContentType, Status::BadRequest},
    router::Method,
    server::http::Server,
};

fn main() -> std::io::Result<()> {
    Server::new()?
        .route(Method::GET, "/api/v1/cities", |req, res| {
            let keyword = match req.query("keyword") {
                Some(v) => v,
                None => {
                    res.status(BadRequest).body("Missing keyword");
                    return;
                }
            };

            debug!("Keyword: {}", keyword);

            let client = Client::new("https://photon.komoot.io");
            let path = format!("/api/?q={}", keyword);

            let body = client.get(&path).unwrap();
            res.content_type(ContentType::JSON).body(body);
        })
        .route(Method::GET, "/api/v1/reverse", |req, res| {
            let latitude = match req.query("lat") {
                Some(v) => v,
                None => {
                    res.status(BadRequest).body("Missing latitude");
                    return;
                }
            };

            let longtitude = match req.query("lon") {
                Some(v) => v,
                None => {
                    res.status(BadRequest).body("Missing longtitude");
                    return;
                }
            };

            debug!("Latitude: {}, Longtitude: {}", latitude, longtitude);

            let client = Client::new("https://photon.komoot.io");
            let path = format!("/reverse?lat={}&lon={}", latitude, longtitude);

            let body = client.get(&path).unwrap();
            res.content_type(ContentType::JSON).body(body);
        })
        .route(Method::GET, "/api/v1/temperature", |req, res| {
            let latitude = match req.query("latidude") {
                Some(v) => v,
                None => {
                    res.status(BadRequest).body("Missing latidude");
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
            let path = format!(
                    "/v1/forecast?latitude={}&longitude={}\
                    &current=temperature_2m&current=weather_code&current=wind_speed_10m&current=cloud_cover\
                    &hourly=temperature_2m,precipitation_probability,wind_speed_10m,cloud_cover\
                    &daily=temperature_2m_max,temperature_2m_min,sunrise,sunset,weather_code,precipitation_sum,wind_speed_10m_max\
                    &forecast_days=10&timezone=auto",
                    latitude, longtitude
                );

            let body = client.get(&path).unwrap();
            res.content_type(ContentType::JSON).body(body);
        })
        .run()?;

    Ok(())
}
