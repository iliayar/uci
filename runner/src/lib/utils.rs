use std::path::PathBuf;

pub fn expand_home<S: AsRef<str>>(path: S) -> PathBuf {
    let home = std::env::var("HOME").expect("Should have HOME to be set");
    let home_path = path
        .as_ref()
        .strip_prefix("~/")
        .expect("expand_home expects for path to start with ~");
    PathBuf::from(format!("{}/{}", home, home_path))
}
