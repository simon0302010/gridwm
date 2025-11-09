use chrono::{Datelike, Timelike, TimeZone};

pub fn time_widget() -> String {
    let now = chrono::offset::Local::now();

    let time_str = format!(
        "{}, {} {} {}, {:02}:{:02}:{:02}",
        now.weekday().to_string(),
        now.day(),
        now.format("%B"),
        now.year(),
        now.hour(),
        now.minute(),
        now.second()
    );

    time_str
}