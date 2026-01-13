use std::{error::Error, pin::Pin};

use backon::{ExponentialBuilder, Retryable};
use reqwest::header::{self, HeaderValue};
use ua_generator::ua;

type BoxError = Box<dyn Error>;
type ResolveOutput = Result<String, BoxError>;
type ResolveFuture = Pin<Box<dyn Future<Output = ResolveOutput> + Send>>;

// Resolve a URL to its final form.  This includes HTTP _and_ JS redirects; the latter
// handled by `extract_meta_refresh`
pub async fn resolve(url: &str) -> ResolveOutput {
    // This may not be strictly needed,* but to increase robustness of the core
    // resolver function, we implement expontentail backoff.
    //
    // The best API I could find from some quick research was in this project:
    //   <https://github.com/ihrwein/backoff>
    //
    // The alternatives I found from a Google search were:
    //   <https://github.com/jimmycuadra/retry>
    //   <https://github.com/yoshuawuyts/exponential-backoff>
    //
    // But the APIs were clunky and the packages immature.  Unfortunately,
    // the backoff library was abandomed, but I found a replacement that didn't
    // come up in my Google search (which shows how obsolete traditional search engines
    // are, as LLMs would understand the intent of what I was asking, not just searching
    // literally for Rust crates called "backoff"):
    //   <https://github.com/Xuanwo/backon>
    //   <https://github.com/ihrwein/backoff/issues/66>
    //
    // This backon crate implements ExponentialBackoff, which we build with default
    // parameters.  We default to three retries before exiting:
    //   <https://docs.rs/backon/latest/backon/struct.ExponentialBuilder.html>
    //
    // I only added this when implementing support for Facebook, while I was trying to
    // debug an issue where some of my test cases were failing non-deterministically.  I
    // assumed this was due to hitting some 429 response, so I implemented exponential
    // backoff.  Turns out it was the ransomiser picking the user agents selecting mobile
    // user agents, and then Facebook responding with a mobile URL!
    (|| async { resolve_helper(url.to_string(), 0).await })
        .retry(ExponentialBuilder::default())
        .when(|e| e.to_string() == "retryable")
        .await
}

fn resolve_helper(url: String, depth: u32) -> ResolveFuture {
    Box::pin(async move {
        let url = url.as_str();
        if depth > 5 {
            return Err("Too many meta refresh redirects".into());
        }

        // Create a client that follows redirects and mimics a real browser
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(20))
            .user_agent({
                // We generate a random user agent in the interest of privacy.  The best crate
                // for doing this I found from brief research was:
                //   <https://github.com/spider-rs/ua_generator>
                //
                // However, there are some other contenstants:
                //   <https://github.com/Vrajs16/fake_user_agent>
                //   <https://github.com/TrixSec/rand_agents>
                //
                // They all seem to be imitating this mature Python library which does the
                // same:
                //   <https://github.com/fake-useragent/fake-useragent>
                //
                // NOTE: we need a non-mobile user agent, as some servers will add unwanted
                // subdomains into the URL if requesting from a mobile device.  As such, we
                // generate a user agent with the `Desktop` `FormFactor`.
                //   <github.com/spider-rs/ua_generator/blob/57cb3019/ua_generator/src/ua.rs#L312C8-L317>
                //   <https://docs.rs/ua_generator/latest/ua_generator/ua/fn.spoof_by.html>
                //
                // TODO: it is not yet possible to generate a Desktop-only user agent, so
                //   we use Chrome for now.  See spider-rs/ua_generator#7:
                //   <https://github.com/spider-rs/ua_generator/issues/7>
                //
                // ua::spoof_by(
                //     None,                          // OS
                //     Some(ua::FormFactor::Desktop), // Form factor
                //     None,                          // Browser
                //     None,                          // RNG
                // )
                ua::spoof_chrome_ua()
            })
            .default_headers({
                // We must specify some headers to convince Facebook that we are real.
                //
                // We seem to be able to use the deault headers, as long as we specify
                // Accept, Sec-Fetch-Mode, and Cache-Control.  It seems that Accept-Language,
                // Accept-Encoding, DNT, Connection, Upgrade-Insecure-Requests,
                // Sec-Fetch-Dest, and Sec-Fetch-Site are not required.
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    header::ACCEPT,
                    concat!(
                        "text/html,",
                        "application/xhtml+xml,application/xml;",
                        "q=0.9,image/webp,*/*;q=0.8",
                    )
                    .parse()
                    .unwrap(),
                );
                headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("max-age=0"));
                headers.insert("Sec-Fetch-Mode", HeaderValue::from_static("navigate"));
                headers
            })
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        // Make the request
        let response = client.get(url).send().await?;

        // Get the final URL after all redirects
        let final_url = response.url().clone();

        // Check for meta refresh redirects in the HTML; we may need to follow a redirect
        let html: String = response.text().await?;
        if let Some(meta_url) = extract_meta_refresh(&html) {
            // Handle relative URLs
            let meta_url = if meta_url.starts_with("http") {
                meta_url
            } else {
                final_url.join(&meta_url)?.to_string()
            };

            // Follow the meta refresh recursively
            return resolve_helper(meta_url, depth + 1).await;
        }

        Ok(final_url.to_string())
    })
}

// Extract URL from meta refresh tags like:
// <meta http-equiv="refresh" content="0;url=https://example.com">
// TODO: what about window.href being set?  Is that ever used?
fn extract_meta_refresh(html: &str) -> Option<String> {
    let html_lower = html.to_lowercase();

    // Find meta refresh tag
    if let Some(start) = html_lower.find(r#"<meta"#)
        && let Some(end) = html_lower[start..].find('>')
    {
        let meta_tag = &html[start..start + end];

        // Check if it's a refresh meta tag
        if meta_tag.to_lowercase().contains("http-equiv")
            && meta_tag.to_lowercase().contains("refresh")
        {
            // Extract the URL from content attribute
            if let Some(content_start) = meta_tag.to_lowercase().find("content=") {
                let content_part = &meta_tag[content_start + 8..];

                // Handle both quoted and unquoted values
                let quote_char = if content_part.starts_with('"') {
                    '"'
                } else if content_part.starts_with('\'') {
                    '\''
                } else {
                    ' '
                };

                let content_value = if quote_char != ' ' {
                    content_part[1..].split(quote_char).next()?
                } else {
                    content_part.split_whitespace().next()?
                };

                // Extract URL after "url=" or after semicolon
                if let Some(url_start) = content_value.to_lowercase().find("url=") {
                    return Some(content_value[url_start + 4..].trim().to_string());
                } else if let Some(semicolon) = content_value.find(';') {
                    let url_part = content_value[semicolon + 1..].trim();
                    if let Some(stripped) = url_part.strip_prefix("url=") {
                        return Some(stripped.trim().to_string());
                    }
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    mod sources {
        use super::*;

        mod reddit {
            use super::*;

            mod posts {
                use super::*;

                #[tokio::test]
                async fn test_identity() {
                    let url = "https://www.reddit.com/r/AskTheWorld/comments/1q2rw7m/";
                    let result = resolve(url).await;
                    assert!(result.is_ok());
                    assert_eq!(url, result.expect("resolved"));
                }

                #[tokio::test]
                async fn test_share_link() {
                    let url = "https://www.reddit.com/r/AskTheWorld/s/mONZu40JNk";
                    let result = resolve(url).await;
                    assert!(result.is_ok());
                    let expected = "https://www.reddit.com/r/AskTheWorld/comments/1q2rw7m/";
                    assert!(result.expect("resolved").starts_with(expected));
                }
            }

            mod comments {
                use super::*;

                #[tokio::test]
                async fn test_identity() {
                    let url =
                        "https://www.reddit.com/r/AskTheWorld/comments/1q2rw7m/comment/nxfc5ci/";
                    let result = resolve(url).await;
                    assert!(result.is_ok());
                    assert_eq!(url, result.expect("resolved"));
                }

                #[tokio::test]
                async fn test_share_link() {
                    let url = "https://www.reddit.com/r/AskTheWorld/s/5dTzVW0T3w";
                    let result = resolve(url).await;
                    assert!(result.is_ok());
                    let expected = "https://www.reddit.com/r/AskTheWorld/comments/1q2rw7m/";
                    assert!(result.expect("resolved").starts_with(expected));
                }
            }
        }

        mod facebook {
            use super::*;

            mod posts {
                use super::*;

                #[tokio::test]
                async fn test_identity() {
                    let url = "https://www.facebook.com/MoreFMWellington/posts/pfbid0vdnCZ6brToAep5XKfrM7FJBuMcuzsg64y896v4Ce2DJefNKGqe8mYhiJvAZwA5SGl";
                    let result = resolve(url).await;
                    assert!(result.is_ok());
                    assert!(result.expect("resolved").starts_with(url));
                }

                // https://www.facebook.com/share/p/187BayNfDu/ -> https://www.facebook.com/MoreFMWellington/posts/pfbid0vdnCZ6brToAep5XKfrM7FJBuMcuzsg64y896v4Ce2DJefNKGqe8mYhiJvAZwA5SGl?rdid=meEnrHMlBkOtDSrX
                // https://www.facebook.com/share/p/1BgeVKA3em/ -> https://www.facebook.com/rnznewzealand/posts/pfbid0jYRXtR6dGzJeAgLzpYA2ovWwuqtcVRnCMt5TWRmjcwDBPV4yBcNYLr1nwKhKupiPl?rdid=yhfTkLFfmiUYezxF // venezuela
                // https://www.facebook.com/share/p/1BQ1Th6jhw/?mibextid=wwXIfr -> https://www.facebook.com/groups/vicdeals/permalink/25652124024437331/ // elliot
                // https://www.facebook.com/share/p/1JZjfaFSrS/ -> https://www.facebook.com/MCDONewt/posts/pfbid02rgYgaUWkPGcXxMBwgntDtzhYtbmXmCLVrqdguvFS2pnSQRYan1MW5yyfwX1ZqjR1l?rdid=fPrLfmze1NgA6Pqj
                // https://www.facebook.com/share/v/1866jpsdXs/?mibextid=wwXIfr -> https://www.facebook.com/groups/vicdeals/permalink/25658677397115327/
                // https://www.facebook.com/share/1JigZ3TAud/?mibextid=wwXIfr -> https://www.facebook.com/photo.php?fbid=1279617124197361&set=a.301086902050393&type=3

                #[tokio::test]
                async fn test_basic() {
                    // NOTE: MY OWN
                    let url = "https://www.facebook.com/share/v/1866jpsdXs/?mibextid=wwXIfr";
                    let result = resolve(url).await;
                    let expected =
                        "https://www.facebook.com/groups/vicdeals/permalink/25658677397115327/";
                    assert!(result.expect("resolved").starts_with(expected))
                }
            }

            mod story {
                use super::*;

                #[tokio::test]
                async fn test_identity() {
                    let url = "https://www.facebook.com/permalink.php?story_fbid=pfbid02mNMcJYekXP4bnUFkWguBsNddw6GkLHrWZG4ENa23x2h3G2SbbMeJRHByXuxhjKj1l&id=100088004222911";
                    let result = resolve(url).await;
                    assert!(result.is_ok());
                    assert!(result.expect("resolved").starts_with(url));
                }

                #[tokio::test]
                async fn test_basic() {
                    let url = "https://www.facebook.com/share/p/1FKNBd86BV/";
                    let result = resolve(url).await;
                    let expected = "https://www.facebook.com/permalink.php?story_fbid=pfbid02mNMcJYekXP4bnUFkWguBsNddw6GkLHrWZG4ENa23x2h3G2SbbMeJRHByXuxhjKj1l&id=100088004222911";
                    assert!(result.expect("resolved").starts_with(expected))
                }
            }

            mod reels {
                use super::*;

                #[tokio::test]
                async fn test_identity() {
                    let url = "https://www.facebook.com/reel/1309748351194528";
                    let result = resolve(url).await;
                    assert!(result.is_ok());
                    assert!(result.expect("resolved").starts_with(url));
                }

                #[tokio::test]
                async fn test_basic() {
                    let url = "https://www.facebook.com/share/r/14QeSSeP3nu/";
                    let result = resolve(url).await;
                    let expected = "https://www.facebook.com/reel/1309748351194528";
                    assert!(result.expect("resolved").starts_with(expected))
                }
                // https://www.facebook.com/share/r/1AZhvx3n72/ -> https://www.facebook.com/reel/1605919000854039/?rdid=VxhE0u0GlwyGLnFD&share_url=https%3A%2F%2Fwww.facebook.com%2Fshare%2Fr%2F1AZhvx3n72%2F // reel
            }

            mod comments {
                use super::*;

                #[tokio::test]
                async fn test_identity() {
                    let url = "https://www.facebook.com/groups/vicdeals/permalink/25654608820855518/?comment_id=25654673274182406";
                    let result = resolve(url).await;
                    assert!(result.is_ok());
                    assert!(result.expect("resolved").starts_with(url));
                }

                #[tokio::test]
                async fn test_basic() {
                    let url = "https://www.facebook.com/share/17cLnKdQth/";
                    let result = resolve(url).await;
                    let expected = "https://www.facebook.com/groups/vicdeals/permalink/25654608820855518/?comment_id=25654673274182406";
                    assert!(result.expect("resolved").starts_with(expected))
                }
            }

            mod photos {
                use super::*;

                #[tokio::test]
                async fn test_identity() {
                    let url = "https://www.facebook.com/photo.php?fbid=1279617124197361";
                    let result = resolve(url).await;
                    assert!(result.is_ok());
                    assert!(result.expect("resolved").starts_with(url));
                }

                #[tokio::test]
                async fn test_basic() {
                    let url = "https://www.facebook.com/share/1JigZ3TAud/?mibextid=wwXIfr";
                    let result = resolve(url).await;
                    let expected = "https://www.facebook.com/photo.php?fbid=1279617124197361&set=a.301086902050393&type=3";
                    assert!(result.expect("resolved").starts_with(expected))
                }
            }
        }

        mod instagram {
            use super::*;

            #[tokio::test]
            async fn test_identity() {
                let url = "https://www.instagram.com/p/DS8F57NjS_S";
                let result = resolve(url).await;
                assert!(result.is_ok());
                assert!(result.expect("resolved").starts_with(url));
            }

            #[tokio::test]
            async fn test_basic() {
                let url = "https://www.instagram.com/p/DS8F57NjS_S/?igsh=MWxidXNpbWV6djIxcQ==";
                let result = resolve(url).await;
                let expected = "https://www.instagram.com/p/DS8F57NjS_S";
                assert!(result.expect("resolved").starts_with(expected))
            }
        }

        // TODO
        mod linkedin {}
    }

    mod meta_refresh {
        use super::*;

        #[test]
        fn test_basic() {
            let html = r#"<meta http-equiv="refresh" content="0;url=https://example.com">"#;
            let result = extract_meta_refresh(html);
            assert_eq!(result, Some("https://example.com".to_string()));
        }

        #[test]
        fn test_with_delay() {
            let html = r#"<meta http-equiv="refresh" content="5;url=https://example.com/page">"#;
            let result = extract_meta_refresh(html);
            assert_eq!(result, Some("https://example.com/page".to_string()));
        }

        #[test]
        fn test_single_quotes() {
            let html = r#"<meta http-equiv='refresh' content='0;url=https://example.com'>"#;
            let result = extract_meta_refresh(html);
            assert_eq!(result, Some("https://example.com".to_string()));
        }

        #[test]
        fn test_no_quotes() {
            let html = r#"<meta http-equiv=refresh content=0;url=https://example.com>"#;
            let result = extract_meta_refresh(html);
            assert_eq!(result, Some("https://example.com".to_string()));
        }

        #[test]
        fn test_uppercase() {
            let html = r#"<META HTTP-EQUIV="REFRESH" CONTENT="0;URL=https://example.com">"#;
            let result = extract_meta_refresh(html);
            assert_eq!(result, Some("https://example.com".to_string()));
        }

        #[test]
        fn test_mixed_case() {
            let html = r#"<Meta Http-Equiv="Refresh" Content="0;Url=https://example.com">"#;
            let result = extract_meta_refresh(html);
            assert_eq!(result, Some("https://example.com".to_string()));
        }

        // TODO: get working with spaces
        /*#[test]
            fn test_with_spaces() {
                let html = r#"<meta http-equiv="refresh" content="0; url = https://example.com ">"#;
                let result = extract_meta_refresh(html);
                assert_eq!(result, Some("https://example.com".to_string()));
        }*/

        #[test]
        fn test_in_full_html() {
            let html = r#"
                <!DOCTYPE html>
                <html>
                <head>
                    <title>Redirect</title>
                    <meta http-equiv="refresh" content="0;url=https://example.com">
                </head>
                <body>Redirecting...</body>
                </html>
            "#;
            let result = extract_meta_refresh(html);
            assert_eq!(result, Some("https://example.com".to_string()));
        }

        #[test]
        fn test_no_meta_refresh() {
            let html = r#"<html><body>No redirect here</body></html>"#;
            let result = extract_meta_refresh(html);
            assert_eq!(result, None);
        }

        #[test]
        fn test_meta_without_refresh() {
            let html = r#"<meta charset="utf-8">"#;
            let result = extract_meta_refresh(html);
            assert_eq!(result, None);
        }

        #[test]
        fn test_without_url() {
            let html = r#"<meta http-equiv="refresh" content="5">"#;
            let result = extract_meta_refresh(html);
            assert_eq!(result, None);
        }

        #[test]
        fn test_with_relative_url() {
            let html = r#"<meta http-equiv="refresh" content="0;url=/relative/path">"#;
            let result = extract_meta_refresh(html);
            assert_eq!(result, Some("/relative/path".to_string()));
        }

        #[test]
        fn test_with_url_with_query_params() {
            let html = r#"<meta http-equiv="refresh" content="0;url=https://example.com?foo=bar&baz=qux">"#;
            let result = extract_meta_refresh(html);
            assert_eq!(
                result,
                Some("https://example.com?foo=bar&baz=qux".to_string())
            );
        }

        #[test]
        fn test_empty_html() {
            let html = "";
            let result = extract_meta_refresh(html);
            assert_eq!(result, None);
        }

        #[test]
        fn test_malformed_meta_tag() {
            let html = r#"<meta http-equiv="refresh" content=">"#;
            let result = extract_meta_refresh(html);
            assert_eq!(result, None);
        }
    }

    mod errors {
        use super::*;

        #[tokio::test]
        async fn test_invalid_url() {
            let result = resolve("not a valid url").await;
            assert!(result.is_err());
        }

        #[tokio::test]
        #[ignore]
        async fn test_too_many_redirects() {
            // This is hard to test without a mock server.  A URL with > 20
            // redirects should error
        }

        #[tokio::test]
        #[ignore]
        async fn test_timeout() {
            // This is hard to test without a mock server.  If the server doesn't
            // respond in 30 seconds then the function will error.
        }

        #[tokio::test]
        async fn test_invalid_scheme() {
            let result = resolve("hxxp://example.com").await;
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_dns_failure() {
            let result = resolve("5792d248-2714-4923-8aa4-6c8ff4016a44.govt.nz").await;
            assert!(result.is_err());
        }

        #[tokio::test]
        #[ignore]
        async fn test_connection_refused() {
            // Attempt to connect to server on a port that's open but not listening
            //
            // This is hard to test without a mock server
        }
    }
}
