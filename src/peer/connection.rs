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

//! BOLT-1. Manages state of the remote peer and handles direct communications
//! with it. Relies on transport layer (BOLT-8-based) protocol.

use std::fmt::Display;
use std::io::Cursor;

use amplify::Bipolar;
use internet2::addr::{LocalNode, NodeAddr};
use internet2::presentation::{self, Unmarshall};
use internet2::session::{BrontideSession, BrontozaurSession, LocalSession, Split};
use internet2::{session, transport, SendRecvMessage, TypedEnum};

pub trait RecvMessage {
    fn recv_message<D>(&mut self, d: &D) -> Result<D::Data, presentation::Error>
    where
        D: Unmarshall,
        <D as Unmarshall>::Data: Display,
        <D as Unmarshall>::Error: Into<presentation::Error>;
}

pub trait SendMessage {
    fn send_message(
        &mut self,
        message: impl TypedEnum + Display,
    ) -> Result<usize, presentation::Error>;
}

pub struct PeerConnection {
    session: Box<dyn SendRecvMessage>,
}

pub struct PeerReceiver {
    //#[cfg(not(feature = "async"))]
    receiver: Box<dyn session::RecvMessage + Send>,
    /* #[cfg(feature = "async")]
     * receiver: Box<dyn AsyncRecvFrame>, */
}

pub struct PeerSender {
    //#[cfg(not(feature = "async"))]
    sender: Box<dyn session::SendMessage + Send>,
    /* #[cfg(feature = "async")]
     * sender: Box<dyn AsyncSendFrame>, */
}

impl PeerConnection {
    pub fn with(session: impl SendRecvMessage + 'static) -> Self {
        Self { session: Box::new(session) }
    }

    pub fn connect_brontide(local: LocalNode, remote: NodeAddr) -> Result<Self, transport::Error> {
        BrontideSession::connect(local.private_key(), remote).map(PeerConnection::with)
    }

    pub fn connect_brontozaur(
        local: LocalNode,
        remote: NodeAddr,
    ) -> Result<Self, transport::Error> {
        BrontozaurSession::connect(local.private_key(), remote).map(PeerConnection::with)
    }
}

impl RecvMessage for PeerConnection {
    fn recv_message<D>(&mut self, d: &D) -> Result<D::Data, presentation::Error>
    where
        D: Unmarshall,
        <D as Unmarshall>::Data: Display,
        <D as Unmarshall>::Error: Into<presentation::Error>,
    {
        debug!("Awaiting incoming messages from the remote peer");
        let payload = self.session.recv_raw_message()?;
        trace!("Incoming data from the remote peer: {:?}", payload);
        let message: D::Data = d.unmarshall(Cursor::new(payload)).map_err(Into::into)?;
        debug!("Message from the remote peer: {}", message);
        Ok(message)
    }
}

impl SendMessage for PeerConnection {
    fn send_message(
        &mut self,
        message: impl TypedEnum + Display,
    ) -> Result<usize, presentation::Error> {
        debug!("Sending LN message to the remote peer: {}", message);
        let data = message.serialize();
        trace!("Encoded message representation: {:?}", data);
        Ok(self.session.send_raw_message(&data)?)
    }
}

impl RecvMessage for PeerReceiver {
    fn recv_message<D>(&mut self, d: &D) -> Result<D::Data, presentation::Error>
    where
        D: Unmarshall,
        <D as Unmarshall>::Data: Display,
        <D as Unmarshall>::Error: Into<presentation::Error>,
    {
        debug!("Awaiting incoming messages from the remote peer");
        let payload = self.receiver.recv_raw_message()?;
        trace!("Incoming data from the remote peer: {:?}", payload);
        let message: D::Data = d.unmarshall(Cursor::new(payload)).map_err(Into::into)?;
        debug!("Message from the remote peer: {}", message);
        Ok(message)
    }
}

impl SendMessage for PeerSender {
    fn send_message(
        &mut self,
        message: impl TypedEnum + Display,
    ) -> Result<usize, presentation::Error> {
        debug!("Sending LN message to the remote peer: {}", message);
        let data = message.serialize();
        trace!("Encoded message representation: {:?}", data);
        Ok(self.sender.send_raw_message(&data)?)
    }
}

impl Bipolar for PeerConnection {
    type Left = PeerReceiver;
    type Right = PeerSender;

    fn join(_left: Self::Left, _right: Self::Right) -> Self {
        // TODO: Implement
        unimplemented!()
    }

    fn split(self) -> (Self::Left, Self::Right) {
        let session = self.session.into_any();
        let (input, output) = if session.downcast_ref::<BrontozaurSession>().is_some() {
            let session = session
                .downcast::<BrontozaurSession>()
                .expect("downcast can't process type accepted by downcast_ref");
            (*session).split()
        } else if session.downcast_ref::<BrontideSession>().is_some() {
            let session = session
                .downcast::<BrontideSession>()
                .expect("downcast can't process type accepted by downcast_ref");
            (*session).split()
        } else if session.downcast_ref::<LocalSession>().is_some() {
            let session = session
                .downcast::<LocalSession>()
                .expect("downcast can't process type accepted by downcast_ref");
            (*session).split()
        } else {
            panic!("Impossible to split this type of session")
        };
        (PeerReceiver { receiver: input }, PeerSender { sender: output })
    }
}
