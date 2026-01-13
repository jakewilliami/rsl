use super::{CleanUrlError, UrlCleaner};
use url::Url;

pub struct InstagramCleaner;

impl UrlCleaner for InstagramCleaner {
    fn clean(&self, url: &mut Url) -> Result<(), CleanUrlError> {
        // Step 1: remove query parameters
        //
        // Importantly, we remove tracking information from the igsh query parameter
        url.set_query(None);

        // Step 2: remove trailing slash if any (provides no information)
        url.path_segments_mut()
            .map_err(|_| CleanUrlError::PathSegmentsError)?
            .pop_if_empty();

        Ok(())
    }
}
