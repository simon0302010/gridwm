use chrono::{Datelike, Timelike};
use log::{error, warn};
use std::thread::sleep;
use sysinfo::{Components, MINIMUM_CPU_UPDATE_INTERVAL, System};

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

    let mut temp_str = "Failed to get CPU temperature".to_string();
    let components = Components::new_with_refreshed_list();
    for component in &components {
        if let Some(temperature) = component.temperature() {
            if component.label().to_lowercase().contains("package") {
                temp_str = format!("{}*C", temperature);
                break;
            }
        }
    }

    format!("CPU: {}%, {}", avg, temp_str)
}

pub fn mem_widget() -> String {
    let mut sys = System::new();
    sys.refresh_memory();

    let total = sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    let used = sys.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0;

    format!("Memory: {:.1}/{:.1} GiB", used, total)
}

pub fn desktop_widget(num: usize) -> String {
    format!("Desktop {}", num + 1)
}

pub fn battery_widget() -> String {
    let manager = match battery::Manager::new() {
        Ok(m) => m,
        Err(e) => {
            error!("failed to create battery manager: {}", e);
            return "Error".to_string();
        }
    };

    let batteries = match manager.batteries() {
        Ok(bat) => bat,
        Err(e) => {
            error!("failed to get list of batteries: {}", e);
            return "Error".to_string();
        }
    };

    let mut data: Vec<String> = Vec::new();
    for (idx, maybe_battery) in batteries.enumerate() {
        let battery = match maybe_battery {
            Ok(bat) => bat,
            Err(e) => {
                warn!("failed to get info for battery #{}", e);
                continue;
            }
        };

        data.push(format!(
            "BAT {}: {:.0}% ({:?})",
            idx + 1,
            battery.state_of_charge().value * 100.0,
            battery.state() // TODO: fix it displaying "Unknown" when not charging
        ));
    }

    if !data.is_empty() {
        data.join(", ")
    } else {
        "No batteries found".to_string()
    }
}

pub fn get_widgets(widgets: &Vec<String>, desktop_num: &usize) -> String {
    let mut data: Vec<String> = Vec::new();
    for widget in widgets {
        data.push(match widget.as_str() {
            "desktop" => desktop_widget(*desktop_num),
            "time" => time_widget(),
            "cpu" => cpu_widget(),
            "mem" => mem_widget(),
            "battery" => battery_widget(),
            other => {
                warn!("no widget \"{}\" found", other);
                continue;
            }
        });
    }

    data.join(" | ")
}
