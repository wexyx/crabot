use std::collections::HashSet;
use url::Url;
pub(super) fn normalize(mut url: Url, ignore: &[String]) -> Url {
    // Hash routers identify different application pages; callers must not fetch them
    // as if they were ordinary HTTP fragments (the server never receives them).
    if !url
        .fragment()
        .is_some_and(|f| f.starts_with('/') || f.starts_with('!'))
    {
        url.set_fragment(None);
    }
    let query = url
        .query_pairs()
        .filter(|(key, _)| {
            let key = key.to_ascii_lowercase();
            !key.starts_with("utm_")
                && !matches!(
                    key.as_str(),
                    "gclid" | "dclid" | "fbclid" | "msclkid" | "mc_cid" | "mc_eid" | "_ga" | "_gl"
                )
                && !ignore.iter().any(|item| item.eq_ignore_ascii_case(&key))
        })
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect::<Vec<_>>();
    url.set_query(None);
    if !query.is_empty() {
        url.query_pairs_mut().extend_pairs(query);
    }
    url
}
pub(super) fn extract(bytes: &[u8], base: &Url, ignore: &[String], maximum: usize) -> Vec<Url> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return vec![];
    };
    let html = scraper::Html::parse_document(text);
    let selector = scraper::Selector::parse("a[href]").unwrap();
    let mut seen = HashSet::new();
    html.select(&selector)
        .filter_map(|node| {
            let href = node.value().attr("href")?;
            let url = normalize(base.join(href).ok()?, ignore);
            (matches!(url.scheme(), "http" | "https")
                && url.origin() == base.origin()
                && seen.insert(url.to_string()))
            .then_some(url)
        })
        .take(maximum)
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tracking_does_not_destroy_document_identity() {
        let url = normalize(
            Url::parse("https://example.org/view?id=42&utm_source=x&lang=zh&other=y#section")
                .unwrap(),
            &[],
        );
        assert_eq!(
            url.as_str(),
            "https://example.org/view?id=42&lang=zh&other=y"
        );
        assert_ne!(
            url,
            normalize(
                Url::parse("https://example.org/view?id=43&lang=zh&other=y").unwrap(),
                &[]
            )
        );
        assert_eq!(
            normalize(url, &["other".into()]).query(),
            Some("id=42&lang=zh")
        );
        assert_eq!(
            normalize(Url::parse("https://example.org/#/doc/42").unwrap(), &[]).fragment(),
            Some("/doc/42")
        );
    }
    #[test]
    fn links_stay_same_origin_and_deduplicate() {
        let base = Url::parse("https://example.org/start").unwrap();
        let hits=extract(br#"<a href="/a?utm_source=x">a</a><a href="/a">same</a><a href="/a?id=1">doc</a><a href="https://other.org/a">outside</a><a href="javascript:alert(1)">bad</a>"#,&base,&[],20);
        assert_eq!(hits.len(), 2);
    }
}
