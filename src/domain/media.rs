use std::path::Path;

/// Containers accepted by every file-opening entry point. Decoders live in libmpv.
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mov", "mkv", "webm", "avi", "m4v", "wmv", "mpg", "mpeg", "ts", "mts", "m2ts", "flv",
    "ogv", "3gp",
];

pub const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "bmp", "webp", "tif", "tiff"];

pub fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            IMAGE_EXTENSIONS
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
}

pub fn is_media_path(path: &Path) -> bool {
    is_video_path(path) || is_image_path(path)
}

pub fn is_video_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            VIDEO_EXTENSIONS
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_common_containers_case_insensitively() {
        for extension in VIDEO_EXTENSIONS {
            assert!(is_video_path(Path::new(&format!(
                "日本語.{}",
                extension.to_uppercase()
            ))));
        }
        for invalid in ["movie.mp4.exe", "notes.txt", "mp4", "movie"] {
            assert!(!is_video_path(Path::new(invalid)));
        }
    }

    #[test]
    fn distinguishes_still_images_from_videos() {
        for extension in IMAGE_EXTENSIONS {
            let path = format!("photo.{}", extension.to_uppercase());
            assert!(is_image_path(Path::new(&path)));
            assert!(is_media_path(Path::new(&path)));
            assert!(!is_video_path(Path::new(&path)));
        }
        assert!(!is_image_path(Path::new("photo.jpg.exe")));
        assert!(is_media_path(Path::new("movie.mp4")));
    }
}
