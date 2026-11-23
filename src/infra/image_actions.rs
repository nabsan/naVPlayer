use anyhow::{Result, bail, ensure};
use std::{
    cmp::Ordering,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use windows::{
    Win32::{
        Foundation::E_ABORT,
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
            CoCreateInstance, CoInitializeEx, CoUninitialize,
        },
        UI::Shell::{
            self, IFileOperation, IFileOperationProgressSink, IFileOperationProgressSink_Impl,
            IShellItem,
        },
    },
    core::{Error, HRESULT, PCWSTR, Ref, implement},
};

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

pub fn image_siblings(path: &Path) -> Result<Vec<PathBuf>> {
    let path = std::path::absolute(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Image folder unavailable"))?;
    let mut files = Vec::new();
    for entry in std::fs::read_dir(parent)? {
        let entry = entry?;
        if entry.file_type()?.is_file() && crate::domain::media::is_image_path(&entry.path()) {
            files.push(entry.path());
        }
    }
    files.sort_by(|a, b| {
        let a_name = wide(Path::new(a.file_name().unwrap()));
        let b_name = wide(Path::new(b.file_name().unwrap()));
        let order =
            unsafe { Shell::StrCmpLogicalW(PCWSTR(a_name.as_ptr()), PCWSTR(b_name.as_ptr())) };
        match order.cmp(&0) {
            Ordering::Equal => a.cmp(b),
            order => order,
        }
    });
    Ok(files)
}

pub fn adjacent_image(path: &Path, next: bool) -> Result<Option<PathBuf>> {
    let path = std::path::absolute(path)?;
    let files = image_siblings(&path)?;
    let Some(index) = files.iter().position(|file| file == &path) else {
        bail!("Current image no longer exists");
    };
    Ok(if next {
        files.get(index + 1)
    } else {
        index.checked_sub(1).and_then(|index| files.get(index))
    }
    .cloned())
}

pub fn image_after_delete(path: &Path) -> Result<Option<PathBuf>> {
    Ok(adjacent_image(path, true)?.or(adjacent_image(path, false)?))
}

fn recycle_guard(flags: u32) -> windows::core::Result<()> {
    if flags & Shell::TSF_DELETE_RECYCLE_IF_POSSIBLE.0 as u32 == 0 {
        return Err(Error::new(
            E_ABORT,
            "Recycle Bin is unavailable; permanent deletion is disabled",
        ));
    }
    Ok(())
}

#[implement(IFileOperationProgressSink)]
struct RecycleOnly {
    result: Arc<Mutex<Option<HRESULT>>>,
}

#[allow(non_snake_case)]
impl IFileOperationProgressSink_Impl for RecycleOnly_Impl {
    fn PreDeleteItem(&self, flags: u32, _item: Ref<'_, IShellItem>) -> windows::core::Result<()> {
        recycle_guard(flags)
    }
    fn PostDeleteItem(
        &self,
        _flags: u32,
        _item: Ref<'_, IShellItem>,
        result: HRESULT,
        _created: Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        *self.result.lock().unwrap() = Some(result);
        result.ok()
    }
    fn StartOperations(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn FinishOperations(&self, result: HRESULT) -> windows::core::Result<()> {
        result.ok()
    }
    fn PreRenameItem(
        &self,
        _flags: u32,
        _item: Ref<'_, IShellItem>,
        _name: &PCWSTR,
    ) -> windows::core::Result<()> {
        Err(E_ABORT.into())
    }
    fn PostRenameItem(
        &self,
        _flags: u32,
        _item: Ref<'_, IShellItem>,
        _name: &PCWSTR,
        _hr: HRESULT,
        _created: Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreMoveItem(
        &self,
        _flags: u32,
        _item: Ref<'_, IShellItem>,
        _dest: Ref<'_, IShellItem>,
        _name: &PCWSTR,
    ) -> windows::core::Result<()> {
        Err(E_ABORT.into())
    }
    fn PostMoveItem(
        &self,
        _flags: u32,
        _item: Ref<'_, IShellItem>,
        _dest: Ref<'_, IShellItem>,
        _name: &PCWSTR,
        _hr: HRESULT,
        _created: Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreCopyItem(
        &self,
        _flags: u32,
        _item: Ref<'_, IShellItem>,
        _dest: Ref<'_, IShellItem>,
        _name: &PCWSTR,
    ) -> windows::core::Result<()> {
        Err(E_ABORT.into())
    }
    fn PostCopyItem(
        &self,
        _flags: u32,
        _item: Ref<'_, IShellItem>,
        _dest: Ref<'_, IShellItem>,
        _name: &PCWSTR,
        _hr: HRESULT,
        _created: Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreNewItem(
        &self,
        _flags: u32,
        _dest: Ref<'_, IShellItem>,
        _name: &PCWSTR,
    ) -> windows::core::Result<()> {
        Err(E_ABORT.into())
    }
    fn PostNewItem(
        &self,
        _flags: u32,
        _dest: Ref<'_, IShellItem>,
        _name: &PCWSTR,
        _template: &PCWSTR,
        _attributes: u32,
        _hr: HRESULT,
        _created: Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn UpdateProgress(&self, _total: u32, _done: u32) -> windows::core::Result<()> {
        Ok(())
    }
    fn ResetTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn PauseTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn ResumeTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
}

/// Recycle only a single regular image. Never fall back to permanent deletion.
pub fn recycle_image(path: &Path) -> Result<()> {
    let path = std::path::absolute(path)?;
    ensure!(
        crate::domain::media::is_image_path(&path),
        "Only still images can be deleted"
    );
    ensure!(
        std::fs::symlink_metadata(&path)?.file_type().is_file(),
        "Only regular image files can be deleted"
    );
    struct ComGuard;
    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok()?;
        let _com = ComGuard;
        let operation: IFileOperation =
            CoCreateInstance(&Shell::FileOperation, None, CLSCTX_INPROC_SERVER)?;
        operation.SetOperationFlags(
            Shell::FOFX_RECYCLEONDELETE
                | Shell::FOFX_ADDUNDORECORD
                | Shell::FOFX_EARLYFAILURE
                | Shell::FOF_NOERRORUI
                | Shell::FOF_SILENT
                | Shell::FOF_NOCONFIRMATION
                | Shell::FOF_NO_CONNECTED_ELEMENTS
                | Shell::FOF_NORECURSION,
        )?;
        let encoded = wide(&path);
        let item: IShellItem = Shell::SHCreateItemFromParsingName(PCWSTR(encoded.as_ptr()), None)?;
        let result = Arc::new(Mutex::new(None));
        let sink: IFileOperationProgressSink = RecycleOnly {
            result: result.clone(),
        }
        .into();
        operation.DeleteItem(&item, &sink)?;
        operation.PerformOperations()?;
        ensure!(
            !operation.GetAnyOperationsAborted()?.as_bool(),
            "Could not move image to Recycle Bin"
        );
        let result = result
            .lock()
            .unwrap()
            .ok_or_else(|| anyhow::anyhow!("Recycle operation did not complete"))?;
        result.ok()?;
        ensure!(!path.try_exists()?, "Image was not moved to Recycle Bin");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn permanent_delete_is_vetoed() {
        assert!(recycle_guard(0).is_err());
        assert!(recycle_guard(Shell::TSF_DELETE_RECYCLE_IF_POSSIBLE.0 as u32).is_ok());
    }
    #[test]
    fn navigation_is_natural_image_only_and_does_not_wrap() {
        let folder = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("target/validation/order-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        for name in ["photo10.jpg", "photo2.jpg", "photo1.jpg", "movie.mp4"] {
            std::fs::write(folder.join(name), b"synthetic ordering fixture").unwrap();
        }
        assert_eq!(
            adjacent_image(&folder.join("photo1.jpg"), true).unwrap(),
            Some(folder.join("photo2.jpg"))
        );
        assert_eq!(
            adjacent_image(&folder.join("photo10.jpg"), false).unwrap(),
            Some(folder.join("photo2.jpg"))
        );
        assert!(
            adjacent_image(&folder.join("photo1.jpg"), false)
                .unwrap()
                .is_none()
        );
        assert!(
            adjacent_image(&folder.join("photo10.jpg"), true)
                .unwrap()
                .is_none()
        );
    }
}
