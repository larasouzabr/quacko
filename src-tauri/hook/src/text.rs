use serde_json::Value;

pub const PROMPT_CHARS: usize = 100;

pub fn field(input: Option<&Value>, path: &str) -> String {
    let mut current = match input {
        Some(v) if v.is_object() => v,
        _ => return String::new(),
    };
    for key in path.split('.') {
        let next = match key.parse::<usize>() {
            Ok(i) => current.get(i),
            Err(_) => current.get(key),
        };
        match next {
            Some(v) => current = v,
            None => return String::new(),
        }
    }
    match current {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

pub fn one_line(text: &str) -> String {
    let spaced: String = text
        .chars()
        .map(|c| {
            if matches!(c, '\n' | '\r' | '\t') {
                ' '
            } else {
                c
            }
        })
        .collect();
    let mut squeezed = String::with_capacity(spaced.len());
    for c in spaced.chars() {
        if c == ' ' && squeezed.ends_with(' ') {
            continue;
        }
        squeezed.push(c);
    }
    let trimmed = squeezed.trim_matches(' ');
    if trimmed.chars().count() > PROMPT_CHARS {
        let mut cut: String = trimmed.chars().take(PROMPT_CHARS - 1).collect();
        cut.push('…');
        cut
    } else {
        trimmed.to_string()
    }
}

pub fn quoted(text: &str) -> String {
    if text.is_empty() {
        String::new()
    } else {
        format!("“{text}”")
    }
}

pub fn basename(path: &str) -> String {
    let trimmed = path.trim_end_matches(['/', '\\']);
    if trimmed.is_empty() {
        return if path.is_empty() {
            String::new()
        } else {
            path[..1].to_string()
        };
    }
    trimmed
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(trimmed)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn le_campos_aninhados_e_tolera_tipos() {
        let v = json!({"cwd": "/x", "roots": ["/a/b"], "n": 3, "b": true, "z": null});
        assert_eq!(field(Some(&v), "cwd"), "/x");
        assert_eq!(field(Some(&v), "roots.0"), "/a/b");
        assert_eq!(field(Some(&v), "n"), "3");
        assert_eq!(field(Some(&v), "b"), "true");
        assert_eq!(field(Some(&v), "z"), "");
        assert_eq!(field(Some(&v), "falta"), "");
        assert_eq!(field(Some(&json!([1])), "0"), "", "só objeto");
        assert_eq!(field(None, "cwd"), "");
    }

    #[test]
    fn uma_linha_e_corte_em_cem_caracteres() {
        assert_eq!(
            one_line("  adicione testes\npara o parser   de datas \t"),
            "adicione testes para o parser de datas"
        );
        let long = "á".repeat(300);
        let cut = one_line(&long);
        assert_eq!(cut.chars().count(), PROMPT_CHARS);
        assert!(cut.ends_with('…'));
        assert_eq!(
            one_line(&"b".repeat(PROMPT_CHARS)),
            "b".repeat(PROMPT_CHARS)
        );
    }

    #[test]
    fn nome_da_pasta_em_unix_e_windows() {
        assert_eq!(basename("/Users/x/lagoa-api"), "lagoa-api");
        assert_eq!(basename("/Users/x/app/"), "app");
        assert_eq!(basename(r"C:\Users\Some Name\meu-app"), "meu-app");
        assert_eq!(basename("/"), "/");
        assert_eq!(basename(""), "");
    }

    #[test]
    fn aspas_so_com_texto() {
        assert_eq!(quoted("oi"), "“oi”");
        assert_eq!(quoted(""), "");
    }
}
