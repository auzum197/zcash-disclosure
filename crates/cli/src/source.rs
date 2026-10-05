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
use zcash_protocol::consensus::BlockHeight;
use zdisclosure_core::tx;

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
            let bytes =
                tx::from_hex_or_bytes(input::read_bytes(path, stdin)?).map_err(Error::usage)?;
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
                tx: tx::parse(params, bytes, *height).map_err(Error::usage)?,
                height: *height,
                depth: None,
            }),
            Source::Lightwalletd(server) => {
                let txid = txid.ok_or_else(|| Error::usage("--lightwalletd needs a txid"))?;
                let fetched = server.fetch(params, txid)?;
                if fetched.tx.txid() != txid {
                    return Err(Error::failed(format!(
                        "the server returned transaction {} for {txid}",
                        fetched.tx.txid()
                    )));
                }
                Ok(fetched)
            }
        }
    }
}

/// A lightwalletd server, reached over the canonical protocol.
pub struct Lightwalletd {
    rt: Runtime,
    endpoint: Endpoint,
}

/// How long one server call may take before the transaction counts as unobtainable.
const RPC_TIMEOUT: Duration = Duration::from_secs(30);

/// Whether `url` points at this machine, where plaintext http does not leave it.
fn loopback(url: &str) -> bool {
    let rest = url.strip_prefix("http://").unwrap_or(url);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host = authority
        .rsplit_once(':')
        .map_or(authority, |(host, _)| host)
        .trim_matches(|c| c == '[' || c == ']');
    matches!(host, "localhost" | "127.0.0.1" | "::1")
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
        } else if !loopback(url) {
            return Err(Error::usage(format!(
                "--lightwalletd must use https:// for a server off this machine: {url}"
            )));
        }
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| Error::unavailable(format!("cannot start the runtime: {e}")))?;
        Ok(Self { rt, endpoint })
    }

    fn fetch(&self, params: &Params, txid: TxId) -> Result<Fetched, Error> {
        let (data, reported, tip) = match self
            .rt
            .block_on(tokio::time::timeout(RPC_TIMEOUT, self.raw_and_tip(txid)))
        {
            Ok(inside) => inside?,
            Err(_) => {
                return Err(Error::unavailable(format!(
                    "{txid}: the server did not answer within {RPC_TIMEOUT:?}"
                )));
            }
        };
        let (height, depth) = place(txid, reported, tip)?;
        let tx = tx::parse(params, &data, height)
            .map_err(|e| Error::unavailable(format!("{txid}: {e}")))?;
        Ok(Fetched {
            tx,
            height,
            depth: Some(depth),
        })
    }

    /// Asks the server for the raw transaction and the height of its tip, as
    /// `(data, reported height, tip)`. The two requests go over separate connections, as
    /// lightwallet-core asks of identity-bearing calls.
    async fn raw_and_tip(&self, txid: TxId) -> Result<(Vec<u8>, u64, u64), Error> {
        let unavailable = |e: lightwallet_core::Error| Error::unavailable(format!("{txid}: {e}"));
        let identity =
            CanonicalIdentityClient::new(IdentityTransport::connect_lazy(self.endpoint.clone()));
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
        Ok((raw.data, raw.height, tip))
    }
}

/// Where a transaction the server reported at `reported` sits in the chain it reported
/// with `tip`, as `(height, depth)`. Height 0 is the mempool, and depth counts both
/// ends. The two numbers come from the same server over two connections, so pairs that
/// cannot describe one chain are rejected.
fn place(txid: TxId, reported: u64, tip: u64) -> Result<(BlockHeight, u64), Error> {
    let next = tip
        .checked_add(1)
        .ok_or_else(|| Error::unavailable(format!("{txid}: the server reports tip {tip}")))?;
    let (height, depth) = match reported {
        0 => (next, 0),
        u64::MAX => {
            return Err(Error::unavailable(format!(
                "{txid} is mined only on a fork outside the main chain"
            )));
        }
        h if h > tip => {
            return Err(Error::unavailable(format!(
                "{txid}: the server reports height {h} above its tip {tip}"
            )));
        }
        h => (h, next - h),
    };
    let height = u32::try_from(height)
        .map(BlockHeight::from_u32)
        .map_err(|_| Error::unavailable(format!("{txid}: height {height} is out of range")))?;
    Ok((height, depth))
}

#[cfg(test)]
mod tests {
    use super::loopback;

    #[test]
    fn only_local_urls_count_as_loopback() {
        assert!(loopback("http://localhost:9067"));
        assert!(loopback("http://127.0.0.1:9067"));
        assert!(loopback("http://[::1]:9067"));
        assert!(!loopback("http://zec.rocks:443"));
        assert!(!loopback("http://localhost.example:9067"));
    }
}
