//! 32-byte opaque identifiers for ontology entities.
//!
//! The construction model mirrors `atomr-dledger-types::id`: each ID is
//! a `[u8; 32]` newtype, hex-encoded for display, and derived from a
//! v4 UUID hashed with Blake3 using a domain-separated key so the
//! crate avoids depending on a CSPRNG directly.

use core::fmt;
use core::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::Zeroize;

/// Errors raised by ID parsing.
#[derive(Debug, Error)]
pub enum IdError {
    /// The provided string was not valid for the target ID type.
    #[error("invalid id: {0}")]
    Invalid(String),
}

macro_rules! define_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize, Zeroize)]
        #[serde(transparent)]
        pub struct $name(#[serde(with = "serde_bytes_array")] pub [u8; 32]);

        impl $name {
            /// Wrap a raw 32-byte array as this id without validation.
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            /// Borrow the underlying 32 bytes.
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            /// Move the 32 bytes out of the wrapper.
            pub const fn into_bytes(self) -> [u8; 32] {
                self.0
            }

            /// Derive a fresh id from a v4 UUID hashed with Blake3.
            ///
            /// The UUID provides entropy; Blake3 widens it to 32 bytes
            /// with a domain-separated key so different ID types never
            /// collide on the same UUID input.
            pub fn new_random() -> Self {
                let raw = uuid::Uuid::new_v4();
                let mut hasher =
                    blake3::Hasher::new_derive_key(concat!("atomr-ontology-core/id/v1/", stringify!($name)));
                hasher.update(raw.as_bytes());
                let out = hasher.finalize();
                let mut bytes = [0u8; 32];
                bytes.copy_from_slice(out.as_bytes());
                Self(bytes)
            }

            /// Content-addressed id derived deterministically from
            /// the supplied bytes. Identical input → identical id.
            ///
            /// Useful for nodes whose identity is defined by their
            /// canonical IRI or by a hash of their properties.
            pub fn content_address(input: &[u8]) -> Self {
                let mut hasher = blake3::Hasher::new_derive_key(concat!(
                    "atomr-ontology-core/id/v1/",
                    stringify!($name),
                    "/content"
                ));
                hasher.update(input);
                let out = hasher.finalize();
                let mut bytes = [0u8; 32];
                bytes.copy_from_slice(out.as_bytes());
                Self(bytes)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), hex::encode(self.0))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&hex::encode(self.0))
            }
        }

        impl FromStr for $name {
            type Err = IdError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let raw = hex::decode(s).map_err(|e| IdError::Invalid(e.to_string()))?;
                if raw.len() != 32 {
                    return Err(IdError::Invalid(format!("expected 32 bytes, got {}", raw.len())));
                }
                let mut bytes = [0u8; 32];
                bytes.copy_from_slice(&raw);
                Ok(Self(bytes))
            }
        }

        impl From<[u8; 32]> for $name {
            fn from(b: [u8; 32]) -> Self {
                Self(b)
            }
        }

        impl AsRef<[u8]> for $name {
            fn as_ref(&self) -> &[u8] {
                &self.0
            }
        }
    };
}

define_id!(NodeId, "Opaque identifier for a labeled-property-graph node.");
define_id!(EdgeId, "Opaque identifier for a labeled-property-graph edge.");
define_id!(RecordId, "Opaque identifier for a flat record snapshot of a node and its properties.");
define_id!(ProvenanceId, "Opaque identifier for a provenance assertion (PROV-O activity / derivation).");

pub(crate) mod serde_bytes_array {
    use serde::{Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
        serde_bytes::Bytes::new(bytes).serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        let buf: &[u8] = serde_bytes::deserialize(d)?;
        if buf.len() != 32 {
            return Err(serde::de::Error::invalid_length(buf.len(), &"32 bytes"));
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(buf);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_ids_are_distinct() {
        let a = NodeId::new_random();
        let b = NodeId::new_random();
        assert_ne!(a, b);
    }

    #[test]
    fn content_address_is_deterministic() {
        let a = NodeId::content_address(b"https://example.org/Acme");
        let b = NodeId::content_address(b"https://example.org/Acme");
        assert_eq!(a, b);
    }

    #[test]
    fn content_address_differs_per_id_type() {
        let n = NodeId::content_address(b"x");
        let e = EdgeId::content_address(b"x");
        // Same input, different domain keys → different bytes.
        assert_ne!(n.as_bytes(), e.as_bytes());
    }

    #[test]
    fn round_trip_hex() {
        let id = NodeId::new_random();
        let parsed: NodeId = id.to_string().parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn parse_rejects_wrong_length() {
        assert!("deadbeef".parse::<NodeId>().is_err());
    }
}
