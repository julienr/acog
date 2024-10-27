use acog::Error;
use std::env;

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
    let cog = acog::open(filename).await?;
    println!(
        "cog width={}, height={}, nbands={}, overviews={}",
        cog.width(),
        cog.height(),
        cog.visual_bands_count(),
        cog.overviews.len()
    );
    for i in 0..cog.overviews.len() {
        let overview = &cog.overviews[i];
        println!(
            "overview i={}, width={}, height{}, tile_width={}, tile_height={}",
            i, overview.width, overview.height, overview.tile_width, overview.tile_height
        );
    }
    println!("geo_key_directory: {:?}", cog.geo_keys);
    println!("georeference: {:?}", cog.georeference);
    Ok(())
}
