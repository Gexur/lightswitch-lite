#![windows_subsystem = "windows"]

fn main() {
    let launch = lightswitch_lite::app::parse_launch();
    std::process::exit(lightswitch_lite::app::run(launch));
}
