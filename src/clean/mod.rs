// Structure of `clean` submodule inspired by:
//   <https://github.com/jakewilliami/citati/tree/8bb1e472/src/source>

use std::error::Error;

use url::Url;

mod reddit;

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

// Trait for platform-specific URL cleaners
trait UrlCleaner {
    // Clean the URL according to platform-specific rules
    fn clean(&self, url: &mut Url) -> Result<(), CleanUrlError>;
}

// Clean URL
pub fn clean_url(url: &str) -> Result<String, CleanUrlError> {
    // Step 1: parse URL
    let mut url = Url::parse(url)?;

    // Step 2: validate scheme
    let scheme = url.scheme();
    if scheme != "https" && scheme != "http" {
        return Err(CleanUrlError::UnsupportedUrlScheme);
    }

    // Step 3: dispatch to defined URL cleaner based on domain name
    let host = url.host_str().expect("url host is valid");
    let cleaner = match psl::domain_str(host) {
        Some(domain) => match domain {
            "reddit.com" => reddit::RedditCleaner,
            _ => return Err(CleanUrlError::UnsupportedUrlHost),
        },
        _ => return Err(CleanUrlError::UnknownDomain),
    };

    // Final step: apply cleaner and return modified URL
    cleaner.clean(&mut url)?;
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
