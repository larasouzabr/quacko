//! Monitor de PRs do GitHub (spec v2, seção 6.2), passo 1: usa o login do `gh`.
//!
//! - Uma consulta GraphQL traz todos os PRs abertos da pessoa com o resumo dos checks
//!   do último commit (`statusCheckRollup`: check runs + status de commit, o que inclui
//!   o Jenkins), com a contagem por estado. Custa 1 ponto do limite de taxa.
//! - Progresso: quando mais checks terminam, o cartão do PR é atualizado ("3 de 12").
//!   Final: quando todos terminaram e o resumo não está mais pendente. Sempre para o
//!   mesmo commit; PR visto pela primeira vez não avisa; push novo reinicia.
//! - Progresso é só o cartão; o resultado final toca o som (Glass/Basso). Música não é pausada.
//! - Consulta a cada 30 s enquanto algum PR está rodando, a cada 3 min quando tudo
//!   terminou; recua em erro de rede e respeita o limite de taxa.
//! - O token vem de `gh auth token` (o app procura o `gh` sozinho, porque um app aberto
//!   pelo Finder não tem o PATH do terminal). O passo 2 (GitHub App + login por código)
//!   troca só a origem do token.

pub mod api;
pub mod auth;
pub mod card;
pub mod commands;
pub mod gh_cli;
pub mod model;
pub mod monitor;
pub mod ui;

#[cfg(test)]
mod tests;

pub use gh_cli::{find_gh, gh_token};
pub use monitor::{device_flow_client_id, spawn, stored_token_present, Monitor, Report};
