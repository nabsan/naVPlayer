use std::path::PathBuf;

use rfd::FileDialog;

pub fn pick_video_files() -> Vec<PathBuf> {
    let extensions: Vec<_> = crate::domain::media::VIDEO_EXTENSIONS
        .iter()
        .chain(crate::domain::media::IMAGE_EXTENSIONS)
        .copied()
        .collect();
    FileDialog::new()
        .add_filter("Videos and Images", &extensions)
        .add_filter("Video", crate::domain::media::VIDEO_EXTENSIONS)
        .add_filter("Images", crate::domain::media::IMAGE_EXTENSIONS)
        .set_title("Open Videos or Images")
        .pick_files()
        .unwrap_or_default()
}

/// Open the containing Explorer folder and select the current media file.
pub fn reveal_file(path: &std::path::Path) -> anyhow::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        Win32::{
            System::Com::{
                COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoTaskMemFree,
                CoUninitialize,
            },
            UI::Shell::{Common::ITEMIDLIST, SHOpenFolderAndSelectItems, SHParseDisplayName},
        },
        core::PCWSTR,
    };
    let path = std::path::absolute(path)?;
    anyhow::ensure!(path.is_file(), "The displayed file no longer exists");
    struct ComGuard;
    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    struct ItemIdList(*mut ITEMIDLIST);
    impl Drop for ItemIdList {
        fn drop(&mut self) {
            unsafe {
                CoTaskMemFree(Some(self.0.cast()));
            }
        }
    }
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok()?;
        let _com = ComGuard;
        let encoded: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut item = ItemIdList(std::ptr::null_mut());
        SHParseDisplayName(PCWSTR(encoded.as_ptr()), None, &mut item.0, 0, None)?;
        SHOpenFolderAndSelectItems(item.0, None, 0)?;
    }
    Ok(())
}
