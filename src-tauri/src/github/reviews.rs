use std::collections::HashMap;

use serde_json::{json, Value};

use super::api::ApiError;
use crate::i18n::Lang;
use crate::model::{Notification, Source, Status};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Approved,
    ChangesRequested,
    Reviewed,
    Comment,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Activity {
    pub id: String,
    pub kind: Kind,
    pub author: String,
    pub bot: bool,
    pub url: String,
    pub updated: String,
    pub inline: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrRef {
    pub number: u64,
    pub title: String,
    pub repo: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub pr: PrRef,
    pub activity: Activity,
    pub edited: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub pr: PrRef,
    pub activity: Activity,
    pub edited: bool,
    pub text: String,
    pub score: Option<(u8, u8)>,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct Seen {
    updated: String,
    score: Option<(u8, u8)>,
}

#[derive(Debug, Default)]
pub struct ReviewWatcher {
    prs: HashMap<String, HashMap<String, Seen>>,
}

pub(super) fn parse_activity(pr: &Value) -> Vec<Activity> {
    let author = |node: &Value| {
        let a = &node["author"];
        (
            a["login"].as_str().unwrap_or("ghost").to_string(),
            a["__typename"].as_str() == Some("Bot"),
        )
    };
    let mut out = Vec::new();
    for node in pr["reviews"]["nodes"].as_array().into_iter().flatten() {
        let kind = match node["state"].as_str() {
            Some("APPROVED") => Kind::Approved,
            Some("CHANGES_REQUESTED") => Kind::ChangesRequested,
            Some("COMMENTED") => Kind::Reviewed,
            _ => continue,
        };
        let (Some(id), Some(url)) = (node["id"].as_str(), node["url"].as_str()) else {
            continue;
        };
        let (login, bot) = author(node);
        out.push(Activity {
            id: id.into(),
            kind,
            author: login,
            bot,
            url: url.into(),
            updated: node["updatedAt"].as_str().unwrap_or_default().into(),
            inline: node["comments"]["totalCount"].as_u64().unwrap_or(0) as u32,
        });
    }
    for node in pr["comments"]["nodes"].as_array().into_iter().flatten() {
        let (Some(id), Some(url)) = (node["id"].as_str(), node["url"].as_str()) else {
            continue;
        };
        let (login, bot) = author(node);
        out.push(Activity {
            id: id.into(),
            kind: Kind::Comment,
            author: login,
            bot,
            url: url.into(),
            updated: node["updatedAt"].as_str().unwrap_or_default().into(),
            inline: 0,
        });
    }
    out
}

impl ReviewWatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn diff(&mut self, prs: &[super::model::Pr], viewer: &str) -> Vec<Candidate> {
        let mut next = HashMap::new();
        let mut out = Vec::new();
        for pr in prs {
            let old = self.prs.get(&pr.url);
            let latest = old.and_then(|o| o.values().map(|s| s.updated.as_str()).max());
            let mut seen = HashMap::new();
            for a in &pr.activity {
                let before = old.and_then(|o| o.get(&a.id));
                let edited = before.is_some_and(|b| b.updated != a.updated);
                let fresh = old.is_some()
                    && before.is_none()
                    && latest.is_none_or(|l| a.updated.as_str() >= l);
                let mine = a.author.eq_ignore_ascii_case(viewer);
                if !mine && (fresh || (edited && a.bot)) {
                    out.push(Candidate {
                        pr: PrRef {
                            number: pr.number,
                            title: pr.title.clone(),
                            repo: pr.repo.clone(),
                        },
                        activity: a.clone(),
                        edited,
                    });
                }
                seen.insert(
                    a.id.clone(),
                    Seen {
                        updated: a.updated.clone(),
                        score: before.and_then(|b| b.score),
                    },
                );
            }
            next.insert(pr.url.clone(), seen);
        }
        self.prs = next;
        out
    }

    pub fn resolve(
        &mut self,
        candidates: Vec<Candidate>,
        texts: &HashMap<String, String>,
    ) -> Vec<Event> {
        let mut out = Vec::new();
        for c in candidates {
            let text = texts.get(&c.activity.id).cloned().unwrap_or_default();
            let score = confidence(&text);
            let seen = self
                .prs
                .values_mut()
                .find_map(|m| m.get_mut(&c.activity.id));
            let before = seen.as_ref().and_then(|s| s.score);
            if let Some(seen) = seen {
                seen.score = score;
            }
            if c.edited && (score.is_none() || score == before) {
                continue;
            }
            let empty = first_line(&text).is_empty();
            if !c.edited
                && empty
                && c.activity.inline == 0
                && c.activity.kind != Kind::Approved
                && c.activity.kind != Kind::ChangesRequested
            {
                continue;
            }
            out.push(Event {
                pr: c.pr,
                activity: c.activity,
                edited: c.edited,
                text,
                score,
            });
        }
        out
    }
}

pub fn confidence(text: &str) -> Option<(u8, u8)> {
    let lower = text.to_lowercase();
    let start = lower.find("confidence")?;
    let window: Vec<char> = lower[start..].chars().take(80).collect();
    for i in 0..window.len() {
        if window[i] != '/' {
            continue;
        }
        let digits = |range: &mut dyn Iterator<Item = &char>| -> Option<u8> {
            let s: String = range.take_while(|c| c.is_ascii_digit()).collect();
            s.parse().ok()
        };
        let before: String = window[..i]
            .iter()
            .rev()
            .skip_while(|c| **c == ' ')
            .take_while(|c| c.is_ascii_digit())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let after = digits(&mut window[i + 1..].iter().skip_while(|c| **c == ' '));
        if let (Ok(n), Some(m)) = (before.parse::<u8>(), after) {
            if (1..=10).contains(&m) && n <= m {
                return Some((n, m));
            }
        }
    }
    None
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default()
        .to_string()
}

pub fn display_name(login: &str, bot: bool) -> String {
    if !bot {
        return login.to_string();
    }
    let name = login.trim_end_matches("[bot]");
    let name = name
        .strip_suffix("-apps")
        .or_else(|| name.strip_suffix("-app"))
        .unwrap_or(name);
    let mut chars = name.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => login.to_string(),
    }
}

fn message(e: &Event, lang: Lang) -> String {
    let who = display_name(&e.activity.author, e.activity.bot);
    let snippet = first_line(&e.text);
    let quoted = |s: &str| format!("{who}: “{s}”");
    let score = e.score.map(|(n, m)| match lang {
        Lang::PtBr => format!("Confiança {n}/{m}"),
        Lang::En => format!("Confidence {n}/{m}"),
    });
    match (lang, e.activity.kind, score) {
        (Lang::PtBr, _, Some(s)) if e.edited => format!("{who} atualizou o review · {s}"),
        (Lang::En, _, Some(s)) if e.edited => format!("{who} updated the review · {s}"),
        (Lang::PtBr, Kind::Approved, _) => format!("{who} aprovou"),
        (Lang::En, Kind::Approved, _) => format!("{who} approved"),
        (Lang::PtBr, Kind::ChangesRequested, _) => format!("{who} pediu mudanças"),
        (Lang::En, Kind::ChangesRequested, _) => format!("{who} requested changes"),
        (Lang::PtBr, _, Some(s)) => format!("{who} revisou · {s}"),
        (Lang::En, _, Some(s)) => format!("{who} reviewed · {s}"),
        (_, _, None) if !snippet.is_empty() => quoted(&snippet),
        (Lang::PtBr, _, None) if e.activity.inline == 1 => format!("{who} deixou 1 comentário"),
        (Lang::PtBr, _, None) => format!("{who} deixou {} comentários", e.activity.inline),
        (Lang::En, _, None) if e.activity.inline == 1 => format!("{who} left a comment"),
        (Lang::En, _, None) => format!("{who} left {} comments", e.activity.inline),
    }
}

fn status(e: &Event) -> Status {
    match (e.activity.kind, e.score) {
        (Kind::ChangesRequested, _) => Status::Attention,
        (_, Some((n, m))) if u32::from(n) * 2 <= u32::from(m) => Status::Attention,
        (_, Some((n, m))) if u32::from(n) * 5 >= u32::from(m) * 4 => Status::Ok,
        (Kind::Approved, _) => Status::Ok,
        _ => Status::Info,
    }
}

pub fn notification(id: u64, e: &Event, lang: Lang) -> Notification {
    let status = status(e);
    let clip = |s: &str, max: usize| -> String {
        let mut out: String = s.chars().take(max).collect();
        if s.chars().count() > max {
            out.pop();
            out.push('…');
        }
        out
    };
    let title = if e.pr.title.trim().is_empty() {
        format!("PR #{}", e.pr.number)
    } else {
        format!("PR #{} · {}", e.pr.number, e.pr.title.trim())
    };
    Notification {
        id,
        source: Source::Ci,
        status,
        title: Some(clip(&title, super::card::MAX_PR_TITLE_CHARS)),
        message: Some(clip(&message(e, lang), crate::model::MAX_MESSAGE_CHARS)),
        project: Some(clip(
            &format!("{}#{}", e.pr.repo, e.pr.number),
            crate::model::MAX_PROJECT_CHARS,
        )),
        url: crate::model::clean_url(&e.activity.url),
        ttl_ms: if status == Status::Attention {
            crate::model::TTL_ATTENTION_MS
        } else {
            12_000
        },
        progress: None,
        sticky: status == Status::Attention,
    }
}

const TEXTS: &str = r#"query($ids: [ID!]!) {
  nodes(ids: $ids) {
    ... on PullRequestReview { id bodyText }
    ... on IssueComment { id bodyText }
  }
}"#;

pub(super) fn parse_texts(body: &Value) -> HashMap<String, String> {
    body["data"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|n| {
            Some((
                n["id"].as_str()?.to_string(),
                n["bodyText"].as_str().unwrap_or_default().to_string(),
            ))
        })
        .collect()
}

pub(crate) async fn fetch_texts(
    client: &reqwest::Client,
    token: &str,
    ids: &[String],
) -> Result<HashMap<String, String>, ApiError> {
    let mut texts = HashMap::new();
    for chunk in ids.chunks(100) {
        let request = json!({ "query": TEXTS, "variables": { "ids": chunk } });
        let body = super::api::graphql(client, token, &request).await?;
        texts.extend(parse_texts(&body));
    }
    Ok(texts)
}
