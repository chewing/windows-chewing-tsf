use exn::{Result, ResultExt};
use roxmltree::Document;

use super::UpdateError;

#[derive(Debug)]
pub(crate) struct Release {
    pub(crate) version: String,
    pub(crate) channel: String,
    pub(crate) url: String,
}

const RELEASES: &str = "https://chewing.im/releases/im.chewing.windows_chewing_tsf.releases.xml";

pub(crate) fn fetch_releases() -> Result<Vec<Release>, UpdateError> {
    let err = || UpdateError("Failed to download release metadata");
    let releases_xml = ureq::get(RELEASES)
        .call()
        .or_raise(err)?
        .body_mut()
        .read_to_string()
        .or_raise(err)?;
    let doc = Document::parse(&releases_xml).or_raise(err)?;
    let mut ret = vec![];
    for rel in doc.root_element().children() {
        if rel.has_tag_name("release") && rel.has_attribute("version") && rel.has_attribute("type")
        {
            let url = rel
                .children()
                .filter(|n| n.has_tag_name("url"))
                .flat_map(|n| n.text())
                .next()
                .unwrap_or("")
                .trim();
            ret.push(Release {
                version: rel.attribute("version").unwrap().to_string(),
                channel: rel.attribute("type").unwrap().to_string(),
                url: url.to_string(),
            })
        }
    }
    Ok(ret)
}
