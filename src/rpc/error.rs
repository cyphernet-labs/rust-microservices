// LNP/BP Core Library implementing LNPBP specifications & standards
// Written in 2020 by
//     Dr. Maxim Orlovsky <orlovsky@pandoracore.com>
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the MIT License
// along with this software.
// If not, see <https://opensource.org/licenses/MIT>.

use std::fmt::{self, Debug, Display, Formatter};
use std::hash::Hash;
use std::io::{Read, Write};
use std::num::ParseIntError;
use std::str::FromStr;

use internet2::{presentation, transport};
use lightning_encoding::{LightningDecode, LightningEncode};
use strict_encoding::{StrictDecode, StrictEncode};

#[cfg(feature = "node")]
use crate::error::RuntimeError;

/// Marker trait for service-specific extended failure codes, representable as u16 integers.
///
/// NB: Failure codes must be within the range 0..=0x0FFF, since the upper 8 bits are always removed
/// from the service-specific code; codes 0x1000-0xFFFF are reserved for the system use.
pub trait FailureCodeExt: Copy + Eq + Ord + Hash + Debug + Into<u16> + From<u16> {}

/// Symbolic representation of a failure code returned by the server
#[derive(Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub enum FailureCode<Ext>
where
    Ext: FailureCodeExt,
{
    /// Presentation-level errors (encoding, TLV)
    Presentation,
    /// Transport-level errors (encryption, connectivity)
    Transport,
    /// Error in packet framing
    Framing,
    /// Request from the client is not supported by the server
    UnexpectedRequest,
    /// Server runtime error
    Runtime,
    /// Other service-specific error types (see [`FailureCodeExt`] for details)
    Other(Ext),
}

impl<Ext> StrictEncode for FailureCode<Ext>
where
    Ext: FailureCodeExt,
{
    fn strict_encode<E: Write>(&self, e: E) -> Result<usize, strict_encoding::Error> {
        u16::from(*self).strict_encode(e)
    }
}

impl<Ext> StrictDecode for FailureCode<Ext>
where
    Ext: FailureCodeExt,
{
    fn strict_decode<D: Read>(d: D) -> Result<Self, strict_encoding::Error> {
        Ok(u16::strict_decode(d)?.into())
    }
}

impl<Ext> LightningEncode for FailureCode<Ext>
where
    Ext: FailureCodeExt,
{
    fn lightning_encode<E: Write>(&self, e: E) -> Result<usize, lightning_encoding::Error> {
        u16::from(*self).lightning_encode(e)
    }
}

impl<Ext> LightningDecode for FailureCode<Ext>
where
    Ext: FailureCodeExt,
{
    fn lightning_decode<D: Read>(d: D) -> Result<Self, lightning_encoding::Error> {
        Ok(u16::lightning_decode(d)?.into())
    }
}

impl<Ext> From<u16> for FailureCode<Ext>
where
    Ext: FailureCodeExt,
{
    fn from(code: u16) -> Self {
        match code {
            code if code == u16::from(FailureCode::<Ext>::Presentation) => {
                FailureCode::Presentation
            }
            code if code == u16::from(FailureCode::<Ext>::Transport) => FailureCode::Transport,
            code if code == u16::from(FailureCode::<Ext>::Framing) => FailureCode::Framing,
            code if code == u16::from(FailureCode::<Ext>::UnexpectedRequest) => {
                FailureCode::UnexpectedRequest
            }
            code if code == u16::from(FailureCode::<Ext>::Runtime) => FailureCode::Runtime,
            code => FailureCode::Other(code.into()),
        }
    }
}

impl<Ext> Display for FailureCode<Ext>
where
    Ext: FailureCodeExt,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result { write!(f, "{:#04x}", u16::from(*self)) }
}

impl<Ext> FromStr for FailureCode<Ext>
where
    Ext: FailureCodeExt,
{
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let code = u16::from_str_radix(s, 16)?;
        Ok(FailureCode::from(code))
    }
}

impl<Ext> From<FailureCode<Ext>> for u16
where
    Ext: FailureCodeExt,
{
    fn from(code: FailureCode<Ext>) -> Self {
        match code {
            FailureCode::Presentation => 0x1000,
            FailureCode::Transport => 0x2000,
            FailureCode::Framing => 0x3000,
            FailureCode::UnexpectedRequest => 0x4000,
            FailureCode::Runtime => 0x5000,
            FailureCode::Other(other) => other.into() & 0x0FFF,
        }
    }
}

/// Information about server-side failure returned through RPC API
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(crate = "serde_crate"))]
#[derive(Clone, PartialEq, Eq, Hash, Debug, Display, StrictEncode, StrictDecode)]
#[display("server failure #{code} {info}")]
pub struct Failure<Ext>
where
    Ext: FailureCodeExt,
{
    /// Failure code
    #[cfg_attr(feature = "serde", serde(with = "serde_with::rust::display_fromstr"))]
    pub code: FailureCode<Ext>,

    /// Detailed information about the failure
    pub info: String,
}

/// Errors happening with RPC APIs received by the server, but originating from the client
/// connection.
#[derive(Clone, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum ClientError {
    /// unexpected client request
    UnexpectedRequest,

    /// Message serialization or structure error.
    #[from(lightning_encoding::Error)]
    #[from(strict_encoding::Error)]
    Presentation(presentation::Error),

    /// Transport-level protocol error.
    #[from]
    Transport(transport::Error),
}

impl From<zmq::Error> for ClientError {
    fn from(err: zmq::Error) -> Self { ClientError::Transport(transport::Error::from(err)) }
}

impl From<presentation::Error> for ClientError {
    fn from(err: presentation::Error) -> Self {
        match err {
            presentation::Error::Transport(err) => err.into(),
            err => ClientError::Presentation(err),
        }
    }
}

/// Errors happening with RPC APIs on the server side and returned to the client
#[derive(Clone, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum ServerError<Ext>
where
    Ext: FailureCodeExt,
{
    /// unexpected server response
    UnexpectedServerResponse,

    /// Server failure
    #[from]
    #[display(inner)]
    ServerFailure(Failure<Ext>),

    /// Message serialization or structure error.
    #[from(lightning_encoding::Error)]
    #[from(strict_encoding::Error)]
    Presentation(presentation::Error),

    /// Transport-level protocol error.
    #[from]
    Transport(transport::Error),

    /// provided RPC endpoint {0} is unknown
    UnknownEndpoint(String),
}

impl<Ext> From<zmq::Error> for ServerError<Ext>
where
    Ext: FailureCodeExt,
{
    fn from(err: zmq::Error) -> Self { ServerError::Transport(transport::Error::from(err)) }
}

impl<Ext> From<presentation::Error> for ServerError<Ext>
where
    Ext: FailureCodeExt,
{
    fn from(err: presentation::Error) -> Self {
        match err {
            presentation::Error::Transport(err) => err.into(),
            err => ServerError::Presentation(err),
        }
    }
}

impl<Ext> From<presentation::Error> for Failure<Ext>
where
    Ext: FailureCodeExt,
{
    fn from(err: presentation::Error) -> Self {
        Failure { info: err.to_string(), code: FailureCode::Presentation }
    }
}

#[cfg(feature = "node")]
impl<E, Ext> From<RuntimeError<E>> for Failure<Ext>
where
    Ext: FailureCodeExt,
    E: crate::error::Error,
{
    fn from(err: RuntimeError<E>) -> Self {
        Failure { code: FailureCode::Runtime, info: err.to_string() }
    }
}
