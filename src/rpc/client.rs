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

use std::collections::HashMap;
use std::io::Cursor;

use internet2::addr::ServiceAddr;
use internet2::session::LocalSession;
use internet2::{
    transport, CreateUnmarshaller, SendRecvMessage, TypedEnum, Unmarshall, Unmarshaller,
    ZmqSocketType,
};

use super::EndpointId;
use crate::rpc::connection::Api;
use crate::rpc::ServerError;
use crate::ZMQ_CONTEXT;

pub struct RpcClient<E, A>
where
    A: Api,
    E: EndpointId,
{
    // TODO: Replace with RpcSession once its implementation is complete
    sessions: HashMap<E, LocalSession>,
    unmarshaller: Unmarshaller<A::Reply>,
}

impl<E, A> RpcClient<E, A>
where
    A: Api,
    E: EndpointId,
{
    pub fn with(endpoints: HashMap<E, ServiceAddr>) -> Result<Self, transport::Error> {
        let mut sessions: HashMap<E, LocalSession> = none!();
        for (service, endpoint) in endpoints {
            sessions.insert(
                service,
                LocalSession::connect(ZmqSocketType::Req, &endpoint, None, None, &ZMQ_CONTEXT)?,
            );
        }
        let unmarshaller = A::Reply::create_unmarshaller();
        Ok(Self { sessions, unmarshaller })
    }

    pub fn request(
        &mut self,
        endpoint: E,
        request: A::Request,
    ) -> Result<A::Reply, ServerError<A::FailureCodeExt>> {
        let data = request.serialize();
        let session = self
            .sessions
            .get_mut(&endpoint)
            .ok_or_else(|| ServerError::UnknownEndpoint(endpoint.to_string()))?;
        session.send_raw_message(&data)?;
        let raw = session.recv_raw_message()?;
        let reply = self.unmarshaller.unmarshall(Cursor::new(raw))?;
        Ok((&*reply).clone())
    }
}
