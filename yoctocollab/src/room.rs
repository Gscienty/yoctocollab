use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio_tungstenite::tungstenite::Message;
use y_octo::Doc;

use crate::doc::Document;

enum RoomMessage {
    Join(u64, UnboundedSender<Message>),
    Message(u64, Vec<u8>),
    Leave(u64),
}

pub struct RoomCommand {
    cmd: UnboundedSender<RoomMessage>,
}

impl RoomCommand {
    fn new(cmd: UnboundedSender<RoomMessage>) -> Self {
        Self { cmd }
    }
}

pub struct Room {
    document: Document,
    receiver: UnboundedReceiver<RoomMessage>,
}

impl Room {
    fn new(name: String, doc: Doc, receiver: UnboundedReceiver<RoomMessage>) -> Self {
        let document = Document::new(name, doc);

        Self { document, receiver }
    }

    async fn run(&mut self) {
        while let Some(msg) = self.receiver.recv().await {
            self.handle_message(msg).await;

            if self.document.is_connection_empty() {
                break;
            }
        }

        // TODO callback
    }

    async fn handle_message(&mut self, msg: RoomMessage) {
        match msg {
            RoomMessage::Join(cid, connection) => {
                if let Err(err) = self.document.connect(cid, connection) {
                    log::error!("join room failed, err: {err}");
                    self.document.disconnect(cid);
                }
            }

            RoomMessage::Leave(cid) => self.document.disconnect(cid),

            RoomMessage::Message(cid, message) => {
                if let Err(err) = self.document.handle_message(cid, &message).await {
                    log::error!("handle message failed, err: {err}");
                    self.document.disconnect(cid);
                }
            }
        }
    }

    pub fn create<F: FnOnce(&str) + Send + 'static>(
        name: String,
        doc: Doc,
        on_destory: F,
    ) -> RoomCommand {
        let (sender, receiver) = unbounded_channel();

        tokio::spawn(async {
            let mut room = Self::new(name, doc, receiver);

            room.run().await;

            on_destory(room.document.get_name());
        });

        RoomCommand::new(sender)
    }
}
