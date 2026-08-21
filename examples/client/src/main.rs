use r_server::{client::Client, info};

fn main() -> std::io::Result<()> {
    let latitude = "52.23455";
    let longtitude = "13.23455";
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
    info!("Respond: {}", body);

    Ok(())
}
