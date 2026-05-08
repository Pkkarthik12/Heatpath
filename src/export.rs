use std::fs::File;
use std::path::Path;

use anyhow::Result;

use crate::scoring::FileHeat;

pub fn print_json(files: &[FileHeat]) -> Result<()> {
    serde_json::to_writer_pretty(std::io::stdout(), files)?;
    println!();
    Ok(())
}

pub fn print_csv(files: &[FileHeat]) -> Result<()> {
    let mut writer = csv::Writer::from_writer(std::io::stdout());
    for file in files {
        writer.serialize(file)?;
    }
    writer.flush()?;
    Ok(())
}

pub fn write_json_file(path: &Path, files: &[FileHeat]) -> Result<()> {
    let file = File::create(path)?;
    serde_json::to_writer_pretty(file, files)?;
    Ok(())
}
