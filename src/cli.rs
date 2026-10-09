//! Разбор аргументов командной строки.

use crate::generator::CharsetOptions;

pub const HELP: &str = r#"skhoron-pwgen: генератор паролей

Без аргументов запускается интерактивный режим.

Использование:
  skhoron-pwgen [ПАРАМЕТРЫ]

Параметры:
  -l, --length N     длина пароля (по умолчанию 24)
  -c, --count N      сколько паролей создать (по умолчанию 1)
      --upper        символы A-Z
      --lower        символы a-z
      --digits       цифры 0-9
      --symbols      символы !@#$%^&*
                     если не указан ни один из четырёх, берутся все
      --no-repeat    символы в пароле не повторяются
      --no-similar   убрать похожие символы 0 O 1 l I
      --exclude STR  исключить все символы из STR
      --entropy      показать энтропию
      --clipboard    скопировать результат в буфер обмена
  -q, --quiet        вывести только пароли
      --json         вывод в JSON
  -h, --help         эта справка
  -V, --version      версия
"#;

#[derive(Debug)]
pub struct Config {
    pub length: usize,
    pub count: usize,
    pub charset: CharsetOptions,
    pub no_repeat: bool,
    pub entropy: bool,
    pub clipboard: bool,
    pub quiet: bool,
    pub json: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            length: 24,
            count: 1,
            charset: CharsetOptions::default(),
            no_repeat: false,
            entropy: false,
            clipboard: false,
            quiet: false,
            json: false,
        }
    }
}

#[derive(Debug)]
pub enum Action {
    Help,
    Version,
    Interactive,
    Run(Config),
}

fn take_value<'a>(
    flag: &str,
    inline: Option<&'a str>,
    args: &'a [String],
    i: &mut usize,
) -> Result<&'a str, String> {
    if let Some(v) = inline {
        return Ok(v);
    }
    *i += 1;
    match args.get(*i) {
        Some(v) => Ok(v.as_str()),
        None => Err(format!("{flag} требует значение")),
    }
}

fn no_value(flag: &str, inline: Option<&str>) -> Result<(), String> {
    if inline.is_some() {
        return Err(format!("{flag} не принимает значение"));
    }
    Ok(())
}

fn number(flag: &str, s: &str) -> Result<usize, String> {
    s.parse::<usize>()
        .map_err(|_| format!("{flag}: ожидается целое число, получено «{s}»"))
}

pub fn parse(args: &[String]) -> Result<Action, String> {
    if args.is_empty() {
        return Ok(Action::Interactive);
    }

    let mut cfg = Config::default();
    let mut picked = false;
    let (mut upper, mut lower, mut digits, mut symbols) = (false, false, false, false);

    let mut i = 0;
    while i < args.len() {
        let raw = args[i].as_str();
        let (flag, inline): (&str, Option<&str>) = match raw.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f, Some(v)),
            _ => (raw, None),
        };

        match flag {
            "-h" | "--help" => return Ok(Action::Help),
            "-V" | "--version" => return Ok(Action::Version),
            "-l" | "--length" => {
                cfg.length = number(flag, take_value(flag, inline, args, &mut i)?)?;
            }
            "-c" | "--count" => {
                cfg.count = number(flag, take_value(flag, inline, args, &mut i)?)?;
            }
            "--exclude" => {
                let v = take_value(flag, inline, args, &mut i)?;
                cfg.charset.exclude.push_str(v);
            }
            "--upper" => {
                no_value(flag, inline)?;
                upper = true;
                picked = true;
            }
            "--lower" => {
                no_value(flag, inline)?;
                lower = true;
                picked = true;
            }
            "--digits" => {
                no_value(flag, inline)?;
                digits = true;
                picked = true;
            }
            "--symbols" => {
                no_value(flag, inline)?;
                symbols = true;
                picked = true;
            }
            "--no-repeat" => {
                no_value(flag, inline)?;
                cfg.no_repeat = true;
            }
            "--no-similar" => {
                no_value(flag, inline)?;
                cfg.charset.no_similar = true;
            }
            "--entropy" => {
                no_value(flag, inline)?;
                cfg.entropy = true;
            }
            "--clipboard" => {
                no_value(flag, inline)?;
                cfg.clipboard = true;
            }
            "-q" | "--quiet" => {
                no_value(flag, inline)?;
                cfg.quiet = true;
            }
            "--json" => {
                no_value(flag, inline)?;
                cfg.json = true;
            }
            _ => return Err(format!("неизвестный параметр: {raw}")),
        }
        i += 1;
    }

    if picked {
        cfg.charset.upper = upper;
        cfg.charset.lower = lower;
        cfg.charset.digits = digits;
        cfg.charset.symbols = symbols;
    }
    Ok(Action::Run(cfg))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn run(v: &[&str]) -> Config {
        match parse(&a(v)).unwrap() {
            Action::Run(c) => c,
            other => panic!("ожидался Run, получено {other:?}"),
        }
    }

    #[test]
    fn no_args_is_interactive() {
        assert!(matches!(parse(&a(&[])), Ok(Action::Interactive)));
    }

    #[test]
    fn help_and_version() {
        assert!(matches!(parse(&a(&["--help"])), Ok(Action::Help)));
        assert!(matches!(parse(&a(&["-V"])), Ok(Action::Version)));
    }

    #[test]
    fn length_separate_and_inline() {
        assert_eq!(run(&["--length", "32"]).length, 32);
        assert_eq!(run(&["--length=40"]).length, 40);
        assert_eq!(run(&["-l", "8"]).length, 8);
    }

    #[test]
    fn defaults() {
        let c = run(&["--count", "3"]);
        assert_eq!(c.length, 24);
        assert_eq!(c.count, 3);
        assert!(c.charset.upper && c.charset.lower && c.charset.digits && c.charset.symbols);
        assert!(!c.no_repeat && !c.json && !c.quiet);
    }

    #[test]
    fn selected_sets_replace_defaults() {
        let c = run(&["--upper", "--digits"]);
        assert!(c.charset.upper && c.charset.digits);
        assert!(!c.charset.lower && !c.charset.symbols);
    }

    #[test]
    fn boolean_flags() {
        let c = run(&[
            "--no-repeat",
            "--no-similar",
            "--entropy",
            "--json",
            "-q",
            "--clipboard",
        ]);
        assert!(c.no_repeat);
        assert!(c.charset.no_similar);
        assert!(c.entropy);
        assert!(c.json);
        assert!(c.quiet);
        assert!(c.clipboard);
    }

    #[test]
    fn exclude_value() {
        assert_eq!(run(&["--exclude", "abc"]).charset.exclude, "abc");
    }

    #[test]
    fn errors() {
        assert!(parse(&a(&["--bogus"])).is_err());
        assert!(parse(&a(&["--length"])).is_err());
        assert!(parse(&a(&["--length", "abc"])).is_err());
        assert!(parse(&a(&["--length", "-5"])).is_err());
        assert!(parse(&a(&["--json=1"])).is_err());
    }
