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
use std::thread;
use std::thread::JoinHandle;

use internet2::addr::InetSocketAddr;
use internet2::{session, LocalNode, LocalSocketAddr, NodeAddr, RemoteNodeAddr, RemoteSocketAddr};
use nix::unistd::{fork, ForkResult, Pid};
use secp256k1::PublicKey;

use super::{PeerConnection, PeerSocket};

#[derive(Clone, Debug)]
pub struct RuntimeParams<Config>
where
    Config: Clone + Debug,
{
    pub config: Config,
    pub id: NodeAddr,
    pub local_id: PublicKey,
    pub remote_id: Option<PublicKey>,
    pub local_socket: Option<InetSocketAddr>,
    pub remote_socket: InetSocketAddr,
    pub connect: bool,
}

impl<Config> RuntimeParams<Config>
where
    Config: Clone + Debug,
{
    fn with(config: Config, local_id: PublicKey) -> Self {
        RuntimeParams {
            config,
            id: NodeAddr::Local(LocalSocketAddr::Posix(s!(""))),
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
    local_node: &LocalNode,
    peer_socket: PeerSocket,
    runtime: fn(connection: PeerConnection, params: RuntimeParams<Config>) -> Result<(), Error>,
) -> Result<(), Error>
where
    Config: 'static + Clone + Debug + std::marker::Send,
    Error: 'static + std::error::Error + std::marker::Send + From<std::io::Error>,
{
    debug!("Peer socket parameter interpreted as {}", peer_socket);

    let mut params = RuntimeParams::with(config, local_node.node_id());
    match peer_socket {
        PeerSocket::Listen(RemoteSocketAddr::Ftcp(inet_addr)) => {
            info!("Running peer daemon in LISTEN mode");

            params.connect = false;
            params.local_socket = Some(inet_addr);
            params.id = NodeAddr::Remote(RemoteNodeAddr {
                node_id: local_node.node_id(),
                remote_addr: RemoteSocketAddr::Ftcp(inet_addr),
            });

            spawner(params, inet_addr, threaded, local_node, runtime)?;
        }
        PeerSocket::Connect(remote_node_addr) => {
            debug!("Running peer daemon in CONNECT mode");

            params.connect = true;
            params.id = NodeAddr::Remote(remote_node_addr.clone());
            params.remote_id = Some(remote_node_addr.node_id);
            params.remote_socket = remote_node_addr.remote_addr.into();

            info!("Connecting to {}", &remote_node_addr);
            let connection = PeerConnection::connect(remote_node_addr, &local_node)
                .expect("Unable to connect to the remote peer");
            runtime(connection, params)?;
        }
        PeerSocket::Listen(_) => {
            unimplemented!("we do not support non-TCP connections for the legacy lightning network")
        }
    }

    unreachable!()
}

enum Handler<Error>
where
    Error: std::error::Error,
{
    Thread(JoinHandle<Result<(), Error>>),
    Process(Pid),
}

fn spawner<Config, Error>(
    mut params: RuntimeParams<Config>,
    inet_addr: InetSocketAddr,
    threaded_daemons: bool,
    local_node: &LocalNode,
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
            let session = session::Raw::with_brontide(stream, node_sk, remote_socket_addr.into())
                .expect("Unable to establish session with the remote peer");
            let connection = PeerConnection::with(session);
            runtime(connection, child_params)
        };

        if threaded_daemons {
            debug!("Spawning child thread");
            let handler =
                thread::Builder::new().name(format!("peerd-listner<{}>", inet_addr)).spawn(init)?;
            handlers.push(Handler::Thread(handler));
            // We have started the thread so awaiting for the next incoming connection
        } else {
            debug!("Forking child process");
            if let ForkResult::Parent { child } =
                unsafe { fork().expect("Unable to fork child process") }
            {
                handlers.push(Handler::Process(child));
                debug!("Child forked with pid {}; returning into main listener event loop", child);
            } else {
                init()?;
                unreachable!("we are in the child process");
            }
        }
        trace!("Total {} peerd are spawned for the incoming connections", handlers.len());
    }
}
