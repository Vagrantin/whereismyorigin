// build.rs

extern crate winres;

fn main() {
    let mut res = winres::WindowsResource::new();
    res.set("FileDescription", "Find Origin/Precharge in MXF");
    res.set("ProductName", "Where is my Origin");
    res.set("LegalCopyright", "© 2025 Dalet - Matthieu Ducorps");
    res.set("FileVersion", "1.1.0");
    res.set("ProductVersion", "1.1.0");
    res.compile().unwrap();
}

