use core::panic;
use std::{cell::RefCell, collections::HashMap, pin::Pin, sync::Arc};

use tokio::{
    net::{TcpListener, TcpStream},
    sync::{
        mpsc::{unbounded_channel, UnboundedSender},
        RwLock,
    },
};
use tokio_tungstenite::{
    accept_hdr_async_with_config,
    tungstenite::{protocol::WebSocketConfig, Message},
};
use y_octo::Doc;

use crate::{
    connection::{self, Connection},
    room::{Room, RoomCommand},
    utils::Snowflake,
};

pub struct Server {
    connection_id_generator: RefCell<Snowflake>,
    rooms: Arc<RwLock<HashMap<String, RoomCommand>>>,
}

impl Server {
    pub fn new(machine_id: u64) -> Self {
        let connection_id_generator = match Snowflake::new(machine_id) {
            Ok(connection_id_generator) => RefCell::new(connection_id_generator),
            Err(err) => {
                panic!("cannot register connection id generator, err: {err:?}");
            }
        };

        Self {
            connection_id_generator,
            rooms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn run(self: Pin<&'static Self>) {
        let listener = match TcpListener::bind("127.0.0.1:2976").await {
            Ok(listener) => listener,
            Err(err) => {
                log::error!("bind tcp listener failed,err: {err}");
                return;
            }
        };

        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(self.handle_stream(stream));
        }
    }

    async fn handle_stream(self: Pin<&Self>, stream: TcpStream) {
        // TODO hardcode
        let room_name = "default";

        let stream = match accept_hdr_async_with_config(
            stream,
            |_res: &_, resp| {
                // TODO get document name
                Ok(resp)
            },
            Some(WebSocketConfig::default()),
        )
        .await
        {
            Ok(stream) => stream,
            Err(err) => {
                log::error!("handle stream failed, err: {err}");
                return;
            }
        };

        let connection_id = match self.connection_id_generator.borrow_mut().gen() {
            Ok(connection_id) => connection_id,
            Err(err) => {
                log::error!("generate connection id failed, err: {err:?}");
                return;
            }
        };

        let (room_outgoing, room_incoming) = unbounded_channel::<Message>();
        let room_command = match self
            .enter_room(connection_id, room_outgoing, room_name)
            .await
        {
            Ok(room_command) => room_command,
            Err(err) => {
                log::error!("cannot get or create doc: {err:?}");
                return;
            }
        };

        let conn = Connection::new(
            connection_id,
            room_name.to_owned(),
            room_command,
            room_incoming,
            stream,
        );
        connection::connection(conn, async {
            self.handle_conn().await;
        });
    }

    async fn handle_conn(self: Pin<&Self>) {}

    async fn enter_room(
        self: Pin<&Self>,
        connection_id: u64,
        room_outgoing: UnboundedSender<Message>,
        doc_name: &str,
    ) -> Result<RoomCommand, ()> {
        if let Some(room_command) = self.rooms.read().await.get(doc_name) {
            if let Err(err) = room_command.join(connection_id, room_outgoing) {
                log::error!("cannot join room, err: {err:?}");
                return Err(());
            }

            return Ok(room_command.clone());
        }

        // TODO read doc
        let doc = Doc::default();

        let mut rooms = self.rooms.write().await;
        if !rooms.contains_key(doc_name) {
            // TODO doc
            let room_command = Room::create(doc_name.to_owned(), doc, |_doc| {
                // TODO
            });
            rooms.insert(doc_name.to_owned(), room_command);
        }

        let room_command = rooms.get(doc_name).ok_or(())?;
        if let Err(err) = room_command.join(connection_id, room_outgoing) {
            log::error!("cannot join room, err: {err:?}");
            return Err(());
        }

        Ok(room_command.clone())
    }
}

unsafe impl Send for Server {}

unsafe impl Sync for Server {}
