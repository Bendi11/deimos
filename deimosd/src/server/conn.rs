use tokio::net::{TcpSocket, TcpStream};


/// A connection to a remote client, with references to state required to serve RPC requests
pub struct Connection {
    sock: TcpStream,
}

impl Connection {
    pub fn new(sock: TcpStream) -> Self {
        Self {
            sock,
        }
    }

    pub async fn serve(self) {
        loop {
            
        }
    }
}
