// LNP Node: node running lightning network protocol and generalized lightning
// channels.
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

use std::convert::TryFrom;
use std::fmt::Debug;
use std::net::{SocketAddr, TcpListener};
use std::thread::JoinHandle;
use std::{io, thread};

use internet2::addr::{InetSocketAddr, LocalNode, NodeId};
use internet2::session::noise::FramingProtocol;
use internet2::session::{BrontideSession, BrontozaurSession};
#[cfg(not(target_os = "windows"))]
use nix::unistd::{fork, ForkResult, Pid};

use super::{PeerConnection, PeerSocket};

#[derive(Clone, Debug)]
pub struct RuntimeParams<Config>
where
    Config: Clone + Debug,
{
    pub config: Config,
    pub framing_protocol: FramingProtocol,
    pub local_id: NodeId,
    pub remote_id: Option<NodeId>,
    pub local_socket: Option<InetSocketAddr>,
    pub remote_socket: InetSocketAddr,
    pub connect: bool,
}

impl<Config> RuntimeParams<Config>
where
    Config: Clone + Debug,
{
    fn with(config: Config, local_id: NodeId, framing_protocol: FramingProtocol) -> Self {
        RuntimeParams {
            config,
            framing_protocol,
            local_id,
            remote_id: None,
            local_socket: None,
            remote_socket: Default::default(),
            connect: false,
        }
    }
}

pub fn run<Config, Error>(
    config: Config,
    threaded: bool,
    framing_protocol: FramingProtocol,
    local_node: LocalNode,
    peer_socket: PeerSocket,
    runtime: fn(connection: PeerConnection, params: RuntimeParams<Config>) -> Result<(), Error>,
) -> Result<(), Error>
where
    Config: 'static + Clone + Debug + Send,
    Error: 'static + std::error::Error + Send + From<io::Error> + From<internet2::transport::Error>,
{
    debug!("Peer socket parameter interpreted as {}", peer_socket);

    let mut params = RuntimeParams::with(config, local_node.node_id(), framing_protocol);
    match peer_socket {
        PeerSocket::Listen(node_addr) => {
            info!("Running peer daemon in LISTEN mode");

            params.connect = false;
            params.local_id = node_addr.id;
            params.local_socket = Some(node_addr.addr);

            spawner(params, node_addr.addr, threaded, framing_protocol, local_node, runtime)?;
        }
        PeerSocket::Connect(node_addr) => {
            debug!("Running peer daemon in CONNECT mode");

            params.connect = true;
            params.remote_id = Some(node_addr.id);
            params.remote_socket = node_addr.addr;

            info!("Connecting to {}", node_addr);
            let connection = match framing_protocol {
                FramingProtocol::Brontide => {
                    PeerConnection::connect_brontide(local_node, node_addr)?
                }
                FramingProtocol::Brontozaur => {
                    PeerConnection::connect_brontozaur(local_node, node_addr)?
                }
            };
            runtime(connection, params)?;
        }
    }

    unreachable!()
}

enum Handler<Error>
where
    Error: std::error::Error,
{
    Thread(JoinHandle<Result<(), Error>>),
    #[cfg(not(target_os = "windows"))]
    Process(Pid),
}

fn spawner<Config, Error>(
    mut params: RuntimeParams<Config>,
    inet_addr: InetSocketAddr,
    threaded: bool,
    framing_protocol: FramingProtocol,
    local_node: LocalNode,
    runtime: fn(connection: PeerConnection, params: RuntimeParams<Config>) -> Result<(), Error>,
) -> Result<(), Error>
where
    Config: 'static + Clone + Debug + std::marker::Send,
    Error: 'static + std::error::Error + std::marker::Send + From<std::io::Error>,
{
    // Handlers for all of our spawned processes and threads
    let mut handlers = vec![];

    info!("Binding TCP socket {}", inet_addr);
    let listener =
        TcpListener::bind(SocketAddr::try_from(inet_addr).expect("Tor is not yet supported"))
            .expect("Unable to bind to Lightning network peer socket");

    info!("Running TCP listener event loop");
    loop {
        debug!("Awaiting for incoming connections...");
        let (stream, remote_socket_addr) =
            listener.accept().expect("Error accepting incpming peer connection");
        info!("New connection from {}", remote_socket_addr);

        params.remote_socket = remote_socket_addr.into();

        let child_params = params.clone();
        let node_sk = local_node.private_key();
        let init = move || {
            debug!("Establishing session with the remote");
            let connection = match framing_protocol {
                FramingProtocol::Brontide => {
                    let session = BrontideSession::with(stream, node_sk, remote_socket_addr.into())
                        .expect("Unable to establish session with the remote peer");
                    PeerConnection::with(session)
                }
                FramingProtocol::Brontozaur => {
                    let session =
                        BrontozaurSession::with(stream, node_sk, remote_socket_addr.into())
                            .expect("Unable to establish session with the remote peer");
                    PeerConnection::with(session)
                }
            };
            runtime(connection, child_params)
        };

        if threaded {
            debug!("Spawning child thread");
            let handler =
                thread::Builder::new().name(format!("peerd-listner<{}>", inet_addr)).spawn(init)?;
            handlers.push(Handler::Thread(handler));
            // We have started the thread so awaiting for the next incoming connection
        } else {
            #[cfg(target_os = "windows")]
            panic!("windows do not (yet) supports multi-process configuration");
            #[cfg(not(target_os = "windows"))]
            {
                debug!("Forking child process");
                if let ForkResult::Parent { child } =
                    unsafe { fork().expect("Unable to fork child process") }
                {
                    handlers.push(Handler::Process(child));
                    debug!(
                        "Child forked with pid {}; returning into main listener event loop",
                        child
                    );
                } else {
                    init()?;
                    unreachable!("we are in the child process");
                }
            }
        }
        trace!("Total {} peerd are spawned for the incoming connections", handlers.len());
    }
}
