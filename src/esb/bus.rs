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
use std::str::FromStr;

use amplify::hex::{self, ToHex};
use internet2::addr::ServiceAddr;
use internet2::{zeromq, ZmqSocketType};

/// Marker traits for service bus identifiers
pub trait BusId: Copy + Eq + Hash + Debug + Display {
    /// Service address type used by this bus
    type Address: ServiceAddress;
}

#[non_exhaustive]
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
    pub topic: Option<String>,
}

impl<A> BusConfig<A>
where
    A: ServiceAddress,
{
    pub fn with_addr(addr: ServiceAddr, api_type: ZmqSocketType, router: Option<A>) -> Self {
        Self {
            api_type,
            carrier: zeromq::Carrier::Locator(addr),
            router,
            queued: false,
            topic: None,
        }
    }

    pub fn with_subscription(
        addr: ServiceAddr,
        api_type: ZmqSocketType,
        router: Option<A>,
        topic: String,
    ) -> Self {
        Self {
            api_type,
            carrier: zeromq::Carrier::Locator(addr),
            router,
            queued: false,
            topic: Some(topic),
        }
    }

    pub fn with_socket(socket: zmq::Socket, api_type: ZmqSocketType, router: Option<A>) -> Self {
        Self {
            api_type,
            carrier: zeromq::Carrier::Socket(socket),
            router,
            queued: false,
            topic: None,
        }
    }
}

/// Marker traits for service bus identifiers
pub trait ServiceAddress:
    Clone + Eq + Hash + Debug + Display + Into<Vec<u8>> + From<Vec<u8>>
{
}

pub type ClientId = u64;

#[derive(Wrapper, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, From, Default)]
#[derive(StrictEncode, StrictDecode)]
pub struct ServiceName([u8; 32]);

impl Display for ServiceName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            write!(f, "{}..{}", self.0[..4].to_hex(), self.0[(self.0.len() - 4)..].to_hex())
        } else {
            f.write_str(&String::from_utf8_lossy(&self.0))
        }
    }
}

impl FromStr for ServiceName {
    type Err = hex::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() > 32 {
            let mut me = Self::default();
            me.0.copy_from_slice(&s.as_bytes()[0..32]);
            Ok(me)
        } else {
            let mut me = Self::default();
            me.0[0..s.len()].copy_from_slice(s.as_bytes());
            Ok(me)
        }
    }
}
