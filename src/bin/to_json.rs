use acog::Error;
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
    let mut reader = acog::open(filename).await?;

    // Decode all entries of all IFDs to dump them
    let fully_decoded_ifds = reader.fully_read_ifds().await?;

    // Dump to JSON for inspection
    let mut file = File::create(output_filename)?;
    file.write_all(
        &serde_json::to_string(&fully_decoded_ifds)
            .unwrap()
            .into_bytes(),
    )?;

    reader.print_cache_stats();
    Ok(())
}
