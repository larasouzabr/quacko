//! Acessórios do usuário (ex.: laço): pasta própria e validação para importar no futuro.
//!
//! Acessório é só dado: um `accessory.json` e um sprite sheet PNG por estado, com a mesma
//! grade do personagem (mesmo tamanho ou múltiplo inteiro, ex.: 1x ou 6x). Nada é
//! executado. A importação de `.zip` recusa caminhos com `..` ou absolutos, links
//! simbólicos, arquivos fora do esperado, PNG falso e tamanhos acima dos limites
//! (os mesmos dos pacotes de personagem).

// A validação e a importação ficam prontas para a futura tela "Importar acessório";
// hoje o app só cria a pasta e os testes usam o resto.
#![allow(dead_code)]

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::Value;

pub const MAX_FILES: usize = 24;
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: u64 = 5 * 1024 * 1024;
const MAX_JSON_BYTES: u64 = 16 * 1024;
const MAX_SCALE: u32 = 16;
const STATES: [&str; 4] = ["ok", "fail", "attention", "info"];
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessoryInfo {
    pub id: String,
    /// Fator em relação ao sprite do personagem (1 = mesmo tamanho; 6 = ampliado 6x).
    pub scale: u32,
}

/// Pasta dos acessórios do usuário (criada ao abrir o app).
pub fn user_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".config").join("quacko").join("accessories"))
}

pub fn ensure_user_dir() -> Option<PathBuf> {
    let dir = user_dir()?;
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn valid_id(id: &str) -> bool {
    (1..=40).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.starts_with('-')
}

fn valid_png_name(name: &str) -> bool {
    name.len() <= 45
        && name.to_ascii_lowercase().ends_with(".png")
        && name[..name.len() - 4]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && name.len() > 4
}

/// Largura e altura de um PNG de verdade (assinatura + cabeçalho IHDR).
pub fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || bytes[..8] != PNG_SIGNATURE || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

/// Confere o accessory.json e devolve o id e os arquivos de cada estado.
fn parse_manifest(text: &str) -> Result<(String, Vec<String>), String> {
    let v: Value =
        serde_json::from_str(text).map_err(|e| format!("accessory.json inválido: {e}"))?;
    if v.get("schema").and_then(Value::as_u64) != Some(1) {
        return Err("schema não suportado (esperado 1)".into());
    }
    let id = v
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| valid_id(id))
        .ok_or("id inválido")?;
    if v.get("slot").and_then(Value::as_str) != Some("head")
        || v.get("follows").and_then(Value::as_str) != Some("head")
    {
        return Err("slot e follows precisam ser \"head\"".into());
    }
    let files = v
        .get("files")
        .and_then(Value::as_object)
        .ok_or("faltam os arquivos (files)")?;
    let mut names = Vec::new();
    for state in STATES {
        let name = files
            .get(state)
            .and_then(Value::as_str)
            .filter(|n| valid_png_name(n))
            .ok_or_else(|| format!("arquivo do estado \"{state}\" ausente ou com nome inválido"))?;
        names.push(name.to_string());
    }
    let recolor = v
        .get("recolor")
        .and_then(Value::as_object)
        .ok_or("falta o recolor")?;
    for key in ["main", "shade", "light"] {
        let ok = recolor.get(key).and_then(Value::as_array).is_some_and(|a| {
            a.len() == 3 && a.iter().all(|c| c.as_u64().is_some_and(|c| c <= 255))
        });
        if !ok {
            return Err(format!("recolor.{key} precisa ser [r, g, b] de 0 a 255"));
        }
    }
    Ok((id.to_string(), names))
}

/// Valida uma pasta de acessório contra o sprite do personagem (`sheet` = largura e
/// altura de um sprite sheet de estado, em pixels do arquivo).
pub fn validate_dir(dir: &Path, sheet: (u32, u32)) -> Result<AccessoryInfo, String> {
    let manifest_path = dir.join("accessory.json");
    let meta =
        fs::symlink_metadata(&manifest_path).map_err(|_| "falta o accessory.json".to_string())?;
    if !meta.is_file() || meta.len() > MAX_JSON_BYTES {
        return Err("accessory.json inválido ou grande demais".into());
    }
    let (id, names) =
        parse_manifest(&fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?)?;

    // Só o accessory.json e os PNGs listados; nada de pastas, links ou arquivos a mais.
    let mut total = meta.len();
    let entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    if entries.len() > MAX_FILES {
        return Err(format!("arquivos demais (máximo {MAX_FILES})"));
    }
    for entry in &entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let kind = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
        if kind.file_type().is_symlink() {
            return Err(format!("\"{name}\" é um link simbólico"));
        }
        if !kind.is_file() {
            return Err(format!("\"{name}\" não é um arquivo"));
        }
        if name != "accessory.json" && !names.contains(&name) {
            return Err(format!("arquivo fora do esperado: \"{name}\""));
        }
    }

    let mut scale = None;
    for name in &names {
        let path = dir.join(name);
        let meta = fs::symlink_metadata(&path).map_err(|_| format!("falta \"{name}\""))?;
        if meta.len() > MAX_FILE_BYTES {
            return Err(format!("\"{name}\" passa de 2 MB"));
        }
        total += meta.len();
        let bytes = fs::read(&path).map_err(|e| e.to_string())?;
        let (w, h) =
            png_size(&bytes).ok_or_else(|| format!("\"{name}\" não é um PNG de verdade"))?;
        let k = (w % sheet.0 == 0)
            .then(|| w / sheet.0)
            .filter(|k| (1..=MAX_SCALE).contains(k) && h == sheet.1 * k);
        let Some(k) = k else {
            return Err(format!(
                "\"{name}\" tem {w}x{h}: precisa ter a grade do personagem ({}x{} ou um múltiplo inteiro)",
                sheet.0, sheet.1
            ));
        };
        if scale.is_some_and(|s| s != k) {
            return Err("os estados precisam ter o mesmo tamanho".into());
        }
        scale = Some(k);
    }
    if total > MAX_TOTAL_BYTES {
        return Err("acessório passa de 5 MB".into());
    }
    Ok(AccessoryInfo {
        id,
        scale: scale.unwrap_or(1),
    })
}

/// Importa um `.zip` para `dest_root/<id>`. Tudo é conferido antes de ficar no lugar
/// final; em qualquer erro, nada fica para trás.
pub fn import_zip(zip_path: &Path, dest_root: &Path, sheet: (u32, u32)) -> Result<PathBuf, String> {
    let file = fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("zip inválido: {e}"))?;
    if archive.len() > MAX_FILES + 1 {
        return Err(format!("arquivos demais (máximo {MAX_FILES})"));
    }
    fs::create_dir_all(dest_root).map_err(|e| e.to_string())?;
    let staging = dest_root.join(format!(".importando-{}", std::process::id()));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|e| e.to_string())?;

    let result = (|| {
        let mut total = 0u64;
        let mut top: Option<String> = None;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
            let raw = entry.name().map(|n| n.to_string()).unwrap_or_default();
            if entry.is_symlink() {
                return Err(format!("\"{}\" é um link simbólico", raw));
            }
            // enclosed_name recusa caminhos absolutos e com "..".
            let path = entry
                .enclosed_name()
                // Recusa explícita de ".." e de caminho absoluto (Unix, Windows ou com drive),
                // sem depender de como cada biblioteca normaliza.
                .filter(|_| {
                    !raw.contains("..")
                        && !raw.starts_with('/')
                        && !raw.starts_with('\\')
                        && !raw.contains(':')
                })
                .ok_or_else(|| format!("caminho inválido no zip: \"{}\"", raw))?;
            let parts: Vec<String> = path
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect();
            if entry.is_dir() {
                if parts.len() > 1 {
                    return Err("pastas dentro de pastas não são aceitas".into());
                }
                continue;
            }
            // Arquivos na raiz, ou todos dentro de uma única pasta (ex.: "laco/").
            let name = match parts.as_slice() {
                [name] => name.clone(),
                [dir, name] => {
                    if top.get_or_insert_with(|| dir.clone()) != dir {
                        return Err("o zip deve ter uma pasta só".into());
                    }
                    name.clone()
                }
                _ => return Err(format!("caminho fundo demais: \"{}\"", raw)),
            };
            if name != "accessory.json" && !valid_png_name(&name) {
                return Err(format!("arquivo fora do esperado: \"{name}\""));
            }
            if entry.size() > MAX_FILE_BYTES {
                return Err(format!("\"{name}\" passa de 2 MB"));
            }
            // Lê com limite (não confia só no tamanho declarado no zip).
            let mut bytes = Vec::new();
            (&mut entry)
                .take(MAX_FILE_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() as u64 > MAX_FILE_BYTES {
                return Err(format!("\"{name}\" passa de 2 MB"));
            }
            total += bytes.len() as u64;
            if total > MAX_TOTAL_BYTES {
                return Err("acessório passa de 5 MB".into());
            }
            fs::write(staging.join(&name), &bytes).map_err(|e| e.to_string())?;
        }
        let info = validate_dir(&staging, sheet)?;
        let dest = dest_root.join(&info.id);
        if dest.exists() {
            return Err(format!("já existe um acessório \"{}\"", info.id));
        }
        fs::rename(&staging, &dest).map_err(|e| e.to_string())?;
        Ok(dest)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sprite sheet de um estado do Pato Pixel em 1x: 2 quadros de 28x28.
    pub(crate) const SHEET_1X: (u32, u32) = (56, 28);

    pub(crate) fn embedded_bow() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/assets/accessories/laco")
    }

    #[test]
    fn laco_embutido_e_valido() {
        assert_eq!(
            validate_dir(&embedded_bow(), SHEET_1X),
            Ok(AccessoryInfo {
                id: "laco".into(),
                scale: 1
            })
        );
    }

    #[test]
    fn arquivo_a_mais_e_recusado() {
        let dir = tempfile::tempdir().unwrap();
        for e in fs::read_dir(embedded_bow()).unwrap() {
            let e = e.unwrap();
            fs::copy(e.path(), dir.path().join(e.file_name())).unwrap();
        }
        fs::write(dir.path().join("script.sh"), "rm -rf ~").unwrap();
        assert!(validate_dir(dir.path(), SHEET_1X)
            .unwrap_err()
            .contains("fora do esperado"));
    }

    // ---------- testes pedidos: tamanho inválido, zip slip, arquivos fora do esperado ----------

    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn fixture_6x() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/laco-6x")
    }

    /// Cópia do laço 1x numa pasta temporária, para estragar à vontade.
    fn bow_copy() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for e in fs::read_dir(embedded_bow()).unwrap() {
            let e = e.unwrap();
            fs::copy(e.path(), dir.path().join(e.file_name())).unwrap();
        }
        dir
    }

    /// Só o começo de um PNG (assinatura + IHDR) com o tamanho pedido: basta para a checagem.
    fn png_header(w: u32, h: u32) -> Vec<u8> {
        let mut b = PNG_SIGNATURE.to_vec();
        b.extend_from_slice(&13u32.to_be_bytes());
        b.extend_from_slice(b"IHDR");
        b.extend_from_slice(&w.to_be_bytes());
        b.extend_from_slice(&h.to_be_bytes());
        b.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        b
    }

    #[test]
    fn laco_6x_tambem_e_aceito() {
        assert_eq!(
            validate_dir(&fixture_6x(), SHEET_1X),
            Ok(AccessoryInfo {
                id: "laco".into(),
                scale: 6
            })
        );
    }

    #[test]
    fn tamanho_fora_da_grade_e_recusado() {
        for (w, h) in [(50, 28), (56, 30), (112, 28), (56 * 17, 28 * 17), (0, 0)] {
            let dir = bow_copy();
            fs::write(dir.path().join("fail.png"), png_header(w, h)).unwrap();
            let err = validate_dir(dir.path(), SHEET_1X).unwrap_err();
            assert!(err.contains("grade do personagem"), "{w}x{h}: {err}");
        }
    }

    #[test]
    fn estados_com_escalas_diferentes_sao_recusados() {
        let dir = bow_copy();
        fs::copy(fixture_6x().join("info.png"), dir.path().join("info.png")).unwrap();
        assert!(validate_dir(dir.path(), SHEET_1X)
            .unwrap_err()
            .contains("mesmo tamanho"));
    }

    #[test]
    fn png_falso_e_recusado() {
        let dir = bow_copy();
        fs::write(dir.path().join("ok.png"), "<svg onload=alert(1)>").unwrap();
        assert!(validate_dir(dir.path(), SHEET_1X)
            .unwrap_err()
            .contains("PNG de verdade"));
    }

    #[cfg(unix)]
    #[test]
    fn link_simbolico_na_pasta_e_recusado() {
        let dir = bow_copy();
        fs::remove_file(dir.path().join("ok.png")).unwrap();
        std::os::unix::fs::symlink("/etc/passwd", dir.path().join("ok.png")).unwrap();
        assert!(validate_dir(dir.path(), SHEET_1X)
            .unwrap_err()
            .contains("link"));
    }

    #[test]
    fn manifest_invalido_e_recusado() {
        for (json, expected) in [
            (r#"{"schema":2}"#, "schema"),
            (r#"{"schema":1,"id":"../x"}"#, "id"),
            (
                r#"{"schema":1,"id":"x","slot":"body","follows":"head"}"#,
                "head",
            ),
        ] {
            let dir = bow_copy();
            fs::write(dir.path().join("accessory.json"), json).unwrap();
            assert!(
                validate_dir(dir.path(), SHEET_1X)
                    .unwrap_err()
                    .contains(expected),
                "{json}"
            );
        }
    }

    /// Monta um zip com (nome, conteúdo) e, opcionalmente, um link simbólico.
    fn make_zip(dir: &Path, entries: &[(&str, Vec<u8>)], symlink: Option<(&str, &str)>) -> PathBuf {
        let path = dir.join("pacote.zip");
        let mut w = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        for (name, bytes) in entries {
            w.start_file(*name, SimpleFileOptions::default()).unwrap();
            w.write_all(bytes).unwrap();
        }
        if let Some((name, target)) = symlink {
            w.add_symlink(name, target, SimpleFileOptions::default())
                .unwrap();
        }
        w.finish().unwrap();
        path
    }

    fn bow_entries(prefix: &str) -> Vec<(String, Vec<u8>)> {
        fs::read_dir(embedded_bow())
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                (
                    format!("{prefix}{}", e.file_name().to_string_lossy()),
                    fs::read(e.path()).unwrap(),
                )
            })
            .collect()
    }

    fn zip_with(dir: &Path, extra: &[(&str, Vec<u8>)], symlink: Option<(&str, &str)>) -> PathBuf {
        let mut entries: Vec<(String, Vec<u8>)> = bow_entries("laco/");
        entries.extend(extra.iter().map(|(n, b)| (n.to_string(), b.clone())));
        let refs: Vec<(&str, Vec<u8>)> = entries
            .iter()
            .map(|(n, b)| (n.as_str(), b.clone()))
            .collect();
        make_zip(dir, &refs, symlink)
    }

    #[test]
    fn importa_zip_valido_e_nao_duplica() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("accessories");
        let zip = zip_with(tmp.path(), &[], None);
        let installed = import_zip(&zip, &dest, SHEET_1X).unwrap();
        assert_eq!(installed, dest.join("laco"));
        assert!(installed.join("ok.png").is_file());
        assert!(import_zip(&zip, &dest, SHEET_1X)
            .unwrap_err()
            .contains("já existe"));
    }

    #[test]
    fn zip_slip_e_recusado_e_nada_fica_para_tras() {
        for evil in [
            "../fora.png",
            "laco/../../fora.png",
            "/etc/fora.png",
            "C:\\fora.png",
            "\\\\servidor\\fora.png",
        ] {
            let tmp = tempfile::tempdir().unwrap();
            let dest = tmp.path().join("accessories");
            let zip = zip_with(tmp.path(), &[(evil, png_header(56, 28))], None);
            let err = import_zip(&zip, &dest, SHEET_1X).unwrap_err();
            assert!(
                err.contains("caminho") || err.contains("fora do esperado"),
                "{evil}: {err}"
            );
            assert!(
                !tmp.path().join("fora.png").exists(),
                "{evil}: escapou da pasta"
            );
            let left: Vec<_> = fs::read_dir(&dest).map(|d| d.collect()).unwrap_or_default();
            assert!(left.is_empty(), "{evil}: sobrou algo em {}", dest.display());
        }
    }

    #[test]
    fn link_simbolico_no_zip_e_recusado() {
        let tmp = tempfile::tempdir().unwrap();
        let zip = zip_with(tmp.path(), &[], Some(("laco/extra.png", "/etc/passwd")));
        let err = import_zip(&zip, &tmp.path().join("acc"), SHEET_1X).unwrap_err();
        assert!(err.contains("link"), "{err}");
    }

    #[test]
    fn arquivo_inesperado_no_zip_e_recusado() {
        for (name, bytes) in [
            ("laco/instalar.sh", b"rm -rf ~".to_vec()),
            ("laco/notas.txt", b"oi".to_vec()),
        ] {
            let tmp = tempfile::tempdir().unwrap();
            let zip = zip_with(tmp.path(), &[(name, bytes)], None);
            let err = import_zip(&zip, &tmp.path().join("acc"), SHEET_1X).unwrap_err();
            assert!(err.contains("fora do esperado"), "{name}: {err}");
        }
    }

    #[test]
    fn png_grande_demais_no_zip_e_recusado() {
        let tmp = tempfile::tempdir().unwrap();
        let mut big = png_header(56, 28);
        big.resize(MAX_FILE_BYTES as usize + 10, 0);
        let mut entries = bow_entries("laco/");
        entries.retain(|(n, _)| n != "laco/ok.png");
        entries.push(("laco/ok.png".into(), big));
        let refs: Vec<(&str, Vec<u8>)> = entries
            .iter()
            .map(|(n, b)| (n.as_str(), b.clone()))
            .collect();
        let zip = make_zip(tmp.path(), &refs, None);
        assert!(import_zip(&zip, &tmp.path().join("acc"), SHEET_1X)
            .unwrap_err()
            .contains("2 MB"));
    }

    #[test]
    fn png_size_le_o_cabecalho() {
        let bytes = fs::read(embedded_bow().join("ok.png")).unwrap();
        assert_eq!(png_size(&bytes), Some(SHEET_1X));
        assert_eq!(png_size(b"GIF89a nao e png de jeito nenhum"), None);
    }
}
