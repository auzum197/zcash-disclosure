//! Reading Disclosures and other input from arguments, files and stdin.

use std::fs;
use std::io::Read;

use zcash_disclosure::{Disclosure, set};
use zcash_protocol::consensus::NetworkType;

use crate::Error;

/// Reads the file at `path`, or stdin when `path` is `-`.
pub fn read_bytes(path: &str, stdin: &mut dyn Read) -> Result<Vec<u8>, Error> {
    if path == "-" {
        let mut buf = vec![];
        stdin
            .read_to_end(&mut buf)
            .map_err(|e| Error::usage(format!("cannot read stdin: {e}")))?;
        Ok(buf)
    } else {
        fs::read(path).map_err(|e| Error::usage(format!("cannot read {path}: {e}")))
    }
}

/// Reads the Disclosures that `arg` names: a Disclosure string, `-` for stdin, or the path
/// of a `.zdisc` file.
pub fn read_disclosures(arg: &str, stdin: &mut dyn Read) -> Result<Vec<Disclosure>, Error> {
    if is_disclosure_string(arg) {
        return decode(arg).map(|d| vec![d]);
    }
    let bytes = read_bytes(arg, stdin)?;
    let text =
        String::from_utf8(bytes).map_err(|_| Error::usage(format!("{arg} is not UTF-8 text")))?;
    parse_text(&text)
}

/// Parses text that holds either one Disclosure or a Disclosure set.
pub fn parse_text(text: &str) -> Result<Vec<Disclosure>, Error> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    match (lines.next(), lines.next()) {
        (None, _) => Err(Error::usage("no disclosure in the input")),
        (Some(line), None) => decode(line.trim()).map(|d| vec![d]),
        _ => set::parse(text).map_err(|e| Error::usage(format!("invalid disclosure set: {e}"))),
    }
}

fn decode(s: &str) -> Result<Disclosure, Error> {
    Disclosure::decode(s).map_err(|e| Error::usage(format!("invalid disclosure: {e}")))
}

fn is_disclosure_string(arg: &str) -> bool {
    [NetworkType::Main, NetworkType::Test, NetworkType::Regtest]
        .iter()
        .any(|&n| arg.starts_with(&format!("{}1", zcash_disclosure::hrp(n))))
}

#[cfg(test)]
mod tests {
    use zcash_disclosure::{Item, Kind};
    use zcash_protocol::TxId;

    use super::*;

    fn disclosure(txid: u8) -> Disclosure {
        Disclosure::new(
            NetworkType::Test,
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
        assert!(s.starts_with("zdisctest1"));
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
}
