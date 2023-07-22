use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Seek};
use std::path::Path;

use flate2::read::GzDecoder;
use tar::{Archive as TarArchive, Entry as TarEntry};
use zip::ZipArchive;

use crate::{ErrorReason, Result, ResultExt};

pub enum Archive {
    TarGz(Box<TarArchive<GzDecoder<BufReader<File>>>>),
    Zip(ZipArchive<BufReader<File>>),
    None(File),
}

impl Archive {
    pub fn new_tar_gz(file: File) -> Self {
        Self::TarGz(Box::new(TarArchive::new(GzDecoder::new(BufReader::new(
            file,
        )))))
    }

    pub fn new_zip(file: File) -> Result<Self> {
        Ok(Self::Zip(
            ZipArchive::new(BufReader::new(file)).reason(ErrorReason::ArchiveOther)?,
        ))
    }

    pub fn new_none(file: File) -> Self {
        Self::None(file)
    }

    pub fn extract_file(&mut self, file: &str, target: &Path) -> Result<()> {
        match self {
            Self::TarGz(archive) => {
                let mut tar_file =
                    find_tar_entry(archive, file)?.reason(ErrorReason::ArchiveFileNotFound)?;
                let mut out_file = extract_file(&mut tar_file, file, target)?;

                if let Ok(mode) = tar_file.header().mode() {
                    set_file_permissions(&mut out_file, mode)?;
                }
            }
            Self::Zip(archive) => {
                let zip_index =
                    find_zip_entry(archive, file)?.reason(ErrorReason::ArchiveFileNotFound)?;
                let mut zip_file = archive
                    .by_index(zip_index)
                    .reason(ErrorReason::ArchiveOther)?;
                let mut out_file = extract_file(&mut zip_file, file, target)?;

                if let Some(mode) = zip_file.unix_mode() {
                    set_file_permissions(&mut out_file, mode)?;
                }
            }
            Self::None(in_file) => {
                let create_dir_result = std::fs::create_dir(target);
                if let Err(e) = &create_dir_result {
                    if e.kind() != std::io::ErrorKind::AlreadyExists {
                        create_dir_result.reason(ErrorReason::ArchiveCopyFailed)?;
                    }
                }

                let mut out_file_path = target.to_path_buf();
                out_file_path.push(file);
                let mut out_file =
                    File::create(out_file_path).reason(ErrorReason::ArchiveCopyFailed)?;
                {
                    let mut reader = BufReader::new(in_file);
                    let mut writer = BufWriter::new(&out_file);

                    std::io::copy(&mut reader, &mut writer)
                        .reason(ErrorReason::ArchiveCopyFailed)?;
                }
                set_file_permissions(&mut out_file, 0o755)?; // rwx for user, rx for group and
                                                             // other.
            }
        }

        Ok(())
    }

    pub fn reset(self) -> Result<Self> {
        match self {
            Self::TarGz(archive) => {
                let mut archive_file = archive.into_inner().into_inner();
                archive_file
                    .rewind()
                    .reason(ErrorReason::ArchiveSeekFailed)?;

                Ok(Self::TarGz(Box::new(TarArchive::new(GzDecoder::new(
                    archive_file,
                )))))
            }
            result @ Self::None(_) | result @ Self::Zip(_) => Ok(result),
        }
    }
}

/// Find an entry in a TAR archive by name and open it for reading. The first part of the path
/// is dropped as that's usually the folder name it was created from.
fn find_tar_entry(
    archive: &mut TarArchive<impl Read>,
    path: impl AsRef<Path>,
) -> Result<Option<TarEntry<impl Read>>> {
    let entries = archive
        .entries()
        .reason(ErrorReason::ArchiveGetEntryFailed)?;
    for entry in entries {
        let entry = entry.reason(ErrorReason::ArchiveGetEntryFailed)?;
        let name = entry.path().reason(ErrorReason::ArchiveGetEntryFailed)?;

        let mut name = name.components();
        name.next();

        if name.as_path() == path.as_ref() {
            return Ok(Some(entry));
        }
    }

    Ok(None)
}

/// Find an entry in a ZIP archive by name and return its index. The first part of the path is
/// dropped as that's usually the folder name it was created from.
fn find_zip_entry(
    archive: &mut ZipArchive<impl Read + Seek>,
    path: impl AsRef<Path>,
) -> Result<Option<usize>> {
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .reason(ErrorReason::ArchiveGetEntryFailed)?;
        let name = entry
            .enclosed_name()
            .reason(ErrorReason::ArchiveGetEntryFailed)?;

        let mut name = name.components();
        name.next();

        if name.as_path() == path.as_ref() {
            return Ok(Some(index));
        }
    }

    Ok(None)
}

fn extract_file(mut read: impl Read, file: &str, target: &Path) -> Result<File> {
    let out = target.join(file);

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).reason(ErrorReason::ArchiveExtractFailed)?;
    }

    let mut out = File::create(target.join(file)).reason(ErrorReason::ArchiveExtractFailed)?;
    io::copy(&mut read, &mut out).reason(ErrorReason::ArchiveExtractFailed)?;

    Ok(out)
}

/// Set the executable flag for a file. Only has an effect on UNIX platforms.
fn set_file_permissions(file: &mut File, mode: u32) -> Result<()> {
    #[cfg(unix)]
    {
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt;

        file.set_permissions(Permissions::from_mode(mode))
            .reason(ErrorReason::ArchiveSetPermissionFailed)?;
    }

    Ok(())
}
