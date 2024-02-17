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
    let mut reader = acog::open(filename).await?;
    println!("reader: {:?}", reader);
    let ifd_reader = reader.ifds[0].make_reader(&mut reader.source).await?;
    // TODO: Read last ifd => on the marina COG, this is the transparency mask (PhotometricInterp = 4)
    //let ifd_reader = reader.ifds.iter().last().unwrap().make_reader()?;
    println!("reader: {:?}", ifd_reader);
    let img_data = ifd_reader.read_image(&mut reader.source).await?;
    println!("img_data.len: {:?}", img_data.len());
    write_to_npy(
        "img.npy",
        img_data,
        [ifd_reader.height, ifd_reader.width, ifd_reader.nbands],
    )?;
    Ok(())
}
