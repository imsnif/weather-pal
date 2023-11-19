use zellij_tile::prelude::*;

use std::collections::BTreeMap;
use json;
use chrono::{self, Timelike};

const TIMEZONE_COMMAND_ID: &str = "TIMEZONE_COMMAND_ID";

#[derive(Default)]
struct HourlyData {
    temperature_2m: f64,
    precipitation_probability: usize,
    wind_speed_10m: f64,
    wind_direction_10m: usize,
    wmo_code: usize,
}

#[derive(Default)]
struct State {
    weather_data: BTreeMap<usize, HourlyData>,
    requested_timezone: Option<String>,
    weather_location: Option<String>,
    geolocation: Option<(f64, f64)>, // lat, lon
    error: Option<String>,
    fetching_data: bool,
    location_being_typed: Option<String>,
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        if let Some(location) = configuration.get("location") {
            self.requested_timezone = Some(location.clone());
        }
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::RunCommands,
            PermissionType::WebAccess
        ]);
        subscribe(&[
            EventType::Key,
            EventType::WebRequestResult,
            EventType::RunCommandResult
        ]);
    }

    fn update(&mut self, event: Event) -> bool {
        let mut should_render = false;
        match event {
            Event::PermissionRequestResult(..) => {
                self.discover_local_timezone_or_make_geocode_request();
            }
            Event::RunCommandResult(exit_code, stdout, stderr, context) => {
                if !stderr.is_empty() {
                    let error = String::from_utf8(stderr).unwrap_or("".to_owned());
                    self.error = Some(format!("Error fetching timezone: {}", error));
                }
                if &context.get("id").map(|s| s.as_str()) == &Some(TIMEZONE_COMMAND_ID) && exit_code == Some(0) {
                    self.requested_timezone = String::from_utf8(stdout).ok().map(|s| s.trim().to_owned());
                }
                make_geocode_request(&self.requested_timezone);
            }
            Event::WebRequestResult(status_code, _headers, body, context) => {
                match context.get("id").map(|s| s.as_str()) {
                    Some("weather") => {
                        if status_code != 200 {
                            self.error = Some("Failed weather web request".to_owned());
                        } else {
                            match parse_weather_data(body) {
                                Ok(weather_data) => {
                                    self.weather_data = weather_data;
                                    self.fetching_data = false;
                                }
                                Err(e) => self.error = Some(format!("Failed to parse data: {}", e)),
                            }
                        }
                        should_render = true;
                    }
                    Some("geocode") => {
                        if status_code != 200 {
                            self.error = Some("Failed geocode web request".to_owned());
                        } else {
                            match parse_lat_lon_and_location(body) {
                                Ok((latitude, longitude, location)) => {
                                    self.geolocation = Some((latitude, longitude));
                                    self.weather_location = Some(location);
                                    make_weather_web_request(latitude, longitude);
                                },
                                Err(e) => self.error = Some(format!("Failed to parse geocode: {}", e)),
                            }
                        }
                        should_render = true;
                    }
                    _ => {}
                }
            }
            Event::Key(key) => {
                if let Key::Char('\n') = key {
                    if let Some(_error) = self.error.take() {
                        self.fetching_data = false;
                    } else {
                        if let Some(location) = self.location_being_typed.take() {
                            self.requested_timezone = Some(location);
                        }
                        self.fetching_data = true;
                        self.discover_local_timezone_or_make_geocode_request();
                    }
                    should_render = true;
                } else if let Key::Ctrl('w') = key {
                    self.error = None;
                    self.location_being_typed = Some(String::new());
                    should_render = true;
                } else if let Key::Backspace = key {
                    self.location_being_typed.as_mut().map(|l| l.pop());
                    should_render = true;
                } else if let Key::Char(character) = key {
                    self.location_being_typed.as_mut().map(|l| l.push(character));
                    should_render = true;
                }
            }
            _ => (),
        };
        should_render
    }

    fn render(&mut self, rows: usize, cols: usize) {
        let time = chrono::Local::now();
        let hour = time.hour() as usize;
        if let Some(error) = &self.error {
            print_text_with_coordinates(Text::new(error).color_range(3, ..), (cols / 2).saturating_sub(error.chars().count() / 2), rows / 2, None, None);
            let controls_text = "Press <ENTER> to reload, <Ctrl-w> to enter a new location";
            print_text_with_coordinates(Text::new(controls_text).color_range(3, 6..13).color_range(3, 25..33), 0, rows, None, None);
        } else if let Some(location_being_typed) = &self.location_being_typed {
            let location_being_typed = format!("Enter desired location: {}_", location_being_typed);
            print_text_with_coordinates(Text::new(&location_being_typed).color_range(3, ..), (cols / 2).saturating_sub(location_being_typed.chars().count() / 2), rows / 2, None, None);
        } else if self.fetching_data {
            let fetching_data_text = "Fetching data...";
            print_text_with_coordinates(Text::new(fetching_data_text).color_range(3, ..), (cols / 2).saturating_sub(fetching_data_text.chars().count() / 2), rows / 2, None, None);
        } else if self.weather_data.is_empty() {
            let controls_text = "Press <ENTER> to run, <Ctrl-w> to enter a new location";
            print_text_with_coordinates(Text::new(controls_text).color_range(3, 6..13).color_range(3, 22..30), (cols / 2).saturating_sub(controls_text.chars().count() / 2), rows / 2, None, None);
        } else {
            if let Some(location) = &self.weather_location {
                print_text_with_coordinates(Text::new(location).color_range(3, ..), (cols / 2).saturating_sub(location.chars().count() / 2), (rows / 2).saturating_sub(5), None, None);
            }
            let mut weather_table = Table::new().add_row(vec![" ", " ", " ", " ", " ", " "]);
            let mut longest_line = 0;
            for (hour, hourly_data) in self.weather_data.iter().skip(hour).take(8) {
                let hour = if hour > &23 { hour - 23 } else { hour + 1 };
                let hour_string = if hour > 9 { hour.to_string() } else { format!("0{}", hour)};
                let hour_text = format!("{}:00", hour_string);
                let (wmo_code_text, wmo_code_len) = wmo_code_to_text(hourly_data.wmo_code);
                let degrees_text = format!("{}", hourly_data.temperature_2m);
                let degrees_symbol_text = "°C";
                let precipitation_text = format!("💧 {}% ", hourly_data.precipitation_probability);
                let wind_direction_text = format!("{}  {}kph", wind_direction_arrow(hourly_data.wind_direction_10m), hourly_data.wind_speed_10m);
                let line_len = hour_text.chars().count() + wmo_code_len + degrees_text.chars().count() + degrees_symbol_text.chars().count() + (precipitation_text.chars().count() + 1) + (wind_direction_text.chars().count() + 1);
                if line_len > longest_line {
                    longest_line = line_len;
                }
                weather_table = weather_table.add_styled_row(vec![
                    Text::new(hour_text).color_range(0, ..),
                    wmo_code_text,
                    Text::new(degrees_text).color_range(2, ..),
                    Text::new(degrees_symbol_text).color_range(2, ..),
                    Text::new(precipitation_text).color_range(1, ..),
                    Text::new(wind_direction_text),
                ]);
            }
            let controls_text = "Press <ENTER> to reload, <Ctrl-w> to enter a new location";
            print_text_with_coordinates(Text::new(controls_text).color_range(3, 6..13).color_range(3, 25..33), 0, rows, None, None);
            print_table_with_coordinates(weather_table, (cols / 2).saturating_sub((longest_line + 5) / 2), (rows / 2).saturating_sub(9 / 2), None, None);
        }
    }
}

impl State {
    fn discover_local_timezone_or_make_geocode_request(&self) {
        if self.requested_timezone.is_some() {
            make_geocode_request(&self.requested_timezone);
        } else {
            let mut run_command_context = BTreeMap::new();
            run_command_context.insert("id".to_owned(), "TIMEZONE_COMMAND_ID".to_owned());
            run_command(&vec!["bash", "-c", "timedatectl | grep \"Time zone\" | awk \'{print $3}\'"], run_command_context);
        }
    }
}

fn wind_direction_arrow(degrees: usize) -> char {
    if degrees < 45 || degrees == 360 {
        '↓' // north
    } else if degrees < 90 {
        '↙' // north-east
    } else if degrees < 135 {
        '←' // east
    } else if degrees < 180 {
        '↖' // south-east
    } else if degrees < 225 {
        '↑' // south
    } else if degrees < 270 {
        '↗' // south-west
    } else if degrees < 315 {
        '→' // west
    } else if degrees < 360 {
        '↘' // north-west
    } else {
        '?'
    }
}

fn wmo_code_to_text(wmo_code: usize) -> (Text, usize) { // text + len
    if wmo_code == 0 {
        let text = "CLEAR SKY";
        (Text::new(text), text.chars().count())
    } else if wmo_code == 1 {
        let text = "MAINLY CLEAR";
        (Text::new(text), text.chars().count())
    } else if wmo_code == 2 {
        let text = "PARTLY CLOUDY";
        (Text::new(text), text.chars().count())
    } else if wmo_code == 3 {
        let text = "OVERCAST";
        (Text::new(text), text.chars().count())
    } else if wmo_code == 45 || wmo_code == 48 {
        let text = "FOG";
        (Text::new(text).color_range(1, ..), text.chars().count())
    } else if wmo_code == 51 {
        let text = "LIGHT DRIZZLE";
        (Text::new(text).color_range(1, ..), text.chars().count())
    } else if wmo_code == 53 {
        let text = "MODERATE DRIZZLE";
        (Text::new(text).color_range(1, ..), text.chars().count())
    } else if wmo_code == 53 {
        let text = "DENSE DRIZZLE";
        (Text::new(text).color_range(3, ..), text.chars().count())
    } else if wmo_code == 56 {
        let text = "FREEZING DRIZZLE (LIGHT)";
        (Text::new(text).color_range(1, ..), text.chars().count())
    } else if wmo_code == 57 {
        let text = "FREEZING DRIZZLE (DENSE)";
        (Text::new(text).color_range(3, ..), text.chars().count())
    } else if wmo_code == 61 {
        let text = "SLIGHT RAIN";
        (Text::new(text).color_range(1, ..), text.chars().count())
    } else if wmo_code == 63 {
        let text = "MODERATE RAIN";
        (Text::new(text).color_range(1, ..), text.chars().count())
    } else if wmo_code == 65 {
        let text = "HEAVY RAIN";
        (Text::new(text).color_range(3, ..), text.chars().count())
    } else if wmo_code == 66 {
        let text = "FREEZING RAIN (LIGHT)";
        (Text::new(text).color_range(1, ..), text.chars().count())
    } else if wmo_code == 67 {
        let text = "FREEZING RAIN (HEAVY)";
        (Text::new(text).color_range(3, ..), text.chars().count())
    } else if wmo_code == 71 {
        let text = "SLIGHT SNOW";
        (Text::new(text).color_range(1, ..), text.chars().count())
    } else if wmo_code == 73 {
        let text = "MODERATE SNOW";
        (Text::new(text).color_range(3, ..), text.chars().count())
    } else if wmo_code == 75 {
        let text = "HEAVY SNOW";
        (Text::new(text).color_range(3, ..), text.chars().count())
    } else if wmo_code == 77 {
        let text = "SNOW GRAINS";
        (Text::new(text).color_range(3, ..), text.chars().count())
    } else if wmo_code == 80 {
        let text = "RAIN SHOWERS (SLIGHT)";
        (Text::new(text).color_range(1, ..), text.chars().count())
    } else if wmo_code == 81 {
        let text = "RAIN SHOWERS (MODERATE)";
        (Text::new(text).color_range(1, ..), text.chars().count())
    } else if wmo_code == 82 {
        let text = "RAIN SHOWERS (VIOLENT)";
        (Text::new(text).color_range(3, ..), text.chars().count())
    } else if wmo_code == 85 {
        let text = "SNOW SHOWERS (SLIGHT)";
        (Text::new(text).color_range(1, ..), text.chars().count())
    } else if wmo_code == 86 {
        let text = "SNOW SHOWERS (HEAVY)";
        (Text::new(text).color_range(3, ..), text.chars().count())
    } else if wmo_code == 95 {
        let text = "THUNDERSTORM";
        (Text::new(text).color_range(3, ..), text.chars().count())
    } else if wmo_code == 96 {
        let text = "THUNDERSTORM (SLIGHT HAIL)";
        (Text::new(text).color_range(3, ..), text.chars().count())
    } else if wmo_code == 99 {
        let text = "THUNDERSTORM (HEAVY HAIL)";
        (Text::new(text).color_range(3, ..), text.chars().count())
    } else {
        (Text::new(""), 0)
    }

}

fn parse_weather_data(body: Vec<u8>) -> Result<BTreeMap<usize, HourlyData>, String> {

    String::from_utf8(body)
        .map_err(|e| e.to_string())
        .and_then(|b| json::parse(&b).map_err(|e| e.to_string()))
        .and_then(|body| {
            let mut weather_data = BTreeMap::new();
            for i in 0..167 {
                let temperature_2m = body["hourly"]["temperature_2m"][i].as_f64().ok_or_else(|| "Failed to parse temperature".to_owned())?;
                let precipitation_probability = body["hourly"]["precipitation_probability"][i].as_usize().ok_or_else(|| "Failed to parse precipitation_probability".to_owned())?;
                let wind_speed_10m = body["hourly"]["wind_speed_10m"][i].as_f64().ok_or_else(|| "Failed to parse wind speed".to_owned())?;
                let wind_direction_10m = body["hourly"]["wind_direction_10m"][i].as_usize().ok_or_else(|| "Failed to parse wind direction")?;
                let wmo_code = body["hourly"]["weather_code"][i].as_usize().ok_or_else(|| "Failed to parse weather code")?;
                weather_data.insert(i, HourlyData {
                    temperature_2m,
                    precipitation_probability,
                    wind_speed_10m,
                    wind_direction_10m,
                    wmo_code,
                });
            }
            Ok(weather_data)
        })


}

fn parse_lat_lon_and_location(body: Vec<u8>) -> Result<(f64, f64, String), String> {
    String::from_utf8(body)
    .map_err(|e| e.to_string())
    .and_then(|b| json::parse(&b).map_err(|e| e.to_string()))
    .and_then(|body| {
        let latitude = body["results"][0]["latitude"].as_f64().ok_or("Failed to parse latitude")?;
        let longitude = body["results"][0]["longitude"].as_f64().ok_or("Failed to parse longitude")?;
        let city = body["results"][0]["name"].as_str().ok_or("Failed to parse city")?;
        let country = body["results"][0]["country"].as_str().ok_or("Failed to parse country")?;
        Ok((latitude, longitude, format!("{}, {}", city, country)))
    })
}

fn make_weather_web_request(latitude: f64, longitude: f64) {
    let mut context = BTreeMap::new();
    context.insert("id".to_owned(), "weather".to_owned());
    web_request(
        format!("https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&hourly=temperature_2m,precipitation_probability,wind_speed_10m,wind_direction_10m,weather_code", latitude, longitude),
        HttpVerb::Get,
        BTreeMap::new(),
        vec![],
        context,
    );
}

fn make_geocode_request(timezone: &Option<String>) {
    if let Some(city) = timezone.as_ref().and_then(|t| t.split('/').last()).map(|c| c.replace(' ', "+").replace('-', "+")) {
        let mut context = BTreeMap::new();
        context.insert("id".to_owned(), "geocode".to_owned());
        web_request(
            format!("https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=en&format=json", city),
            HttpVerb::Get,
            BTreeMap::new(),
            vec![],
            context,
        );
    }
}
