use std::error::Error;
use std::process::Stdio;

use bytes::BytesMut;
use derive_builder::Builder;
use futures_core::Stream;
use futures_sink::Sink;
use futures_util::StreamExt;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::LinesStream;
use tokio_util::codec::{Encoder, FramedWrite};

#[derive(Deserialize, Clone, Debug)]
pub struct KataResponse;

#[derive(Serialize, Clone, Debug, Builder)]
#[builder(setter(into))]
#[serde(rename_all = "camelCase")]
pub struct KataQuery {
    id: String,
    initial_stones: Option<Vec<(Player, String)>>,
    moves: Vec<(Player, String)>,
    // Passing custom rule set is not yet supported, only shorthands can be passed at the moment
    rules: Rules,
    initial_player: Option<Player>,
    komi: Option<f32>,
    white_handicap_bonus: Option<WhiteHandicapBonus>,
    board_x_size: u8,
    board_y_size: u8,
    analyze_turns: Option<Vec<u16>>,
    max_visits: Option<u32>,
    root_policy_temperature: Option<f32>,
    root_fpu_reduction_max: Option<f32>,
    anaysis_pv_len: Option<u16>,
    include_ownership: Option<bool>,
    inlcude_ownership_stdev: Option<bool>,
    include_moves_ownership: Option<bool>,
    include_moves_ownership_stdev: Option<bool>,
    include_policy: Option<bool>,
    include_pv_visits: Option<bool>,
    avoid_moves: Option<Vec<MoveGroup>>,
    allow_moves: Option<[MoveGroup; 1]>,
    // TODO: Maybe use HashMap here instead of Value?
    override_settings: Option<serde_json::Value>,
    report_during_search_every: Option<f32>,
    priority: Option<i32>,
    priorities: Option<Vec<i32>>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MoveGroup {
    player: Player,
    moves: Vec<String>,
    until_depth: u32,
}

#[derive(Serialize, Debug, Clone)]
pub enum WhiteHandicapBonus {
    #[serde(rename = "0")]
    Zero,
    N,
    #[serde(rename = "N-1")]
    NMinusOne,
}

#[derive(Serialize, Debug, Clone)]
pub enum Player {
    #[serde(rename = "B")]
    Black,
    #[serde(rename = "W")]
    White,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum Rules {
    TrompTaylor,
    Chinese,
    ChineseOgs,
    ChineseKgs,
    Japanese,
    Korean,
    StoneScoring,
    Aga,
    Bga,
    NewZealand,
    AgaButton,
}

impl KataQuery {
    pub fn builder() -> KataQueryBuilder {
        Default::default()
    }
}

pub fn start(
    cmd: &mut Command,
) -> (
    impl Sink<KataQuery, Error = impl Error>,
    impl Stream<Item = KataResponse>,
) {
    let mut handle = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let stdin = handle.stdin.unwrap();
    let stdout = BufReader::new(handle.stdout.take().unwrap());

    (
        FramedWrite::new(stdin, KataQueryEncoder),
        LinesStream::new(stdout.lines())
            .map(|x| x.unwrap())
            .map(|line| serde_json::from_str::<KataResponse>(&line).unwrap()),
    )
}

struct KataQueryEncoder;

impl Encoder<KataQuery> for KataQueryEncoder {
    type Error = std::io::Error;

    fn encode(&mut self, item: KataQuery, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend_from_slice(serde_json::to_vec(&item).unwrap().as_slice());
        Ok(())
    }
}
