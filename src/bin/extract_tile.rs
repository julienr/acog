use acog::ppm::write_to_ppm;
use acog::tiler::{extract_tile, TMSTileCoords};
use acog::Error;
use std::env;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    if args.len() <= 2 {
        println!("Usage: <filename> <z> <x> <y>");
        return Err(Error::InvalidData(
            "Missing commandline argument".to_string(),
        ));
    }

    let filename = &args[1];
    // Those are XYZ tile coords
    let z = args[2].parse::<u32>().unwrap();
    let x = args[3].parse::<u64>().unwrap();
    let y = args[4].parse::<u64>().unwrap();

    let mut cog = acog::COG::open(filename).await?;
    let tile = extract_tile(&mut cog, TMSTileCoords::from_zxy(z, x, y)).await?;
    write_to_ppm("img.ppm", &tile.img)?;
    println!("Stats: {}", cog.get_stats());
    Ok(())
}
