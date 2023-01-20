
use anyhow::{Result, Context};

pub fn read_to_vec<P: AsRef<std::path::Path>>(path: P) -> Result<Vec<u8>> {
    let mut file = std::fs::File::open(path.as_ref())
    .with_context(||format!("fail to open file [{}]", path.as_ref().display()))?;

    let mut data = Vec::new();
    std::io::Read::read_to_end(&mut file, &mut data)
    .with_context(||format!("fail to read file [{}]", path.as_ref().display()))?;
 
    // println!("opened file [{}]", path.as_ref().display());
    Ok(data)
}
