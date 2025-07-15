// build.rs

extern crate winresource;

fn main() {
    let mut res = winresource::WindowsResource::new();
    res.set("FileDescription", "Find Origin/Precharge in MXF");
    res.set("ProductName", "Where is my Origin");
    res.set("LegalCopyright", "© 2025 Dalet - Matthieu Ducorps");
    res.set("FileVersion", "1.2");
    res.set("ProductVersion", "1.2");
    res.compile().unwrap();
}
