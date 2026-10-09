use super::Documents;
use serde_json::{Value, json};
use std::collections::{HashSet, VecDeque};
use url::Url;

pub(super) async fn import(store: &Documents, input: Value) -> Result<Value, String> {
    crawl(
        input,
        |url, ignore| async move { super::fetch::fetch(&url, &ignore).await },
        |document| store.store(document),
    )
    .await
}
async fn crawl<F, FF, S, SF>(
    input: Value,
    mut fetch: F,
    mut save_document: S,
) -> Result<Value, String>
where
    F: FnMut(String, Vec<String>) -> FF,
    FF: std::future::Future<Output = Result<(Vec<u8>, String, String), String>>,
    S: FnMut(Value) -> SF,
    SF: std::future::Future<Output = Result<Value, String>>,
{
    let depth = input["depth"].as_u64().unwrap_or(0);
    let maximum = input["max_pages"].as_u64().unwrap_or(20);
    if depth > 5 || !(1..=100).contains(&maximum) {
        return Err("depth must be 0..5; max_pages must be 1..100".into());
    }
    let ignore: Vec<String> = match input.get("ignored_query_params") {
        Some(v) => serde_json::from_value(v.clone())
            .map_err(|_| "ignored_query_params must be a string array")?,
        None => vec![],
    };
    if ignore.len() > 50 || ignore.iter().any(|s| s.len() > 100) {
        return Err("Too many ignored query parameters".into());
    }
    let seed = super::links::normalize(
        Url::parse(input["url"].as_str().ok_or("url required")?).map_err(|e| e.to_string())?,
        &ignore,
    );
    let mut queue = VecDeque::from([(seed, 0)]);
    let mut visited = HashSet::new();
    let mut documents = vec![];
    let mut errors = vec![];
    let mut origin = None;
    let mut attempted = 0;
    let mut truncated = false;
    while let Some((url, level)) = queue.pop_front() {
        if !visited.insert(url.to_string()) {
            continue;
        }
        if attempted >= maximum {
            truncated = true;
            break;
        }
        attempted += 1;
        let outcome=async{
            let (bytes,mime,source)=fetch(url.to_string(),ignore.clone()).await?;
            let final_url=Url::parse(&source).map_err(|e|e.to_string())?;
            if origin.as_ref().is_some_and(|o|*o!=final_url.origin()){return Err("Redirect left the crawl origin".into());}
            origin.get_or_insert(final_url.origin());visited.insert(source.clone());
            if level<depth && (mime.contains("html")||source.ends_with(".html")) {
                for link in super::links::extract(&bytes,&final_url,&ignore,1000){
                    if !visited.contains(link.as_str()) && !queue.iter().any(|(u,_)|*u==link){
                        if queue.len()>=1000{truncated=true;break;}
                        queue.push_back((link,level+1));
                    }
                }
            }
            let (title,content)=Documents::extract(bytes,source.clone(),mime).await?;
            let mut save=json!({"action":"save","title":if title.is_empty(){source.clone()}else{title},"source":source,"content":content});
            if level==0 {
                for key in ["id","expected_version","title"] {if let Some(value)=input.get(key){save[key]=value.clone();}}
            }
            save_document(save).await
        }.await;
        match outcome {
            Ok(doc) => documents.push(doc),
            Err(error) => {
                if attempted == 1 && depth == 0 {
                    return Err(error);
                }
                errors.push(json!({"url":url.as_str(),"error":error}));
            }
        }
        if depth == 0 {
            break;
        }
    }
    Ok(
        json!({"documents":documents,"errors":errors,"attempted":attempted,"depth":depth,"truncated":truncated}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn fixture(
        url: String,
        _ignore: Vec<String>,
    ) -> Result<(Vec<u8>, String, String), String> {
        let path = Url::parse(&url).unwrap();
        let html = match path.path() {
            "/" => {
                r#"<main>Root<a href="/doc?id=1&utm_source=x">one</a><a href="/doc?id=1">duplicate</a><a href="/doc?id=2">two</a><a href="https://elsewhere.test/">outside</a></main>"#
            }
            "/doc" => r#"<main>Document<a href="/child">child</a><a href="/">cycle</a></main>"#,
            "/child" => "<main>Child</main>",
            _ => return Err("not found".into()),
        };
        Ok((html.as_bytes().into(), "text/html".into(), url))
    }
    #[tokio::test]
    async fn depth_budget_and_parameter_identity() {
        for (depth, maximum, count, truncated) in [
            (0, 20, 1, false),
            (1, 20, 3, false),
            (2, 20, 4, false),
            (5, 2, 2, true),
        ] {
            let report = crawl(
                json!({"url":"https://example.test/","depth":depth,"max_pages":maximum}),
                fixture,
                |value| std::future::ready(Ok(value)),
            )
            .await
            .unwrap();
            assert_eq!(report["documents"].as_array().unwrap().len(), count);
            assert_eq!(report["truncated"], truncated);
            assert_eq!(report["errors"], json!([]));
        }
    }
}
