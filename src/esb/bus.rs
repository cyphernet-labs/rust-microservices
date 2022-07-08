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
use std::hash::Hash;

use internet2::addr::ServiceAddr;
use internet2::{zeromq, ZmqSocketType};

/// Marker traits for service bus identifiers
pub trait BusId: Copy + Eq + Hash + Debug + Display {
    /// Service address type used by this bus
    type Address: ServiceAddress;
}

pub struct BusConfig<A>
where
    A: ServiceAddress,
{
    pub api_type: ZmqSocketType,
    pub carrier: zeromq::Carrier,
    pub router: Option<A>,
    /// Indicates whether the messages must be queued, or the send function
    /// must fail immediately if the remote point is not available
    pub queued: bool,
}

impl<A> BusConfig<A>
where
    A: ServiceAddress,
{
    pub fn with_addr(addr: ServiceAddr, api_type: ZmqSocketType, router: Option<A>) -> Self {
        Self { api_type, carrier: zeromq::Carrier::Locator(addr), router, queued: false }
    }

    pub fn with_socket(socket: zmq::Socket, api_type: ZmqSocketType, router: Option<A>) -> Self {
        Self { api_type, carrier: zeromq::Carrier::Socket(socket), router, queued: false }
    }
}

/// Marker traits for service bus identifiers
pub trait ServiceAddress:
    Clone + Eq + Hash + Debug + Display + Into<Vec<u8>> + From<Vec<u8>>
{
}
