use simplelog::*;
use std::fs::File;

pub fn init(path: &str) -> std::io::Result<()> {
    let file = File::create(path)?;

    WriteLogger::init(
        LevelFilter::Debug,
        Config::default(),
        file,
    ).map_err(std::io::Error::other)?;

    Ok(())
}
