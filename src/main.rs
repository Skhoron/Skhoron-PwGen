mod cli;
mod generator;
mod rng;

use std::io::{self, BufRead, Write};
use std::process::{Command, ExitCode, Stdio};

use cli::{Action, Config};
use generator::CharsetOptions;

fn main() -> ExitCode {
    let mut args: Vec<String> = Vec::new();
    for a in std::env::args_os().skip(1) {
        match a.into_string() {
            Ok(s) => args.push(s),
            Err(_) => {
                eprintln!("Ошибка: аргументы должны быть в кодировке UTF-8");
                return ExitCode::from(2);
            }
        }
    }

    let result = match cli::parse(&args) {
        Ok(Action::Help) => {
            print!("{}", cli::HELP);
            return ExitCode::SUCCESS;
        }
        Ok(Action::Version) => {
            println!("skhoron-pwgen {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Ok(Action::Interactive) => interactive().and_then(|cfg| run(&cfg)),
        Ok(Action::Run(cfg)) => run(&cfg),
        Err(e) => {
            eprintln!("Ошибка: {e}\nСправка: skhoron-pwgen --help");
            return ExitCode::from(2);
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Ошибка: {e}");
            ExitCode::from(1)
        }
    }
}

fn run(cfg: &Config) -> Result<(), String> {
    let charset = generator::build_charset(&cfg.charset)?;
    let repeats = !cfg.no_repeat;
    generator::check_params(cfg.length, cfg.count, charset.len(), repeats)?;

    let mut rng = rng::SkhoronRng::new()
        .map_err(|e| format!("не удалось получить начальную энтропию у ОС: {e}"))?;
    let passwords = (0..cfg.count)
        .map(|_| generator::generate(&mut rng, &charset, cfg.length, repeats))
        .collect::<Result<Vec<String>, String>>()?;
    let bits = generator::entropy_bits(charset.len(), cfg.length, repeats);

    if cfg.json {
        println!(
            "{}",
            to_json(&passwords, cfg.length, charset.len(), repeats, bits)
        );
    } else if cfg.quiet {
        for p in &passwords {
            println!("{p}");
        }
        if cfg.entropy {
            eprintln!("Энтропия: {bits:.1} бит");
        }
    } else {
        println!("{}", if passwords.len() == 1 { "Пароль:" } else { "Пароли:" });
        for p in &passwords {
            println!("{p}");
        }
        if cfg.entropy {
            println!("\nЭнтропия: {bits:.1} бит");
        }
    }

    if cfg.clipboard {
        match copy_to_clipboard(&passwords.join("\n")) {
            Ok(()) => {
                if !cfg.quiet {
                    eprintln!("Скопировано в буфер обмена.");
                }
            }
            Err(e) => eprintln!("Буфер обмена: {e}"),
        }
    }
    Ok(())
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn to_json(passwords: &[String], length: usize, charset_len: usize, repeats: bool, bits: f64) -> String {
    let items: Vec<String> = passwords
        .iter()
        .map(|p| format!("\"{}\"", json_escape(p)))
        .collect();
    format!(
        "{{\"passwords\":[{}],\"length\":{},\"charset_size\":{},\"repeats\":{},\"entropy_bits\":{:.2}}}",
        items.join(","),
        length,
        charset_len,
        repeats,
        bits
    )
}

fn clipboard_commands() -> Vec<(&'static str, Vec<&'static str>)> {
    if cfg!(target_os = "macos") {
        vec![("pbcopy", vec![])]
    } else if cfg!(windows) {
        vec![("clip", vec![])]
    } else {
        vec![
            ("wl-copy", vec![]),
            ("xclip", vec!["-selection", "clipboard"]),
            ("xsel", vec!["--clipboard", "--input"]),
        ]
    }
}

fn copy_to_clipboard(text: &str) -> Result<(), String> {
    for (cmd, args) in clipboard_commands() {
        let spawned = Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match spawned {
            Ok(c) => c,
            Err(_) => continue,
        };
        let written = match child.stdin.take() {
            Some(mut stdin) => stdin.write_all(text.as_bytes()).is_ok(),
            None => false,
        };
        let status = match child.wait() {
            Ok(s) => s,
            Err(_) => continue,
        };
        if written && status.success() {
            return Ok(());
        }
    }
    Err("не найдена утилита буфера обмена (pbcopy, clip, wl-copy, xclip или xsel)".to_string())
}

fn read_line(prompt: &str) -> Result<String, String> {
    print!("{prompt}");
    io::stdout().flush().map_err(|e| e.to_string())?;
    let mut s = String::new();
    let n = io::stdin()
        .lock()
        .read_line(&mut s)
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("ввод закрыт".to_string());
    }
    Ok(s.trim().to_string())
}

fn ask_number(prompt: &str, default: usize) -> Result<usize, String> {
    loop {
        let s = read_line(&format!("{prompt} [{default}]: "))?;
        if s.is_empty() {
            return Ok(default);
        }
        match s.parse::<usize>() {
            Ok(n) => return Ok(n),
            Err(_) => println!("Нужно целое число."),
        }
    }
}

fn parse_yes_no(s: &str) -> Option<bool> {
    match s.trim().to_lowercase().as_str() {
        "" | "y" | "yes" | "д" | "да" => Some(true),
        "n" | "no" | "н" | "нет" => Some(false),
        _ => None,
    }
}

fn ask_yes(prompt: &str) -> Result<bool, String> {
    loop {
        let s = read_line(&format!("{prompt} [Y/n]: "))?;
        match parse_yes_no(&s) {
            Some(v) => return Ok(v),
            None => println!("Введи y или n."),
        }
    }
}

fn interactive() -> Result<Config, String> {
    println!("╔══════════════════════════╗");
    println!("║      Skhoron-PwGen       ║");
    println!("╚══════════════════════════╝\n");

    let length = ask_number("Длина", 24)?;
    println!();
    let upper = ask_yes("A-Z")?;
    let lower = ask_yes("a-z")?;
    let digits = ask_yes("0-9")?;
    let symbols = ask_yes("!@#$%^&*")?;

    println!("\nПовторения:\n[1] Разрешены\n[2] Запрещены");
    let mode = loop {
        let m = ask_number("Выбор", 1)?;
        if m == 1 || m == 2 {
            break m;
        }
        println!("Введи 1 или 2.");
    };
    let count = ask_number("Количество", 1)?;
    println!();

    Ok(Config {
        length,
        count,
        charset: CharsetOptions {
            upper,
            lower,
            digits,
            symbols,
            ..CharsetOptions::default()
        },
        no_repeat: mode == 2,
        ..Config::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_escapes_special_chars() {
        assert_eq!(json_escape("a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(json_escape("\n"), "\\u000a");
    }

    #[test]
    fn yes_no_parsing() {
        assert_eq!(parse_yes_no(""), Some(true));
        assert_eq!(parse_yes_no("Y"), Some(true));
        assert_eq!(parse_yes_no(" да "), Some(true));
        assert_eq!(parse_yes_no("N"), Some(false));
        assert_eq!(parse_yes_no("нет"), Some(false));
        assert_eq!(parse_yes_no("m"), None);
    }

    #[test]
    fn json_shape() {
        let s = to_json(&["ab".to_string(), "cd".to_string()], 2, 70, true, 12.25);
        assert_eq!(
            s,
            "{\"passwords\":[\"ab\",\"cd\"],\"length\":2,\"charset_size\":70,\"repeats\":true,\"entropy_bits\":12.25}"
        );
    }
}