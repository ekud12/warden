fn main() {
    #[cfg(target_os = "windows")]
    {
        let bin_name = std::env::var("CARGO_BIN_NAME").unwrap_or_default();
        let description = match bin_name.as_str() {
            "warden-relay" => "Warden Relay",
            _ => "Warden",
        };
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/logo.ico");
        res.set("ProductName", "Warden");
        res.set("FileDescription", description);
        let _ = res.compile();
    }
}
