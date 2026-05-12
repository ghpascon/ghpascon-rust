use std::path::PathBuf;

/// Returns the directory where the current executable is located.
///
/// # Errors
///
/// Returns an error if the executable path cannot be determined or
/// if the path has no parent directory.
///
/// # Example
///
/// ```no_run
/// use ghpascon_rust::utils::path::get_working_dir;
///
/// let dir = get_working_dir().expect("could not determine executable directory");
/// println!("running from: {}", dir.display());
/// ```
pub fn get_working_dir() -> Result<PathBuf, std::io::Error> {
    let exe = std::env::current_exe()?;
    exe.parent().map(|p| p.to_path_buf()).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "executable has no parent directory",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_returns_existing_directory() {
        let dir = get_working_dir().expect("get_working_dir failed");
        assert!(dir.exists(), "directory should exist");
        assert!(dir.is_dir(), "path should be a directory");
    }
}
