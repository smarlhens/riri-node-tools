const SPINNER_FRAMES: &[&str] = if cfg!(target_os = "windows") && cfg!(target_arch = "x86") {
    &["-", "\\", "|", "/"]
} else {
    &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
};

pub fn elegant_spinner() -> impl FnMut() -> &'static str {
    let mut index = 0;

    move || {
        index = (index + 1) % SPINNER_FRAMES.len();
        SPINNER_FRAMES[index]
    }
}
