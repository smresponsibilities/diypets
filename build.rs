fn main() {
    println!("cargo:rerun-if-changed=brand/diypets.ico");
    #[cfg(windows)]
    {
        if std::path::Path::new("brand/diypets.ico").exists() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("brand/diypets.ico");
            let _ = res.compile();
        }
    }
}