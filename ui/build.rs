fn main() {
    // Windows: set executable icon using winres and assets/dome.ico
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/dome.ico");
        res.compile().unwrap();
    }
}
