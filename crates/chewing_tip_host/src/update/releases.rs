use roxmltree::Document;

#[derive(Debug)]
pub(crate) struct Release {
    pub(crate) version: String,
    pub(crate) channel: String,
    pub(crate) url: String,
}

const RELEASES: &str = "https://chewing.im/releases/im.chewing.windows_chewing_tsf.releases.xml";

pub(crate) fn fetch_releases() -> Result<Vec<Release>, FetchReleasesError> {
    let releases_xml = ureq::get(RELEASES).call()?.body_mut().read_to_string()?;
    let doc = Document::parse(&releases_xml)?;
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

#[derive(Debug, thiserror::Error)]
#[error("Failed to download release metadata")]
pub(crate) enum FetchReleasesError {
    Network(#[from] ureq::Error),
    ParseXml(#[from] roxmltree::Error),
}
