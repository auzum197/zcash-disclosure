//! Obtaining the transaction a Disclosure covers, with the height of its block.

use std::collections::BTreeMap;
use std::io::Read;
use std::time::Duration;

use clap::Args;
use lightwallet_core::{
    CanonicalIdentityClient, CanonicalIndexerClient, IdentityTransport, NetworkParams, Txid,
};
use tokio::runtime::Runtime;
use tonic::transport::{ClientTlsConfig, Endpoint};
use zcash_primitives::transaction::Transaction;
use zcash_protocol::TxId;
use zcash_protocol::consensus::{BlockHeight, BranchId};

use crate::network::Params;
use crate::{Error, input};

/// The transaction flags every tool takes.
#[derive(Args, Clone, Debug, Default)]
pub struct TxArgs {
    /// A raw transaction, as hex or binary. `-` reads stdin. Needs --height.
    #[arg(
        long,
        value_name = "PATH",
        requires = "height",
        conflicts_with = "lightwalletd"
    )]
    pub tx: Option<String>,

    /// The height of the block that contains --tx. For a mempool transaction, the height of
    /// the next block.
    #[arg(long, requires = "tx")]
    pub height: Option<u32>,

    /// A lightwalletd server to fetch the transaction and its height from, such as
    /// https://zec.rocks:443.
    #[arg(long, value_name = "URL")]
    pub lightwalletd: Option<String>,
}

impl TxArgs {
    /// Whether the transaction comes from stdin.
    pub fn reads_stdin(&self) -> bool {
        self.tx.as_deref() == Some("-")
    }
}

/// A transaction with its place in the chain.
#[derive(Debug)]
pub struct Fetched {
    /// The transaction.
    pub tx: Transaction,
    /// The height of the block that contains it, or of the next block for a mempool
    /// transaction.
    pub height: BlockHeight,
    /// The number of blocks from the one that contains it to the tip, counting both. A
    /// mempool transaction has depth 0. Known only from lightwalletd.
    pub depth: Option<u64>,
}

/// Where transactions come from.
pub enum Source {
    /// One raw transaction given on the command line.
    Raw {
        /// The serialized transaction.
        bytes: Vec<u8>,
        /// The height given with it.
        height: BlockHeight,
    },
    /// A lightwalletd server.
    Lightwalletd(Box<Lightwalletd>),
}

impl Source {
    /// Opens the source that `args` names, or returns `None` when it names none.
    pub fn open(args: &TxArgs, stdin: &mut dyn Read) -> Result<Option<Self>, Error> {
        if let (Some(path), Some(height)) = (&args.tx, args.height) {
            let bytes = tx_bytes(input::read_bytes(path, stdin)?)?;
            return Ok(Some(Source::Raw {
                bytes,
                height: BlockHeight::from_u32(height),
            }));
        }
        args.lightwalletd
            .as_deref()
            .map(|url| Lightwalletd::new(url).map(|l| Source::Lightwalletd(Box::new(l))))
            .transpose()
    }

    /// Obtains a transaction. A lightwalletd source needs `txid`. A raw source returns its
    /// one transaction whatever `txid` is, and the caller compares the txids.
    pub fn fetch(&self, params: &Params, txid: Option<TxId>) -> Result<Fetched, Error> {
        match self {
            Source::Raw { bytes, height } => Ok(Fetched {
                tx: parse(params, bytes, *height).map_err(Error::usage)?,
                height: *height,
                depth: None,
            }),
            Source::Lightwalletd(server) => {
                let txid = txid.ok_or_else(|| Error::usage("--lightwalletd needs a txid"))?;
                let fetched = server.fetch(params, txid)?;
                if fetched.tx.txid() != txid {
                    return Err(Error::unavailable(format!(
                        "the server returned transaction {} for {txid}",
                        fetched.tx.txid()
                    )));
                }
                Ok(fetched)
            }
        }
    }
}

/// Parses a transaction mined at `height`. The height picks the consensus branch that a v4
/// transaction is read under.
pub fn parse(params: &Params, bytes: &[u8], height: BlockHeight) -> Result<Transaction, String> {
    let mut rest = bytes;
    let tx = Transaction::read(&mut rest, BranchId::for_height(params, height))
        .map_err(|e| format!("cannot parse the transaction: {e}"))?;
    if !rest.is_empty() {
        return Err(format!(
            "{} bytes follow the end of the transaction",
            rest.len()
        ));
    }
    Ok(tx)
}

/// Decodes transaction bytes given as hex or as raw binary.
fn tx_bytes(raw: Vec<u8>) -> Result<Vec<u8>, Error> {
    let trimmed = raw.trim_ascii();
    if !trimmed.is_empty() && trimmed.iter().all(u8::is_ascii_hexdigit) {
        hex::decode(trimmed).map_err(|e| Error::usage(format!("invalid transaction hex: {e}")))
    } else {
        Ok(raw)
    }
}

/// A lightwalletd server, reached over the canonical protocol.
pub struct Lightwalletd {
    rt: Runtime,
    endpoint: Endpoint,
}

impl Lightwalletd {
    fn new(url: &str) -> Result<Self, Error> {
        let mut endpoint = Endpoint::from_shared(url.to_owned())
            .map_err(|e| Error::usage(format!("invalid --lightwalletd url {url}: {e}")))?
            .connect_timeout(Duration::from_secs(30));
        if url.starts_with("https://") {
            endpoint = endpoint
                .tls_config(ClientTlsConfig::new().with_webpki_roots())
                .map_err(|e| Error::usage(format!("TLS configuration: {e}")))?;
        }
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| Error::unavailable(format!("cannot start the runtime: {e}")))?;
        Ok(Self { rt, endpoint })
    }

    fn fetch(&self, params: &Params, txid: TxId) -> Result<Fetched, Error> {
        let unavailable = |e: lightwallet_core::Error| Error::unavailable(format!("{txid}: {e}"));
        let (raw, tip) = self.rt.block_on(async {
            // The txid request and the tip request go over separate connections, as
            // lightwallet-core asks of identity-bearing calls.
            let identity = CanonicalIdentityClient::new(IdentityTransport::connect_lazy(
                self.endpoint.clone(),
            ));
            let raw = identity
                .get_transaction(Txid::new(txid.as_ref().to_vec()))
                .await
                .map_err(unavailable)?;
            let indexer = CanonicalIndexerClient::new(
                self.endpoint.connect_lazy(),
                NetworkParams {
                    chain_name: String::new(),
                    activation_heights: BTreeMap::new(),
                    consensus_branch_id: 0,
                },
            );
            let tip = indexer
                .get_latest_block()
                .await
                .map_err(unavailable)?
                .height;
            Ok::<_, Error>((raw, tip))
        })?;

        let (height, depth) = match raw.height {
            0 => (tip + 1, 0),
            u64::MAX => {
                return Err(Error::unavailable(format!(
                    "{txid} is mined only on a fork outside the main chain"
                )));
            }
            h => (h, (tip + 1).saturating_sub(h)),
        };
        let height = u32::try_from(height)
            .map(BlockHeight::from_u32)
            .map_err(|_| Error::unavailable(format!("{txid}: height {height} is out of range")))?;
        let tx = parse(params, &raw.data, height)
            .map_err(|e| Error::unavailable(format!("{txid}: {e}")))?;
        Ok(Fetched {
            tx,
            height,
            depth: Some(depth),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::tx_bytes;

    #[test]
    fn hex_and_binary_are_told_apart() {
        assert_eq!(
            tx_bytes(b" 0500ff\n".to_vec()).unwrap(),
            vec![0x05, 0x00, 0xff]
        );
        assert_eq!(
            tx_bytes(vec![0x05, 0x00, 0xff]).unwrap(),
            vec![0x05, 0x00, 0xff]
        );
        assert!(tx_bytes(b"abc".to_vec()).is_err());
    }
}
