fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("icon.ico");
        res.set("ProductName", "Pylade Clicker");
        res.set("FileDescription", "Pylade Clicker - Auto Clicker");
        res.set("LegalCopyright", "Copyright (C) 2024");
        res.set("FileVersion", "1.0.0.0");
        res.set("ProductVersion", "1.0.0.0");
        match res.compile() {
            Ok(_) => println!("Icon compiled successfully."),
            Err(e) => panic!("Failed to compile resources: {:?}", e),
        }
    }
}
