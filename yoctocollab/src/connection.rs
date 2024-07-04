use futures::{
    stream::{SplitSink, SplitStream},
    StreamExt,
};
use std::{cell::RefCell, future};

use tokio::{
    net::TcpStream, sync::mpsc::UnboundedReceiver, task::futures::TaskLocalFuture, task_local,
};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

use crate::room::RoomCommand;

pub(super) struct Connection {
    connection_id: u64,
    room_name: String,

    room_command: RoomCommand,
    stream_outgoing: SplitSink<WebSocketStream<TcpStream>, Message>,
    stream_incoming: SplitStream<WebSocketStream<TcpStream>>,
    room_incoming: UnboundedReceiver<Message>,
}

task_local! {
    static CONNECTION: RefCell<Connection>;
}

impl Connection {
    pub(super) fn new(
        connection_id: u64,
        room_name: String,

        room_command: RoomCommand,
        room_incoming: UnboundedReceiver<Message>,

        stream: WebSocketStream<TcpStream>,
    ) -> Self {
        let (stream_outgoing, stream_incoming) = stream.split();

        Self {
            connection_id,
            room_name,

            room_command,
            room_incoming,

            stream_outgoing,
            stream_incoming,
        }
    }
}

pub(super) fn connection<F: future::Future>(
    conn: Connection,
    f: F,
) -> TaskLocalFuture<RefCell<Connection>, F> {
    CONNECTION.scope(RefCell::new(conn), f)
}
