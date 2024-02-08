use acog::npy::write_to_npy;
use acog::{Error, TIFFReader};
use std::env;
use std::fs::File;
use std::io::prelude::*;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    if args.len() <= 1 {
        println!("Usage: <filename>");
        return Err(Error::InvalidData(
            "Missing commandline argument".to_string(),
        ));
    }

    let filename = &args[1];
    let mut reader = TIFFReader::open(filename).await?;
    let ifd_reader = reader.ifds[0].make_reader()?;
    let mut file = File::create("out.json")?;
    file.write_all(&serde_json::to_string(&reader).unwrap().into_bytes())?;
    Ok(())
}
