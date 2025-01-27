use charset::{self, Charset};

use crate::parser::{Ast, Node::*};
use log::warn;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    DecodeUtf8Error(#[from] std::str::Utf8Error),
    #[error(transparent)]
    DecodeBase64Error(#[from] base64::DecodeError),
    #[error(transparent)]
    DecodeQuotedPrintableError(#[from] quoted_printable::QuotedPrintableError),
}

fn decode_utf8(encoded_bytes: &Vec<u8>) -> Result<&str> {
    Ok(std::str::from_utf8(&encoded_bytes)?)
}

fn decode_base64(encoded_bytes: &Vec<u8>) -> Result<Vec<u8>> {
    let decoded_bytes = base64::decode(&encoded_bytes)?;
    Ok(decoded_bytes)
}

fn decode_quoted_printable(encoded_bytes: &Vec<u8>) -> Result<Vec<u8>> {
    let parse_mode = quoted_printable::ParseMode::Robust;

    const SPACE: u8 = ' ' as u8;
    const UNDERSCORE: u8 = '_' as u8;

    let encoded_bytes = encoded_bytes
        .iter()
        .map(|b| if *b == UNDERSCORE { SPACE } else { *b })
        .collect::<Vec<_>>();

    let decoded_bytes = quoted_printable::decode(encoded_bytes, parse_mode)?;

    Ok(decoded_bytes)
}

pub fn decode_with_encoding(
    encoding: char,
    encoded_bytes: &Vec<u8>,
) -> Result<Vec<u8>> {
    match encoding.to_uppercase().next() {
        Some('B') => decode_base64(encoded_bytes),
        Some('Q') | _ => decode_quoted_printable(encoded_bytes),
    }
}

pub fn decode_with_charset(
    charset: &Vec<u8>,
    decoded_bytes: &Vec<u8>,
) -> Result<String> {
    let decoded_str = match Charset::for_label(charset) {
        Some(charset) => charset.decode(decoded_bytes).0,
        None => charset::decode_ascii(decoded_bytes),
    };

    Ok(decoded_str.into_owned())
}

pub fn run(ast: &Ast) -> Result<String> {
    let mut output = String::new();

    for node in ast {
        match node {
            EncodedBytes(node) => {
                let decoded_str = match decode_with_encoding(node.encoding, &node.bytes) {
                    Ok(decoded_bytes) => {
                        match decode_with_charset(&node.charset, &decoded_bytes) {
                            Ok(decodecd_str) => decodecd_str,
                            Err(e) => {
                                warn!("failed to decode bytes to charset {:?} : {:?}", &node.charset, e);
                                String::from_utf8_lossy(&node.bytes).to_string()
                            }
                        }
                    },
                    Err(e) => {
                        warn!("failed to decode bytes from {}: {:?}", node.encoding, e);
                        String::from_utf8_lossy(&node.bytes).to_string()
                    }
                };
                output.push_str(&decoded_str);
            }
            ClearBytes(clear_bytes) => {
                match decode_utf8(&clear_bytes) {
                    Ok(clear_str) => {
                        output.push_str(clear_str);
                    },
                    Err(e) => {
                        warn!("failed to decode clear bytes to utf-8: {:?}", e);
                        output.push_str(&*String::from_utf8_lossy(&clear_bytes))
                    }
                }
            }
        }
    }

    Ok(output)
}
