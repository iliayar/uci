use std::collections::HashMap;

use clap::{Parser, ValueEnum};
use dynconf::*;
use reedline;

#[derive(Parser, Debug)]
#[command(about)]
struct Cli {
    /// Operational mode
    #[arg(short, long, default_value_t = CliMode::ParseFormatString)]
    mode: CliMode,

    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(ValueEnum, Debug, Clone)]
enum CliMode {
    ParseFormatString,
    EvalFormatString,
    EvalExpr,
    DeserializeExample,
}

impl std::fmt::Display for CliMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mode = match self {
            CliMode::ParseFormatString => "parse-fmt-str",
            CliMode::EvalFormatString => "eval-fmt-str",
            CliMode::EvalExpr => "eval-expr",
            CliMode::DeserializeExample => "deserialize-example",
        };
        write!(f, "{}", mode)
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Cli::parse();

    if let CliMode::DeserializeExample = args.mode {
        deserialize_example::run().await;
        return;
    }

    let mut line_editor = reedline::Reedline::create();
    let prompt = reedline::DefaultPrompt::default();

    let mut state = State::initialize();
    state.set_global(Value::Dict(HashMap::from_iter([
        ("a".to_string(), Value::String("boba".to_string())),
        ("b".to_string(), Value::Integer(123)),
        (
            "c".to_string(),
            Value::Dict(HashMap::from_iter([
                ("a".to_string(), Value::String("mogus".to_string())),
                ("b".to_string(), Value::String("ooba".to_string())),
                ("c".to_string(), Value::Integer(420)),
            ])),
        ),
        (
            "d".to_string(),
            Value::Array(vec![Value::String("amogus".to_string())]),
        ),
    ])));
    state.set_current_dir(std::env::current_dir().unwrap().to_path_buf());

    loop {
        let sig = line_editor.read_line(&prompt);
        match sig {
            Ok(reedline::Signal::Success(buffer)) => {
                match args.mode {
                    CliMode::ParseFormatString => {
                        parse_fmt_mode(buffer);
                    }
                    CliMode::EvalFormatString => {
                        eval_fmt_mode(&mut state, buffer).await;
                    }
                    CliMode::EvalExpr => {
                        eval_expr_mode(&mut state, buffer, args.json).await;
                    }
                    CliMode::DeserializeExample => unreachable!(),
                };
            }
            Ok(reedline::Signal::CtrlD) | Ok(reedline::Signal::CtrlC) => {
                println!("Exiting");
                break;
            }
            x => {
                println!("Event: {:?}", x);
            }
        }
    }
}

fn parse_fmt_mode(input: String) {
    match parse_string(&input) {
        Ok(ast) => {
            println!("{:?}", ast);
        }
        Err(err) => {
            println!("Failed to parse: {:?}", err);
        }
    }
}

async fn eval_fmt_mode<'a>(state: &mut State<'a>, input: String) {
    match eval_string(state, &input).await {
        Ok(value) => {
            println!("{}", value);
        }
        Err(err) => {
            println!("Failed to parse: {:?}", err);
        }
    }
}

async fn eval_expr_mode<'a>(state: &mut State<'a>, input: String, json: bool) {
    match eval_expr(state, &format!("${{{}}}", input)).await {
        Ok(value) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&value.to_json()).unwrap()
                );
            } else {
                println!("{:?}", value);
            }
        }
        Err(err) => {
            println!("Failed to parse: {:?}", err);
        }
    }
}

mod deserialize_example {
    use std::{collections::HashMap, path::PathBuf};

    use anyhow::Result;
    use dynconf::*;
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize)]
    struct RootConf {
        a: String,
        b: util::DynString,
        c: util::DynString,
        d: util::Dyn<ConfA>,
    }

    #[derive(Serialize)]
    struct RootConfR {
        a: String,
        b: String,
        c: String,
        d: ConfAR,
    }

    #[derive(Deserialize)]
    struct ConfA {
        a: i64,
        b: util::Dyn<ConfB>,
        c: util::DynString,
        d: util::Dyn<ConfD>,
    }

    #[derive(Serialize)]
    struct ConfAR {
        a: i64,
        b: ConfBR,
        c: String,
        d: ConfDR,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(transparent)]
    struct ConfB {
        values: HashMap<String, util::DynString>,
    }

    #[derive(Serialize)]
    #[serde(transparent)]
    struct ConfBR {
        values: HashMap<String, String>,
    }

    #[derive(Deserialize, Serialize)]
    struct ConfD {
        b: util::Lazy<util::Dyn<ConfB>>,
    }

    #[derive(Serialize)]
    struct ConfDR {
        b: util::LoadedLazy<util::Dyn<ConfB>>,
    }

    #[async_trait::async_trait]
    impl DynValue for RootConf {
        type Target = RootConfR;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            Ok(RootConfR {
                a: self.a,
                b: self.b.load(state).await?,
                c: self.c.load(state).await?,
                d: self.d.load(state).await?,
            })
        }
    }

    #[async_trait::async_trait]
    impl DynValue for ConfA {
        type Target = ConfAR;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            Ok(ConfAR {
                d: self.d.load(state).await?,
                a: self.a,
                b: self.b.load(state).await?,
                c: self.c.load(state).await?,
            })
        }
    }

    #[async_trait::async_trait]
    impl DynValue for ConfB {
        type Target = ConfBR;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            Ok(ConfBR {
                values: self.values.load(state).await?,
            })
        }
    }

    #[async_trait::async_trait]
    impl DynValue for ConfD {
        type Target = ConfDR;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            Ok(ConfDR {
                b: self.b.load(state).await?,
            })
        }
    }

    pub async fn run() {
        let mut state = State::initialize();

        let conf_loaded = util::load::<RootConf>(
            &mut state,
            PathBuf::from("./data/deserialize_example/root.yaml"),
        )
        .await
        .unwrap();

        println!(
            "{}",
            serde_json::to_string_pretty(&conf_loaded).expect("Failed to serialize conf")
        );

        println!(
            "{}",
            serde_json::to_string_pretty(
                &conf_loaded
                    .d
                    .d
                    .b
                    .load(&mut state)
                    .await
                    .expect("Failed to evaluate lazy")
            )
            .expect("Failed to serialize lazy conf")
        )
    }
}
