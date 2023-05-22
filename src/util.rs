use std::{error::Error, fs, path::PathBuf};

fn get_filepaths(
    infile: Option<PathBuf>,
    files: Option<Vec<PathBuf>>,
) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut filepaths = Vec::new();

    if let Some(infile) = infile {
        let paths = fs::read_to_string(infile)?;
        for path in paths.lines() {
            filepaths.push(PathBuf::from(path));
        }
    }

    if let Some(files) = files {
        for file in files {
            filepaths.push(file);
        }
    }

    Ok(filepaths)
}
