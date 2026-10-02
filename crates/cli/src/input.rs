//! Reading Disclosures and other input from arguments, files and stdin. The decoding
//! itself lives in `zdisclosure-core`, this module adds the filesystem and the size cap.

use std::fs::File;
use std::io::Read;

use zcash_disclosure::Disclosure;
use zdisclosure_core::input as core;

use crate::Error;

/// The largest file or stream the tools read into memory. A canonical Disclosure is bounded
/// far below this.
const MAX_INPUT: u64 = 16 * 1024 * 1024;

/// Reads the file at `path`, or stdin when `path` is `-`. Input larger than [`MAX_INPUT`]
/// is a usage error.
pub fn read_bytes(path: &str, stdin: &mut dyn Read) -> Result<Vec<u8>, Error> {
    let mut buf = vec![];
    match path {
        "-" => {
            stdin
                .take(MAX_INPUT + 1)
                .read_to_end(&mut buf)
                .map_err(|e| Error::usage(format!("cannot read stdin: {e}")))?;
        }
        _ => {
            File::open(path)
                .and_then(|f| f.take(MAX_INPUT + 1).read_to_end(&mut buf))
                .map_err(|e| Error::usage(format!("cannot read {path}: {e}")))?;
        }
    }
    if buf.len() as u64 > MAX_INPUT {
        let name = if path == "-" { "stdin" } else { path };
        return Err(Error::usage(format!(
            "{name} is larger than {MAX_INPUT} bytes"
        )));
    }
    Ok(buf)
}

/// Reads the Disclosures that `arg` names: a Disclosure string, `-` for stdin, or the path
/// of a `.zdisc` file.
pub fn read_disclosures(arg: &str, stdin: &mut dyn Read) -> Result<Vec<Disclosure>, Error> {
    if core::is_disclosure_string(arg) {
        return core::decode(arg).map_err(Error::usage).map(|d| vec![d]);
    }
    let bytes = read_bytes(arg, stdin)?;
    let text =
        String::from_utf8(bytes).map_err(|_| Error::usage(format!("{arg} is not UTF-8 text")))?;
    core::parse_text(&text).map_err(Error::usage)
}

#[cfg(test)]
mod tests {
    use zcash_disclosure::{Item, Kind, set};
    use zcash_protocol::TxId;

    use super::*;

    fn disclosure(txid: u8) -> Disclosure {
        Disclosure::new(
            zcash_protocol::consensus::NetworkType::Test,
            TxId::from_bytes([txid; 32]),
            vec![Item {
                kind: Kind::Sapling,
                index: 3,
                pk_d: [7; 32],
                secret: [9; 32],
            }],
        )
        .unwrap()
    }

    #[test]
    fn a_string_argument_decodes_without_touching_stdin() {
        let s = disclosure(1).encode();
        assert!(s.starts_with("zdutest1"));
        let mut stdin: &[u8] = b"unused";
        assert_eq!(
            read_disclosures(&s, &mut stdin).unwrap(),
            vec![disclosure(1)]
        );
    }

    #[test]
    fn stdin_takes_one_line_or_a_set() {
        let one = disclosure(1).encode();
        let mut stdin = one.as_bytes();
        assert_eq!(
            read_disclosures("-", &mut stdin).unwrap(),
            vec![disclosure(1)]
        );

        let set = set::format(&[disclosure(1), disclosure(2)]).unwrap();
        let mut stdin = set.as_bytes();
        assert_eq!(
            read_disclosures("-", &mut stdin).unwrap(),
            vec![disclosure(1), disclosure(2)]
        );
    }

    #[test]
    fn broken_input_is_a_usage_error() {
        let mut upper = disclosure(1).encode().to_uppercase();
        upper.push('\n');
        let mut stdin = upper.as_bytes();
        assert_eq!(
            read_disclosures("-", &mut stdin).unwrap_err().exit,
            crate::Exit::Usage
        );
        let mut empty: &[u8] = b"\n\n";
        assert!(read_disclosures("-", &mut empty).is_err());
    }

    #[test]
    fn oversized_input_is_refused() {
        let mut stdin = std::io::repeat(0).take(MAX_INPUT + 2);
        assert_eq!(
            read_disclosures("-", &mut stdin).unwrap_err().exit,
            crate::Exit::Usage
        );
    }
}
