use acog::npy::write_to_npy;
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
    let tile_data = extract_tile(&mut cog, TMSTileCoords::from_xyz(x, y, z)).await?;
    write_to_npy(
        "img.npy",
        tile_data,
        // TODO: Remove hardcoded tile size
        [256, 256, 3],
    )?;
    Ok(())
}
