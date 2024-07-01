use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::tungstenite::{
    protocol::{frame::coding::CloseCode, CloseFrame},
    Message,
};

use crate::protocol::Context;

use super::document::Document;

pub(super) struct DocumentContext<'s> {
    document: &'s mut Document,
    connection: UnboundedSender<Message>,
    closed: bool,
}

impl<'s> DocumentContext<'s> {
    pub(super) fn new(document: &'s mut Document, connection: UnboundedSender<Message>) -> Self {
        Self {
            document,
            connection,
            closed: false,
        }
    }

    pub(super) const fn is_closed(&self) -> bool {
        self.closed
    }
}

impl<'s> Context for DocumentContext<'s> {
    fn unicast(&self, msg: Vec<u8>) {
        if self.connection.send(Message::Binary(msg)).is_err() {
            log::error!("unicast message failed");
        }
    }

    fn broadcast(&self, msg: Vec<u8>) {
        for (_, connection) in self.document.connections.iter() {
            if connection.send(Message::Binary(msg.clone())).is_err() {
                log::error!("broadcast message failed");
            }
        }
    }

    fn get_document(&self) -> &y_octo::Doc {
        &self.document.doc
    }

    fn get_document_mut(&mut self) -> &mut y_octo::Doc {
        &mut self.document.doc
    }

    fn get_awareness(&self) -> &y_octo::Awareness {
        &self.document.awareness
    }

    fn get_awareness_mut(&mut self) -> &mut y_octo::Awareness {
        &mut self.document.awareness
    }

    fn get_document_name(&self) -> &str {
        &self.document.name
    }

    async fn close(&mut self) {
        if self
            .connection
            .send(Message::Close(Some(CloseFrame {
                code: CloseCode::Normal,
                reason: "provider_initiated".into(),
            })))
            .is_err()
        {
            log::error!("close failed");
        }
        self.connection.closed().await;

        self.closed = true;
    }
}
