use std::{collections::HashMap, path::PathBuf};

use crate::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct RootConf {
    a: String,
    b: util::DynString,
    c: util::DynString,
    d: util::Dyn<ConfA>,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
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

#[derive(Serialize, Debug, PartialEq, Eq)]
struct ConfAR {
    a: i64,
    b: ConfBR,
    c: String,
    d: ConfDR,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
#[serde(transparent)]
struct ConfB {
    values: HashMap<String, util::DynString>,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(transparent)]
struct ConfBR {
    values: HashMap<String, String>,
}

#[derive(Deserialize, Serialize)]
struct ConfD {
    b: util::Lazy<util::Dyn<ConfB>>,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
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

#[tokio::test]
async fn test_deserialize() {
    let mut state = State::initialize();

    let conf_loaded = util::load::<RootConf>(
        &mut state,
        PathBuf::from("./test_data/deserialize/root.yaml"),
    )
    .await
    .unwrap();

    assert_eq!(
        conf_loaded,
        RootConfR {
            a: "123".to_string(),
            b: "aboba-123".to_string(),
            c: "./test_data/deserialize/.".to_string(),
            d: ConfAR {
                a: 123,
                b: ConfBR {
                    values: HashMap::from_iter([
                        ("c".to_string(), "!23".to_string()),
                        ("a".to_string(), "gg".to_string()),
                        ("b".to_string(), "567".to_string()),
                    ])
                },
                c: "BOOBA".to_string(),
                d: ConfDR {
                    b: util::LoadedLazy {
                        value: serde_yaml::Value::String("${${load(./nested/b.yaml)}}".to_string()),
                        current_dir: Some("./test_data/deserialize/other_nested".into()),
                        _phantom: std::marker::PhantomData::default(),
                    }
                }
            }
        }
    );

    assert_eq!(
        conf_loaded.d.d.b.load(&mut state).await.unwrap(),
        ConfBR {
            values: HashMap::from_iter([("abo".to_string(), "ba".to_string())])
        }
    )
}
