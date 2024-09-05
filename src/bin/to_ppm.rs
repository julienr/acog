use acog::image::ImageBuffer;
use acog::ppm::write_to_ppm;
use acog::{Error, ImageRect};
use std::env;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    if args.len() <= 2 || args.len() > 3 && args.len() != 7 {
        println!("Usage: <filename> <overview:int> [i_from] [j_from] [i_to] [j_to]");
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

    let rect = if args.len() > 3 {
        ImageRect {
            i_from: args[3].parse::<u64>().unwrap(),
            j_from: args[4].parse::<u64>().unwrap(),
            i_to: args[5].parse::<u64>().unwrap(),
            j_to: args[6].parse::<u64>().unwrap(),
        }
    } else {
        ImageRect {
            i_from: 0,
            j_from: 0,
            i_to: overview.height,
            j_to: overview.width,
        }
    };
    println!("Extracting rect={:?}", rect);

    let img_data = overview
        .make_reader(&mut cog.source)
        .await?
        .read_image_part(&mut cog.source, &rect)
        .await?;
    write_to_ppm(
        "img.ppm",
        &ImageBuffer {
            data: img_data,
            width: rect.width() as usize,
            height: rect.height() as usize,
            nbands: overview.nbands as usize,
        },
    )?;
    Ok(())
}
