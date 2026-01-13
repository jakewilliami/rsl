<h1 align="center">RSL: Resolve Share Link</h1>

## Description

In times past, it was much simpler to remove tracking information.  Manually remove some query parameters before hitting "send" and it was all gone.  Now, big players are dynamically constructing shortened links specifically for sharing.  For instance, [Reddit has been doing this](https://www.reddit.com/r/privacy/comments/16rm64l) for at least a couple of years.  This is but one method used to allow organisations to build relational networks between you and the people with whom you share content.

RSL is a simple command line utility that locally resolves a share link to its "canonical" form, without tracking.  It follows the chain of redirects, using a random user agent, in the interest of privacy.  A VPN would be additionally recommended if you are resolving a link that is not your own.

After the link is resolved to its final form, sans tracking information, it is copied to your clipboard.  The clipboard functonality should also work over SSH, RDP, on WSL, and with X11.

## Quick Start

```shell
$ just

$ ./rsl https://reddit.com/r/privacy/s/ZNNlWWQprj
https://www.reddit.com/r/AskTheWorld/comments/1q2rw7m/comment/nxfc5ci

$ pbpaste  # on macOS
https://www.reddit.com/r/AskTheWorld/comments/1q2rw7m/comment/nxfc5ci
```

## Input Validation

RSL supports specific input validation for Reddit, Facebook, and Instagram share links.  An option will be implemented that will allow resultion without input validation or special handling.
