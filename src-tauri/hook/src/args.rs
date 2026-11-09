#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Args {
    pub from: String,
    pub sound: String,
    pub source: String,
    pub status: String,
    pub message: String,
    pub title: String,
    pub project: String,
    pub url: String,
    pub sticky: bool,
}

const SOURCES: [&str; 4] = ["agent", "claude", "ci", "custom"];
const STATUSES: [&str; 4] = ["ok", "fail", "attention", "info"];

pub fn parse<I: IntoIterator<Item = String>>(raw: I) -> Args {
    let mut args = Args::default();
    let mut rest = raw.into_iter().peekable();
    if let Some(first) = rest.peek() {
        if !first.starts_with('-') {
            args.sound = rest.next().unwrap_or_default();
        }
    }
    while let Some(opt) = rest.next() {
        let field = match opt.as_str() {
            "--sticky" => {
                args.sticky = true;
                continue;
            }
            "--from" => &mut args.from,
            "--sound" => &mut args.sound,
            "--source" => &mut args.source,
            "--status" => &mut args.status,
            "--message" => &mut args.message,
            "--title" => &mut args.title,
            "--project" => &mut args.project,
            "--url" => &mut args.url,
            _ => continue,
        };
        *field = rest.next().unwrap_or_default();
    }
    if !SOURCES.contains(&args.source.as_str()) {
        args.source.clear();
    }
    if !STATUSES.contains(&args.status.as_str()) {
        args.status.clear();
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(list: &[&str]) -> Args {
        parse(list.iter().map(|s| s.to_string()))
    }

    #[test]
    fn primeiro_argumento_sem_traco_e_o_som() {
        assert_eq!(p(&["/x/Glass.aiff"]).sound, "/x/Glass.aiff");
        assert_eq!(p(&["--from", "claude"]).sound, "");
    }

    #[test]
    fn flags_com_valor_e_sticky() {
        let a = p(&[
            "--from", "cursor", "--status", "fail", "--sticky", "--title", "PR #1",
        ]);
        assert_eq!(
            (
                a.from.as_str(),
                a.status.as_str(),
                a.sticky,
                a.title.as_str()
            ),
            ("cursor", "fail", true, "PR #1")
        );
    }

    #[test]
    fn valores_fora_da_lista_e_flags_desconhecidas() {
        let a = p(&["--status", "talvez", "--source", "hacker"]);
        assert_eq!((a.status.as_str(), a.source.as_str()), ("", ""));
        assert_eq!(p(&["--desconhecida", "--status", "ok"]).status, "ok");
        assert_eq!(p(&["--status", "info", "--message"]).message, "");
    }
}
