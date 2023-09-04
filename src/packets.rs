use crate::markov::MAX_PACKETS;
use serde::{Deserialize, Serialize};
use serde_with::formats::CommaSeparator;
use serde_with::serde_as;
use serde_with::StringWithSeparator;
use std::cmp::min;
use std::collections::BTreeMap;
use std::fmt::Display;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
#[serde_as]
#[derive(Debug, PartialEq, Eq, Clone, Hash, Default, Serialize, Deserialize)]
pub struct Packets {
    #[serde_as(as = "[StringWithSeparator::<CommaSeparator, u8>; MAX_PACKETS]")]
    pub(crate) inner: [Vec<u8>; MAX_PACKETS],
}
impl Display for Packets {
    // Hex dump the packets
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        for i in 0..MAX_PACKETS {
            if !self.inner[i].is_empty() {
                s.push_str(hex::encode(&self.inner[i]).as_str());
                s.push('\n');
            }
        }
        write!(f, "{}", s)
    }
}
impl Packets {
    pub fn append(&mut self, packet: &mut Vec<u8>) {
        // Search the first free slot and insert it there
        let size = self.size();
        if size < MAX_PACKETS {
            self.inner[size] = packet.clone();
        }
    }
    pub fn is_full(&self) -> bool {
        self.inner.iter().all(|x| !x.is_empty())
    }
    pub fn size(&self) -> usize {
        min(1, self.inner.iter().filter(|x| !x.is_empty()).count())
    }
    pub fn new() -> Self {
        Self {
            inner: Default::default(),
        }
    }
}

#[serde_as]
#[derive(Debug, PartialEq, Eq, Clone, Hash, Default, Serialize, Deserialize)]
pub struct PacketQueue {
    #[serde_as(as = "BTreeMap<StringWithSeparator::<CommaSeparator, u8>, _>")]
    pub(crate) inner: BTreeMap<Vec<u8>, Packets>,
}

impl PacketQueue {
    pub async fn read_from_file(path: impl AsRef<Path>) -> color_eyre::Result<Self> {
        let mut content = String::new();
        File::open(path).await?.read_to_string(&mut content).await?;
        let queue = toml::from_str(&content)?;
        Ok(queue)
    }
}
