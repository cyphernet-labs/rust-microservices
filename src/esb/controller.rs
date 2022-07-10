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
use internet2::{zeromq, SendRecvMessage, Unmarshall, Unmarshaller, ZmqSocketType};

use super::{BusId, Error, ServiceAddress};
use crate::esb::BusConfig;
#[cfg(feature = "node")]
use crate::node::TryService;
use crate::rpc::Request;
use crate::ZMQ_CONTEXT;

/// Trait for types handling specific set of ESB RPC API requests structured as
/// a single type implementing [`Request`].
pub trait Handler<B>
where
    Self: Sized,
    B: BusId,
    Error<B::Address>: From<Self::Error>,
{
    type Request: Request;
    type Error: std::error::Error;

    fn identity(&self) -> B::Address;

    fn on_ready(&mut self, _endpoints: &mut EndpointList<B>) -> Result<(), Self::Error> { Ok(()) }

    fn handle(
        &mut self,
        endpoints: &mut EndpointList<B>,
        bus_id: B,
        source: B::Address,
        request: Self::Request,
    ) -> Result<(), Self::Error>;

    fn handle_err(
        &mut self,
        endpoints: &mut EndpointList<B>,
        error: Error<B::Address>,
    ) -> Result<(), Self::Error>;
}

struct Endpoint<A>
where
    A: ServiceAddress,
{
    pub(self) session: LocalSession,
    pub(self) router: Option<A>,
}

impl<A> Endpoint<A>
where
    A: ServiceAddress,
{
    pub(self) fn send_to<R>(&mut self, source: A, dest: A, request: R) -> Result<(), Error<A>>
    where
        R: Request,
    {
        let data = request.serialize();
        let router = match self.router {
            None => {
                trace!("Sending {} from {} to {} directly", request, source, dest,);
                dest.clone()
            }
            Some(ref router) if &source == router => {
                trace!("Routing {} from {} to {}", request, source, dest,);
                dest.clone()
            }
            Some(ref router) => {
                trace!("Sending {} from {} to {} via router {}", request, source, dest, router,);
                router.clone()
            }
        };
        let src = source.clone();
        let dst = dest.clone();
        self.session
            .send_routed_message(&source.into(), &router.into(), &dest.into(), &data)
            .map_err(|err| Error::Send(src, dst, err))?;
        Ok(())
    }

    #[inline]
    pub(self) fn set_identity(&mut self, identity: A) -> Result<(), Error<A>> {
        self.session.set_identity(&identity.into(), &ZMQ_CONTEXT).map_err(Error::from)
    }
}

#[derive(Default)]
pub struct EndpointList<B>(pub(self) HashMap<B, Endpoint<B::Address>>)
where
    B: BusId;

impl<B> EndpointList<B>
where
    B: BusId,
{
    pub fn new() -> Self { Self(Default::default()) }

    pub fn send_to<R>(
        &mut self,
        bus_id: B,
        source: B::Address,
        dest: B::Address,
        request: R,
    ) -> Result<(), Error<B::Address>>
    where
        R: Request,
    {
        let session =
            self.0.get_mut(&bus_id).ok_or_else(|| Error::UnknownBusId(bus_id.to_string()))?;
        session.send_to(source, dest, request)
    }

    pub fn set_identity(
        &mut self,
        bus_id: B,
        identity: B::Address,
    ) -> Result<(), Error<B::Address>> {
        self.0
            .get_mut(&bus_id)
            .ok_or_else(|| Error::UnknownBusId(bus_id.to_string()))?
            .set_identity(identity)
    }
}

#[derive(Getters)]
pub struct Controller<B, R, H>
where
    R: Request,
    B: BusId,
    H: Handler<B, Request = R>,
    Error<B::Address>: From<H::Error>,
{
    endpoints: EndpointList<B>,
    unmarshaller: Unmarshaller<R>,
    handler: H,
}

#[derive(Debug)]
pub struct PollItem<B, R>
where
    B: BusId,
    R: Request,
{
    pub bus_id: B,
    pub source: B::Address,
    pub request: R,
}

impl<B, R, H> Controller<B, R, H>
where
    R: Request,
    B: BusId,
    H: Handler<B, Request = R>,
    Error<B::Address>: From<H::Error>,
{
    pub fn with(
        service_bus: HashMap<B, BusConfig<B::Address>>,
        handler: H,
    ) -> Result<Self, Error<B::Address>> {
        let endpoints = EndpointList::new();
        let unmarshaller = R::create_unmarshaller();
        let mut me = Self { endpoints, unmarshaller, handler };
        for (id, config) in service_bus {
            me.add_service_bus(id, config)?;
        }
        Ok(me)
    }

    pub fn add_service_bus(
        &mut self,
        id: B,
        config: BusConfig<B::Address>,
    ) -> Result<(), Error<B::Address>> {
        let session = match config.carrier {
            zeromq::Carrier::Locator(locator) => {
                debug!(
                    "Creating ESB session for service {} located at {} with identity '{}'",
                    id,
                    locator,
                    self.handler.identity()
                );
                // TODO: Replace with RpcSession once its impl is completed
                LocalSession::connect(
                    config.api_type,
                    &locator,
                    None,
                    Some(&self.handler.identity().into()),
                    &ZMQ_CONTEXT,
                )?
            }
            // TODO: Replace with RpcSession once its impl is completed
            zeromq::Carrier::Socket(socket) => {
                debug!("Creating ESB session for service {}", &id);
                // TODO: Replace with RpcSession once its impl is completed
                LocalSession::with_zmq_socket(config.api_type, socket)
            }
        };
        if !config.queued {
            session.as_socket().set_router_mandatory(true)?;
        }
        if config.api_type == ZmqSocketType::Sub {
            session.as_socket().set_subscribe(config.topic.unwrap_or_default().as_bytes())?;
        }
        let router = match config.router {
            Some(router) if router == self.handler.identity() => None,
            router => router,
        };
        self.endpoints.0.insert(id, Endpoint { session, router });
        Ok(())
    }

    pub fn send_to(
        &mut self,
        bus_id: B,
        dest: B::Address,
        request: R,
    ) -> Result<(), Error<B::Address>> {
        self.endpoints.send_to(bus_id, self.handler.identity(), dest, request)
    }

    pub fn recv_poll(&mut self) -> Result<Vec<PollItem<B, R>>, Error<B::Address>> {
        let mut vec = vec![];
        for bus_id in self.poll()? {
            let sender = self.endpoints.0.get_mut(&bus_id).expect("must exist, just indexed");

            let routed_frame = sender.session.recv_routed_message()?;
            let request = (&*self.unmarshaller.unmarshall(Cursor::new(routed_frame.msg))?).clone();
            let source = B::Address::from(routed_frame.src);

            vec.push(PollItem { bus_id, source, request });
        }

        Ok(vec)
    }
}

#[cfg(feature = "node")]
impl<B, R, H> TryService for Controller<B, R, H>
where
    R: Request,
    B: BusId,
    H: Handler<B, Request = R>,
    Error<B::Address>: From<H::Error>,
{
    type ErrorType = Error<B::Address>;

    fn try_run_loop(mut self) -> Result<(), Self::ErrorType> {
        self.handler.on_ready(&mut self.endpoints)?;
        loop {
            match self.run() {
                Ok(_) => trace!("request processing complete"),
                Err(err) => {
                    error!("ESB request processing error: {}", err);
                    self.handler.handle_err(&mut self.endpoints, err)?;
                }
            }
        }
    }
}

impl<B, R, H> Controller<B, R, H>
where
    R: Request,
    B: BusId,
    H: Handler<B, Request = R>,
    Error<B::Address>: From<H::Error>,
{
    #[cfg(feature = "node")]
    fn run(&mut self) -> Result<(), Error<B::Address>> {
        for bus_id in self.poll()? {
            let sender = self.endpoints.0.get_mut(&bus_id).expect("must exist, just indexed");

            let routed_frame = sender.session.recv_routed_message()?;
            let request = (&*self.unmarshaller.unmarshall(Cursor::new(routed_frame.msg))?).clone();
            let source = B::Address::from(routed_frame.src);
            let dest = B::Address::from(routed_frame.dst);

            if dest == self.handler.identity() {
                // We are the destination
                debug!("{} -> {}: {}", source, dest, request);

                self.handler.handle(&mut self.endpoints, bus_id, source, request)?;
            } else {
                // Need to route
                trace!("Routing {} from {} to {}", request, source, dest);
                self.endpoints.send_to(bus_id, source, dest, request)?
            }
        }

        Ok(())
    }

    fn poll(&mut self) -> Result<Vec<B>, Error<B::Address>> {
        let mut index = vec![];
        let mut items = self
            .endpoints
            .0
            .iter()
            .map(|(service, sender)| {
                index.push(service);
                sender.session.as_socket().as_poll_item(zmq::POLLIN | zmq::POLLERR)
            })
            .collect::<Vec<_>>();

        trace!("Awaiting for ESB request from {} service buses...", items.len());
        let _ = zmq::poll(&mut items, -1)?;

        let service_buses = items
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

        trace!("Received ESB request from {} service busses...", service_buses.len());

        Ok(service_buses)
    }
}
