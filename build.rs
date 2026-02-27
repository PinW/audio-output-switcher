fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/speakers.ico");
        res.set("FileDescription", "Audio Output Switcher");
        res.set("ProductName", "Audio Output Switcher");
        res.compile().expect("Failed to compile Windows resources");
    }
}
