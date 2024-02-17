use acog::npy::write_to_npy;
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
    let mut cog = acog::open(filename).await?;
    // TODO: Read all overview and dump them to a npz
    println!("reader: {:?}", cog);
    let img_data = cog.overviews[0]
        .ifd
        .make_reader(&mut cog.source)
        .await?
        .read_image(&mut cog.source)
        .await?;
    println!("img_data.len: {:?}", img_data.len());
    write_to_npy(
        "img.npy",
        img_data,
        [
            cog.height() as usize,
            cog.width() as usize,
            cog.nbands() as usize,
        ],
    )?;
    Ok(())
}
