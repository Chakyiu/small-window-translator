fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=resources/icons/icon.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set("ProductName", "Small Window Translator");
        res.set("FileDescription", "Select-text translator");
        res.set("CompanyName", "swtrans");
        res.set("LegalCopyright", "Copyright (c) 2026 chakyiu");
        res.set("OriginalFilename", "swtrans.exe");
        res.set("InternalName", "swtrans");
        res.set("Comments", "https://github.com/Chakyiu/small-window-translator");
        res.set_icon("resources/icons/icon.ico");
        if let Err(err) = res.compile() {
            println!("cargo:warning=winresource: {err}");
        }
    }
}
