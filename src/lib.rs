use std::error::Error;
use std::process::Stdio;

use bytes::BytesMut;
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

#[derive(Serialize, Clone, Debug)]
pub struct KataQuery;

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
