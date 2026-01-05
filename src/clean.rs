use std::error::Error;

use url::Url;

// Error type for clean URL function
#[derive(Debug, derive_more::Display)]
pub enum CleanUrlError {
    ParseError(url::ParseError),
    PathSegmentsError,
    UnknownDomain,
    UnsupportedUrlScheme,
    UnsupportedUrlHost,
    UnsupportedUrlPath,
}

impl Error for CleanUrlError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CleanUrlError::ParseError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<url::ParseError> for CleanUrlError {
    fn from(e: url::ParseError) -> Self {
        CleanUrlError::ParseError(e)
    }
}

// Clean URL
pub fn clean_url(url: &str) -> Result<String, CleanUrlError> {
    // Step 0: parse URL
    let mut url = Url::parse(url)?;

    // Step 1: confirm the scheme is http/https
    let scheme = url.scheme();
    if scheme != "https" && scheme != "http" {
        return Err(CleanUrlError::UnsupportedUrlScheme);
    }

    // Step 2: verify host/domain is Reddit
    let host = url.host_str().expect("url host is valid");
    match psl::domain(host.as_bytes()) {
        Some(domain) => {
            if domain != "reddit.com" {
                return Err(CleanUrlError::UnsupportedUrlHost);
            }
        }
        _ => return Err(CleanUrlError::UnknownDomain),
    }

    // Step 3: remove query parameters
    url.set_query(None);

    // Step 4: remove trailing slash if any (provides no information)
    url.path_segments_mut()
        .map_err(|_| CleanUrlError::PathSegmentsError)?
        .pop_if_empty(); // remove trailing slash if present

    // Step 5: possibly remove trailing path (additional post information)
    let segments: Vec<_> = url
        .path_segments()
        .ok_or(CleanUrlError::PathSegmentsError)?
        .collect();

    // https://www.reddit.com/r/<sub>/comments/<post_id>/<post_short_name> (optional short name)
    let is_post_with_short_name = matches!(segments.as_slice(), ["r", _, "comments", _, _]);
    let is_post = is_post_with_short_name || matches!(segments.as_slice(), ["r", _, "comments", _]);

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

    // Final step: return as String; URL has now been cleaned
    Ok(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    mod reddit {
        use super::*;

        mod posts {
            use super::*;

            #[test]
            fn test_trivial() {
                let url = "https://www.reddit.com/r/AskTheWorld/comments/1q2rw7m";
                let result = clean_url(url);
                assert!(result.is_ok());
                assert_eq!(url, result.expect("gcleaned"));
            }

            #[test]
            fn test_basic() {
                let url = "https://www.reddit.com/r/AskTheWorld/comments/1q2rw7m/what_comes_to_mind_when_you_think_of_new_zealand/?share_id=l2suzjz-JpaaqZSjbaNmt&utm_content=1&utm_medium=ios_app&utm_name=ioscss&utm_source=share&utm_term=1";
                let result = clean_url(url);
                assert!(result.is_ok());
                let expected = "https://www.reddit.com/r/AskTheWorld/comments/1q2rw7m";
                assert_eq!(expected, result.expect("cleaned"))
            }
        }

        mod comments {
            use super::*;

            #[test]
            fn test_trivial() {
                let url = "https://www.reddit.com/r/AskTheWorld/comments/1q2rw7m/comment/nxfc5ci";
                let result = clean_url(url);
                assert!(result.is_ok());
                assert_eq!(url, result.expect("cleaned"));
            }

            #[test]
            fn test_basic() {
                let url = "https://www.reddit.com/r/AskTheWorld/comments/1q2rw7m/comment/nxfc5ci/?context=3&share_id=8ws3zlfg6lxtYbyGrudio&utm_content=1&utm_medium=ios_app&utm_name=ioscss&utm_source=share&utm_term=1";
                let result = clean_url(url);
                assert!(result.is_ok());
                let expected =
                    "https://www.reddit.com/r/AskTheWorld/comments/1q2rw7m/comment/nxfc5ci";
                assert_eq!(expected, result.expect("cleaned"))
            }
        }
    }

    mod errors {
        use super::*;

        #[test]
        fn test_invalid_url() {
            assert!(matches!(
                clean_url("not a valid url"),
                Err(CleanUrlError::ParseError(_))
            ));
        }

        #[test]
        fn test_empty_host() {
            assert!(matches!(
                clean_url("https://"),
                Err(CleanUrlError::ParseError(url::ParseError::EmptyHost))
            ));
        }

        #[test]
        fn test_unknown_domain() {
            assert!(matches!(
                clean_url("https:///path/to/file"),
                Err(CleanUrlError::UnknownDomain)
            ));
        }

        #[test]
        fn test_unsupported_scheme() {
            assert!(matches!(
                clean_url("fpt://www.reddit.com/"),
                Err(CleanUrlError::UnsupportedUrlScheme)
            ));
        }

        #[test]
        fn test_unsupported_host() {
            assert!(matches!(
                clean_url("https://example.com/"),
                Err(CleanUrlError::UnsupportedUrlHost)
            ));
        }

        #[test]
        fn test_unsupported_path() {
            assert!(matches!(
                clean_url("https://reddit.com"),
                Err(CleanUrlError::UnsupportedUrlPath)
            ));
            assert!(matches!(
                clean_url("https://reddit.com/"),
                Err(CleanUrlError::UnsupportedUrlPath)
            ));
            assert!(matches!(
                clean_url("http:reddit.com/"),
                Err(CleanUrlError::UnsupportedUrlPath)
            ));
            assert!(matches!(
                clean_url("https://reddit.com/u/spez"),
                Err(CleanUrlError::UnsupportedUrlPath)
            ));
        }

        #[test]
        #[ignore]
        fn test_path_segments_error() {
            // I believe, in practice, `PathSegmentsError` is unreachable due to
            // URL validation during parsing.
        }
    }
}
