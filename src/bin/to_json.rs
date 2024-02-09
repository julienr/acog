use acog::{Error, TIFFReader};
use std::env;
use std::fs::File;
use std::io::prelude::*;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    if args.len() <= 2 {
        println!("Usage: <filename> <out json>");
        return Err(Error::InvalidData(
            "Missing commandline argument".to_string(),
        ));
    }

    let filename = &args[1];
    let output_filename = &args[2];
    let reader = TIFFReader::open(filename).await?;
    let mut file = File::create(output_filename)?;
    file.write_all(&serde_json::to_string(&reader).unwrap().into_bytes())?;
    Ok(())
}
