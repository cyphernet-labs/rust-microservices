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

use internet2::session::LocalSession;
use internet2::{
    transport, zeromq, CreateUnmarshaller, SendRecvMessage, TypedEnum, Unmarshall, Unmarshaller,
    ZmqSocketType,
};

use super::{Api, EndpointId, Failure};
use crate::node::TryService;
use crate::rpc::ClientError;
use crate::ZMQ_CONTEXT;

/// Trait for types handling specific set of RPC API requests structured as a
/// single type implementing [`Request`]. They must return a corresponding reply
/// type implementing [`Reply`]. This request/replu pair is structured as an
/// [`Api`] trait provided in form of associated type parameter.
///
/// [`Request`]: super::Request
/// [`Reply`]: super::Reply
pub trait Handler<E>
where
    Self: Sized,
    E: EndpointId,
{
    type Api: Api;
    type Error: crate::error::Error + Into<Failure<<Self::Api as Api>::FailureCodeExt>>;

    /// Function that processes specific request and returns either response or
    /// a error that can be converted into a failure response
    fn handle(
        &mut self,
        endpoint: E,
        request: <Self::Api as Api>::Request,
    ) -> Result<<Self::Api as Api>::Reply, Self::Error>;

    fn handle_err(&mut self, error: ClientError) -> Result<(), ClientError>;
}

pub struct RpcServer<E, A, H>
where
    A: Api,
    H::Error: Into<Failure<A::FailureCodeExt>>,
    A::Reply: From<Failure<A::FailureCodeExt>>,
    E: EndpointId,
    H: Handler<E, Api = A>,
{
    // TODO: Replace with RpcSession once its implementation is complete
    sessions: HashMap<E, LocalSession>,
    unmarshaller: Unmarshaller<A::Request>,
    handler: H,
}

impl<E, A, H> RpcServer<E, A, H>
where
    A: Api,
    H::Error: Into<Failure<A::FailureCodeExt>>,
    A::Reply: From<Failure<A::FailureCodeExt>>,
    E: EndpointId,
    H: Handler<E, Api = A>,
{
    pub fn with(
        endpoints: HashMap<E, zeromq::Carrier>,
        handler: H,
    ) -> Result<Self, transport::Error> {
        let mut sessions: HashMap<E, LocalSession> = none!();
        for (endpoint, carrier) in endpoints {
            sessions.insert(endpoint, match carrier {
                zeromq::Carrier::Locator(locator) => {
                    debug!("Creating RPC session for endpoint {} located at {}", endpoint, locator);
                    LocalSession::connect(ZmqSocketType::Rep, &locator, None, None, &ZMQ_CONTEXT)?
                }
                zeromq::Carrier::Socket(socket) => {
                    debug!("Creating RPC session for endpoint {}", &endpoint);
                    LocalSession::with_zmq_socket(ZmqSocketType::Rep, socket)
                }
            });
        }
        let unmarshaller = A::Request::create_unmarshaller();
        Ok(Self { sessions, unmarshaller, handler })
    }
}

impl<E, A, H> TryService for RpcServer<E, A, H>
where
    A: Api,
    H::Error: Into<Failure<A::FailureCodeExt>>,
    A::Reply: From<Failure<A::FailureCodeExt>>,
    E: EndpointId,
    H: Handler<E, Api = A>,
{
    type ErrorType = ClientError;

    fn try_run_loop(mut self) -> Result<(), Self::ErrorType> {
        loop {
            match self.run() {
                Ok(_) => debug!("RPC request processing complete"),
                Err(err) => {
                    error!("RPC request processing error: {}", err);
                    self.handler.handle_err(err)?;
                }
            }
        }
    }
}

impl<E, A, H> RpcServer<E, A, H>
where
    A: Api,
    H::Error: Into<Failure<A::FailureCodeExt>>,
    A::Reply: From<Failure<A::FailureCodeExt>>,
    E: EndpointId,
    H: Handler<E, Api = A>,
{
    fn run(&mut self) -> Result<(), ClientError> {
        let mut index = vec![];
        let mut items = self
            .sessions
            .iter()
            .map(|(endpoint, session)| {
                index.push(endpoint);
                session.as_socket().as_poll_item(zmq::POLLIN | zmq::POLLERR)
            })
            .collect::<Vec<_>>();

        trace!("Awaiting for RPC request from {} endpoints...", items.len());
        let _ = zmq::poll(&mut items, -1)?;

        let endpoints = items
            .iter()
            .enumerate()
            .filter_map(
                |(i, item)| {
                    if item.get_revents().is_empty() {
                        None
                    } else {
                        Some(*index[i])
                    }
                },
            )
            .collect::<Vec<_>>();
        trace!("Received RPC request from {} endpoints...", endpoints.len());

        for endpoint in endpoints {
            let session = &mut self.sessions.get_mut(&endpoint).expect("must exist, just indexed");

            let raw = session.recv_raw_message()?;
            let request = &*self.unmarshaller.unmarshall(Cursor::new(raw))?;

            debug!("RPC: got request {}", request);
            let reply = self
                .handler
                .handle(endpoint, request.clone())
                .unwrap_or_else(|err| A::Reply::from(err.into()));
            debug!("RPC: replying with {:?}", reply);
            let data = reply.serialize();
            session.send_raw_message(&data)?;
        }

        Ok(())
    }
}
