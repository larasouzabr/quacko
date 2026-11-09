use std::time::Duration;

use serde_json::{json, Value};

use super::model::*;

const GRAPHQL_URL: &str = "https://api.github.com/graphql";
const REST_URL: &str = "https://api.github.com";
const SEARCH: &str = "is:pr is:open author:@me archived:false";
const QUERY: &str = r#"query($q: String!) {
  viewer { login }
  search(query: $q, type: ISSUE, first: 50) {
    nodes {
      ... on PullRequest {
        number title url
        repository { nameWithOwner }
        commits(last: 1) { nodes { commit { oid statusCheckRollup {
          state
          contexts(first: 0) {
            checkRunCountsByState { state count }
            statusContextCountsByState { state count }
          }
        } } } }
      }
    }
  }
}"#;

#[derive(Debug)]
pub enum ApiError {
    Unauthorized,
    RateLimited(Duration),
    Other(String),
}

pub fn parse(body: &Value) -> Result<(String, Vec<Pr>), ApiError> {
    if let Some(errors) = body.get("errors").and_then(Value::as_array) {
        let msg = errors
            .iter()
            .filter_map(|e| e.get("message").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(ApiError::Other(format!("GraphQL: {msg}")));
    }
    let data = body
        .get("data")
        .ok_or_else(|| ApiError::Other("resposta sem data".into()))?;
    let login = data["viewer"]["login"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let prs = data["search"]["nodes"]
        .as_array()
        .map(|nodes| nodes.iter().filter_map(parse_pr).collect())
        .unwrap_or_default();
    Ok((login, prs))
}

fn parse_pr(node: &Value) -> Option<Pr> {
    let commit = &node["commits"]["nodes"][0]["commit"];
    let rollup = &commit["statusCheckRollup"];
    Some(Pr {
        number: node["number"].as_u64()?,
        title: node["title"].as_str().unwrap_or_default().to_string(),
        url: node["url"].as_str()?.to_string(),
        repo: node["repository"]["nameWithOwner"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        head: commit["oid"].as_str()?.to_string(),
        rollup: serde_json::from_value(rollup["state"].clone()).ok(),
        checks: count_checks(&rollup["contexts"]),
    })
}

pub(super) fn count_checks(contexts: &Value) -> Checks {
    let mut c = Checks::default();
    let mut add = |list: &Value, waiting: &[&str], failed: &[&str]| {
        for entry in list.as_array().into_iter().flatten() {
            let n = entry["count"].as_u64().unwrap_or(0) as u32;
            let state = entry["state"].as_str().unwrap_or_default();
            c.total += n;
            if !waiting.contains(&state) {
                c.done += n;
            }
            if failed.contains(&state) {
                c.failed += n;
            }
        }
    };
    add(
        &contexts["checkRunCountsByState"],
        &["QUEUED", "IN_PROGRESS", "WAITING", "PENDING", "REQUESTED"],
        &[
            "FAILURE",
            "TIMED_OUT",
            "STARTUP_FAILURE",
            "CANCELLED",
            "ACTION_REQUIRED",
        ],
    );
    add(
        &contexts["statusContextCountsByState"],
        &["PENDING", "EXPECTED"],
        &["FAILURE", "ERROR"],
    );
    c
}

pub(crate) async fn fetch(
    client: &reqwest::Client,
    token: &str,
) -> Result<(String, Vec<Pr>), ApiError> {
    let response = client
        .post(GRAPHQL_URL)
        .bearer_auth(token)
        .json(&json!({ "query": QUERY, "variables": { "q": SEARCH } }))
        .send()
        .await
        .map_err(|e| ApiError::Other(e.to_string()))?;
    parse(&read_json(response).await?)
}

pub(crate) async fn get_rest(
    client: &reqwest::Client,
    token: &str,
    path: &str,
) -> Result<Value, ApiError> {
    let response = client
        .get(format!("{REST_URL}{path}"))
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| ApiError::Other(e.to_string()))?;
    read_json(response).await
}

async fn read_json(response: reqwest::Response) -> Result<Value, ApiError> {
    let status = response.status();
    let retry_after = response
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    let remaining_zero = response
        .headers()
        .get("x-ratelimit-remaining")
        .is_some_and(|v| v.as_bytes() == b"0");
    match status.as_u16() {
        401 => return Err(ApiError::Unauthorized),
        403 | 429 if retry_after.is_some() || remaining_zero => {
            return Err(ApiError::RateLimited(Duration::from_secs(
                retry_after.unwrap_or(600).max(60),
            )));
        }
        s if !(200..300).contains(&s) => return Err(ApiError::Other(format!("HTTP {s}"))),
        _ => {}
    }
    response
        .json()
        .await
        .map_err(|e| ApiError::Other(e.to_string()))
}
