use r_server::{
    core::http::Server,
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

            println!("Latitude: {}", latitude);
            println!("Longtitude: {}", longtitude);

            // Your temperature lookup/calculation goes here.
            // let temperature = 25.5;

            res.content_type(ContentType::JSON)
                .body(format!("{{\"value\":{}}}", latitude));
        })
        .run()?;

    Ok(())
}
