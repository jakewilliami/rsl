use super::{CleanUrlError, UrlCleaner};
use url::Url;

pub struct RedditCleaner;

impl UrlCleaner for RedditCleaner {
    fn clean(&self, url: &mut Url) -> Result<(), CleanUrlError> {
        // Step 1: verify host/domain is Reddit
        let host = url.host_str().expect("url host is valid");
        match psl::domain(host.as_bytes()) {
            Some(domain) => {
                if domain != "reddit.com" {
                    return Err(CleanUrlError::UnsupportedUrlHost);
                }
            }
            _ => return Err(CleanUrlError::UnknownDomain),
        }

        // Step 2: remove query parameters
        url.set_query(None);

        // Step 3: remove trailing slash if any (provides no information)
        url.path_segments_mut()
            .map_err(|_| CleanUrlError::PathSegmentsError)?
            .pop_if_empty(); // remove trailing slash if present

        // Step 4: possibly remove trailing path (additional post information)
        let segments: Vec<_> = url
            .path_segments()
            .ok_or(CleanUrlError::PathSegmentsError)?
            .collect();

        // https://www.reddit.com/r/<sub>/comments/<post_id>/<post_short_name> (optional short name)
        let is_post_with_short_name = matches!(segments.as_slice(), ["r", _, "comments", _, _]);
        let is_post =
            is_post_with_short_name || matches!(segments.as_slice(), ["r", _, "comments", _]);

        // https://www.reddit.com/r/<sub>/comments/<post_id>/comment/<comment_id>
        let is_comment = matches!(segments.as_slice(), ["r", _, "comments", _, "comment", _]);

        if !is_post && !is_comment {
            return Err(CleanUrlError::UnsupportedUrlPath);
        }

        // If the URL is a post, remove its short name (final segment)
        if is_post_with_short_name {
            url.path_segments_mut()
                .map_err(|_| CleanUrlError::PathSegmentsError)?
                .pop();
        }

        Ok(())
    }
}
