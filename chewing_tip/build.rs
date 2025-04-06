use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    embed_resource::compile("ChewingTextService.rc", embed_resource::NONE).manifest_required()?;
    Ok(())
}
