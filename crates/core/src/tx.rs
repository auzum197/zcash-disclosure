//! Transaction bytes, as hex or binary, parsed under the consensus branch of a height.

use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::{BlockHeight, BranchId};

use crate::Error;
use crate::network::Params;

/// Decodes transaction bytes given as hex or as raw binary. Content that is all hex digits
/// after trimming decodes as hex, anything else is taken as it is.
pub fn from_hex_or_bytes(raw: Vec<u8>) -> Result<Vec<u8>, Error> {
    let trimmed = raw.trim_ascii();
    if !trimmed.is_empty() && trimmed.iter().all(u8::is_ascii_hexdigit) {
        hex::decode(trimmed).map_err(|e| Error::new(format!("invalid transaction hex: {e}")))
    } else {
        Ok(raw)
    }
}

/// Parses a transaction mined at `height`. The height picks the consensus branch that a v4
/// transaction is read under.
pub fn parse(params: &Params, bytes: &[u8], height: BlockHeight) -> Result<Transaction, Error> {
    let mut rest = bytes;
    let tx = Transaction::read(&mut rest, BranchId::for_height(params, height))
        .map_err(|e| Error::new(format!("cannot parse the transaction: {e}")))?;
    if !rest.is_empty() {
        return Err(Error::new(format!(
            "{} bytes follow the end of the transaction",
            rest.len()
        )));
    }
    Ok(tx)
}

#[cfg(test)]
mod tests {
    use super::from_hex_or_bytes;

    #[test]
    fn hex_and_binary_are_told_apart() {
        assert_eq!(
            from_hex_or_bytes(b" 0500ff\n".to_vec()).unwrap(),
            vec![0x05, 0x00, 0xff]
        );
        assert_eq!(
            from_hex_or_bytes(vec![0x05, 0x00, 0xff]).unwrap(),
            vec![0x05, 0x00, 0xff]
        );
        assert!(from_hex_or_bytes(b"abc".to_vec()).is_err());
    }
}
