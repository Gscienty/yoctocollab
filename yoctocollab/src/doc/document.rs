use std::{collections::HashMap, u64};

use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::tungstenite::Message;
use y_octo::{Awareness, Doc, JwstCodecResult};

use crate::protocol::{handle_message, handle_query_awareness};

use super::context::DocumentContext;

pub struct Document {
    pub(super) name: String,
    pub(super) doc: Doc,
    pub(super) awareness: Awareness,

    pub(super) connections: HashMap<u64, UnboundedSender<Message>>,
}

impl Document {
    pub fn new(name: String, doc: Doc) -> Self {
        Self {
            name,
            doc,
            awareness: Awareness::new(0),

            connections: HashMap::new(),
        }
    }

    pub fn connect(
        &mut self,
        cid: u64,
        connection: UnboundedSender<Message>,
    ) -> JwstCodecResult<()> {
        let ctx = DocumentContext::new(self, connection.clone());
        handle_query_awareness(&ctx)?;

        self.connections.insert(cid, connection);

        Ok(())
    }

    pub fn disconnect(&mut self, cid: u64) {
        self.connections.remove(&cid);
    }

    pub async fn handle_message(&mut self, cid: u64, message: &[u8]) -> JwstCodecResult<()> {
        let connection = if let Some(connection) = self.connections.get(&cid) {
            connection.clone()
        } else {
            return Ok(());
        };

        let mut ctx = DocumentContext::new(self, connection);
        handle_message(&mut ctx, message).await?;

        if ctx.is_closed() {
            self.connections.remove(&cid);
        }

        Ok(())
    }

    pub(crate) fn is_connection_empty(&self) -> bool {
        self.connections.is_empty()
    }

    pub(crate) fn get_name(&self) -> &str {
        &self.name
    }
}
