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

use std::fmt::{Debug, Display};

use internet2::addr::ServiceAddr;
use internet2::presentation::{CreateUnmarshaller, Error, TypedEnum};
use internet2::session::LocalSession;
use internet2::{SendRecvMessage, ZmqSocketType};

/// Marker trait for LNP RPC requests
pub trait Request: Debug + Display + TypedEnum + CreateUnmarshaller {}

/// Marker trait for LNP RPC replies
pub trait Reply: Debug + Display + TypedEnum + CreateUnmarshaller {}

/// RPC API pair, connecting [`Request`] type with [`Reply`]
pub trait Api {
    /// Requests supported by RPC API
    type Request: Request;

    /// Replies supported by RPC API
    type Reply: Reply;
}

#[allow(dead_code)]
pub struct RpcConnection<A>
where
    A: Api,
{
    api: A,
    session: Box<dyn SendRecvMessage>,
}

impl<A> RpcConnection<A>
where
    A: Api,
{
    pub fn connect(
        api: A,
        // TODO: Convert parameter to ServiceAddr once RpcSession will be complete
        remote: &ServiceAddr,
        local: &ServiceAddr,
        ctx: &zmq::Context,
    ) -> Result<Self, Error> {
        // TODO: Use RpcSession once its implementation is complete
        let session =
            Box::new(LocalSession::connect(ZmqSocketType::Req, remote, Some(local), None, ctx)?);
        Ok(Self { api, session })
    }

    pub fn accept(
        api: A,
        remote: &ServiceAddr,
        local: &ServiceAddr,
        ctx: &zmq::Context,
    ) -> Result<Self, Error> {
        let session =
            Box::new(LocalSession::connect(ZmqSocketType::Rep, remote, Some(local), None, ctx)?);
        Ok(Self { api, session })
    }
}
