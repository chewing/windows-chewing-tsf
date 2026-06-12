use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Seek;
use std::path::Path;

use scoped_error::Error;
use scoped_error::expect_error_fn;
use zip::ZipArchive;

/// Extracts a ZIP file to `dest` without preserving directory structure.
pub(crate) fn unzip<R>(dest: &Path, input: R) -> Result<(), Error>
where
    R: Read + Seek,
{
    let err = || Error::new(format!("Unable to unzip file to {}", dest.display()));
    expect_error_fn(err, || {
        let mut archive = ZipArchive::new(input)?;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            if file.is_dir() {
                continue;
            }
            let Some(name) = file.enclosed_name() else {
                return Err(format!(
                    "unable to extract file {:?} because it has an invalid path.",
                    file.name()
                )
                .into());
            };
            let out_path = dest.join(&name.file_name().unwrap());
            eprintln!("Extracting {} as {}", name.display(), out_path.display());
            let mut out_file = File::create(out_path)?;
            io::copy(&mut file, &mut out_file)?;
        }
        Ok(())
    })
}
