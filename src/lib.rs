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

// Errors and warnings are currently not handled
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum KataResponse {
    #[serde(rename_all = "camelCase")]
    Result {
        id: String,
        is_during_search: bool,
        move_infos: Vec<MoveInfo>,
        root_info: RootInfo,
        #[serde(default)]
        ownership: Option<Vec<f32>>,
        #[serde(default)]
        ownership_stdev: Option<Vec<f32>>,
        #[serde(default)]
        policy: Option<Vec<f32>>,
    },

    #[serde(rename_all = "camelCase")]
    Resultless {
        id: String,
        is_during_search: bool,
        turn_number: u16,
        no_results: bool,
    },
    #[serde(rename_all = "camelCase")]
    TerminateAck {
        id: String,
        action: ActionTerminate,
        #[serde(default)]
        turn_number: Option<u16>,
        terminate_id: String,
    },
    Version {
        action: ActionQueryVersion,
        git_hash: String,
        id: String,
        version: String,
    },
    CacheCleared {
        id: String,
        action: ActionClearCache,
    },
}

#[derive(Clone, Deserialize, Debug)]
pub enum GitHashOmitted {
    #[serde(rename = "<omitted>")]
    Omitted,
}

#[derive(Clone, Deserialize, Debug)]
#[serde(untagged)]
pub enum GitHash {
    Omitted(GitHashOmitted),
    Included(String),
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RootInfo {
    pub winrate: f32,
    pub score_lead: f32,
    pub score_selfplay: f32,
    #[serde(default)]
    pub utility: Option<f32>,
    pub visits: u32,
    #[serde(default)]
    pub this_hash: Option<String>,
    #[serde(default)]
    pub sym_hash: Option<String>,
    #[serde(default)]
    pub current_player: Option<Player>,
    // It is unclear then katago includes this fields in the response, and even are they optional,
    // so they are ignored currently
    //pub raw_st_wr_error: f32,
    //pub raw_st_score_error: f32,
    //pub raw_var_time_left: u32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MoveInfo {
    pub r#move: String,
    pub winrate: f32,
    pub visits: u32,
    pub score_lead: f32,
    pub score_selfplay: f32,
    pub score_stdev: f32,
    pub prior: f32,
    pub utility: f32,
    pub lcb: f32,
    pub utility_lcb: f32,
    pub order: u16,
    pub is_symmetry_of: Option<String>,
    pub pv: Vec<String>,
    #[serde(default)]
    pub pv_visits: Option<Vec<u32>>,
    #[serde(default)]
    pub pv_edge_visits: Option<Vec<u32>>,
    #[serde(default)]
    pub ownership: Option<Vec<f32>>,
    #[serde(default)]
    pub ownership_stdev: Option<Vec<f32>>,
}

#[serde_with::skip_serializing_none]
#[derive(Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum KataAction {
    Query {
        #[serde(flatten)]
        inner: KataQuery,
    },
    QueryVersion {
        id: String,
        action: ActionQueryVersion,
    },
    ClearCache {
        id: String,
        action: ActionClearCache,
    },
    #[serde(rename_all = "camelCase")]
    Terminate {
        id: String,
        action: ActionTerminate,
        terminate_id: String,
        turn_numbers: Option<Vec<u16>>,
    },
}

#[derive(Serialize, Clone, Debug, Deserialize)]
pub enum ActionQueryVersion {
    #[serde(rename = "query_version")]
    ActionQueryVersion,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum ActionClearCache {
    #[serde(rename = "clear_cache")]
    ActionClearCache,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ActionTerminate {
    #[serde(rename = "terminate")]
    ActionTerminate,
}

#[serde_with::skip_serializing_none]
#[derive(Serialize, Clone, Debug, Builder)]
#[builder(setter(into))]
#[serde(rename_all = "camelCase")]
pub struct KataQuery {
    id: String,
    #[builder(default)]
    initial_stones: Option<Vec<(Player, String)>>,
    moves: Vec<(Player, String)>,
    // Passing custom rule set is not yet supported, only shorthands can be passed at the moment
    rules: Rules,
    #[builder(default)]
    initial_player: Option<Player>,
    #[builder(default)]
    komi: Option<f32>,
    #[builder(default)]
    white_handicap_bonus: Option<WhiteHandicapBonus>,
    board_x_size: u8,
    board_y_size: u8,
    #[builder(default)]
    analyze_turns: Option<Vec<u16>>,
    #[builder(default)]
    max_visits: Option<u32>,
    #[builder(default)]
    root_policy_temperature: Option<f32>,
    #[builder(default)]
    root_fpu_reduction_max: Option<f32>,
    #[builder(default)]
    anaysis_pv_len: Option<u16>,
    #[builder(default)]
    include_ownership: Option<bool>,
    #[builder(default)]
    inlcude_ownership_stdev: Option<bool>,
    #[builder(default)]
    include_moves_ownership: Option<bool>,
    #[builder(default)]
    include_moves_ownership_stdev: Option<bool>,
    #[builder(default)]
    include_policy: Option<bool>,
    #[builder(default)]
    include_pv_visits: Option<bool>,
    #[builder(default)]
    avoid_moves: Option<Vec<MoveGroup>>,
    #[builder(default)]
    allow_moves: Option<[MoveGroup; 1]>,
    // TODO: Maybe use HashMap here instead of Value?
    #[builder(default)]
    override_settings: Option<serde_json::Value>,
    #[builder(default)]
    report_during_search_every: Option<f32>,
    #[builder(default)]
    priority: Option<i32>,
    #[builder(default)]
    priorities: Option<Vec<i32>>,
}

impl KataQuery {
    pub fn builder() -> KataQueryBuilder {
        Default::default()
    }
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

#[derive(Serialize, Debug, Clone, Deserialize)]
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

pub fn start(
    cmd: &mut Command,
) -> (
    impl Sink<KataAction, Error = impl Error>,
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
        FramedWrite::new(stdin, KataActionEncoder),
        LinesStream::new(stdout.lines())
            .map(|x| x.unwrap())
            .map(|line| serde_json::from_str::<KataResponse>(&line).unwrap()),
    )
}

struct KataActionEncoder;

impl Encoder<KataAction> for KataActionEncoder {
    type Error = std::io::Error;

    fn encode(&mut self, item: KataAction, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend_from_slice(serde_json::to_vec(&item).unwrap().as_slice());
        dst.extend_from_slice(b"\n");
        Ok(())
    }
}
