use chrono::{Datelike, Timelike};
use std::thread::sleep;
use sysinfo::{MINIMUM_CPU_UPDATE_INTERVAL, System};

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

pub fn cpu_widget() -> String {
    let mut sys = System::new();
    sys.refresh_cpu_all();
    sleep(MINIMUM_CPU_UPDATE_INTERVAL);
    sys.refresh_cpu_all();
    let mut usages: Vec<u32> = Vec::new();
    for cpu in sys.cpus() {
        usages.push(cpu.cpu_usage() as u32);
    }
    let sum: u32 = usages.iter().sum();
    let avg = if usages.len() > 0 {
        sum / usages.len() as u32
    } else {
        0
    };
    format!("CPU: {}%", avg)
}