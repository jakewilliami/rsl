use super::{CleanUrlError, UrlCleaner};
use std::collections::HashMap;
use url::Url;

pub struct FacebookCleaner;

impl UrlCleaner for FacebookCleaner {
    fn clean(&self, url: &mut Url) -> Result<(), CleanUrlError> {
        // Step 1: store query parameters before removing them
        let params: HashMap<String, String> = url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        url.set_query(None);

        // Step 2: remove trailing slash if any (provides no information)
        url.path_segments_mut()
            .map_err(|_| CleanUrlError::PathSegmentsError)?
            .pop_if_empty(); // remove trailing slash if present

        let segments: Vec<_> = url
            .path_segments()
            .ok_or(CleanUrlError::PathSegmentsError)?
            .collect();

        // Step 3: trivial, early return if the kind of link does not require
        // any query parameters

        // https://www.facebook.com/<page>/posts/<post ID>
        let is_page_post = matches!(segments.as_slice(), [_, "posts", _]);

        // https://www.facebook.com/groups/<group>/permalink/<post ID>
        let is_group_post = matches!(segments.as_slice(), ["groups", _, "permalink", _]);

        // https://www.facebook.com/reel/<post ID>
        let is_reel = matches!(segments.as_slice(), ["reel", _]);

        if is_page_post || is_reel || (is_group_post && !&params.contains_key("comment_id")) {
            return Ok(());
        }

        // Step 4: we need to conditionally add some query parameters

        // Case 4.1: the link is a permalink
        if matches!(segments.as_slice(), ["permalink.php"]) {
            // 4.1 a: the permalink is for a story; we need to add the story ID and
            //   (presumably poster's) ID
            //
            // https://www.facebook.com/permalink.php?story_fbid=<story ID>&id=<id>
            if let Some(story_fbid) = params.get("story_fbid") {
                url.query_pairs_mut()
                    .append_pair("story_fbid", story_fbid)
                    .append_pair("id", params.get("id").expect("must have id"));
                return Ok(());
            }
            unreachable!()
        } else if matches!(segments.as_slice(), ["photo.php"]) {
            // 4.1 b: the permalink is for a photo; we need to add its ID back
            //
            // https://www.facebook.com/photo.php?fbid=<photo ID>
            url.query_pairs_mut()
                .append_pair("fbid", params.get("fbid").expect("must have fbid"));
            return Ok(());
        }

        // Case 4.2: the link is a group post permalink containing a comment
        //
        // https://www.facebook.com/groups/<group>/permalink/25654608820855518/?comment_id=<commend ID>
        if is_group_post && params.contains_key("comment_id") {
            url.query_pairs_mut().append_pair(
                "comment_id",
                params.get("comment_id").expect("100% Rust bug"),
            );
            return Ok(());
        }

        // Probably unreachable?
        unreachable!("if we got here then there was a case for Facebook that wasn't handled");
    }
}
