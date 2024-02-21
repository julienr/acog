use acog::npy::write_to_npy;
use acog::Error;
use std::env;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    if args.len() <= 2 {
        println!("Usage: <filename> <overview:int>");
        return Err(Error::InvalidData(
            "Missing commandline argument".to_string(),
        ));
    }

    let filename = &args[1];
    let overview_index = args[2].parse::<usize>().unwrap();
    let mut cog = acog::open(filename).await?;
    println!(
        "cog width={}, height={}, nbands={}, overviews={}",
        cog.width(),
        cog.height(),
        cog.nbands(),
        cog.overviews.len()
    );
    for i in 0..cog.overviews.len() {
        let overview = &cog.overviews[i];
        println!(
            "overview i={}, width={}, height{}, tile_width={}, tile_height={}",
            i, overview.width, overview.height, overview.tile_width, overview.tile_height
        );
    }
    let overview = &cog.overviews[overview_index];
    let img_data = overview
        .make_reader(&mut cog.source)
        .await?
        .read_image(&mut cog.source)
        .await?;
    write_to_npy(
        "img.npy",
        img_data,
        [
            overview.height as usize,
            overview.width as usize,
            overview.nbands as usize,
        ],
    )?;
    Ok(())
}
