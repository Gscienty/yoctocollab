use std::sync::{Arc, Mutex};

use y_octo::{
    read_var_string, read_var_u64, write_sync_message, write_var_string, AwarenessEvent,
    JwstCodecError, JwstCodecResult, SyncMessage,
};

use super::{
    awareness::read_awareness_update,
    context::Context,
    message_type::{DocMessage, MessageType},
    sync::{
        read_sync_step1, read_sync_step2, read_sync_update, write_sync_status, write_sync_step1,
        write_sync_step2, write_sync_update,
    },
};

pub async fn handle_message<CTX: Context>(ctx: &mut CTX, message: &[u8]) -> JwstCodecResult<()> {
    let (tail, name) = read_var_string_inline(message)?;

    if ctx.get_document_name() != name {
        return Err(JwstCodecError::RootStructNotFound(format!(
            "not found document name `{name}`"
        )));
    }

    let (tail, typ) = read_var_u64_inline(tail)?;
    let typ: MessageType = typ.try_into()?;
    match typ {
        MessageType::Sync | MessageType::SyncReply => {
            handle_sync_message(ctx, tail)?;
        }
        MessageType::Awareness => {
            handle_awareness_message(ctx, tail)?;
        }
        MessageType::QueryAwareness => {
            handle_query_awareness(ctx)?;
        }
        MessageType::Stateless => {}
        MessageType::BroadcastStateless => {}
        MessageType::Auth => {
            // server ignore, maybe custom impl it
        }
        MessageType::Close => {
            ctx.close().await;
        }
        MessageType::SyncStatus => {
            // server ignore
        }
    }

    Ok(())
}

fn handle_sync_message<CTX: Context>(ctx: &mut CTX, message: &[u8]) -> JwstCodecResult<()> {
    let (tail, typ) = read_var_u64_inline(message)?;
    let typ: DocMessage = typ.try_into()?;

    match typ {
        DocMessage::Step1 => {
            let state_vector = read_sync_step1(tail)?;

            let doc = write_sync_step1(ctx.get_document())?;
            ctx.unicast([message_header(ctx)?, doc].concat());

            let update = write_sync_step2(ctx.get_document(), &state_vector)?;
            ctx.unicast([message_header(ctx)?, update].concat());
        }
        DocMessage::Step2 => {
            let update = read_sync_step2(tail)?;
            let broadcast_update = write_sync_update(&update)?;
            ctx.get_document_mut().apply_update_from_binary(update)?;

            ctx.broadcast([message_header(ctx)?, broadcast_update].concat());
            ctx.unicast([message_header(ctx)?, write_sync_status(true)?].concat());
        }
        DocMessage::Update => {
            let update = read_sync_update(tail)?;
            let broadcast_update = write_sync_update(&update)?;
            ctx.get_document_mut().apply_update_from_binary(update)?;

            ctx.broadcast([message_header(ctx)?, broadcast_update].concat());
            ctx.unicast([message_header(ctx)?, write_sync_status(true)?].concat());
        }
    }

    Ok(())
}

fn handle_awareness_message<CTX: Context>(ctx: &mut CTX, message: &[u8]) -> JwstCodecResult<()> {
    let update = read_awareness_update(message)?;

    // callback
    let values: Arc<Mutex<Vec<AwarenessEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let callback_values = Arc::clone(&values);
    ctx.get_awareness_mut().on_update(move |_, event| {
        let mut values = callback_values.lock().unwrap();
        values.push(event);
    });
    ctx.get_awareness_mut().apply_update(update);

    let states = values
        .lock()
        .unwrap()
        .first()
        .map(|value| value.get_updated(ctx.get_awareness().get_states()));
    if let Some(states) = states {
        let mut buffer = Vec::new();
        write_sync_message(&mut buffer, &SyncMessage::Awareness(states))
            .map_err(|err| JwstCodecError::InvalidWriteBuffer(err.to_string()))?;

        ctx.broadcast([message_header(ctx)?, buffer].concat());
    }

    Ok(())
}

pub fn handle_query_awareness<CTX: Context>(ctx: &CTX) -> JwstCodecResult<()> {
    if ctx.get_awareness().get_states().is_empty() {
        return Ok(());
    }

    let states = ctx.get_awareness().get_states().clone();
    let mut buffer = Vec::new();
    write_sync_message(&mut buffer, &SyncMessage::Awareness(states))
        .map_err(|err| JwstCodecError::InvalidWriteBuffer(err.to_string()))?;

    ctx.unicast([message_header(ctx)?, buffer].concat());

    Ok(())
}

#[inline]
fn message_header<CTX: Context>(ctx: &CTX) -> JwstCodecResult<Vec<u8>> {
    let mut head = Vec::with_capacity(9 + ctx.get_document_name().len());
    write_var_string(&mut head, ctx.get_document_name())
        .map_err(|err| JwstCodecError::InvalidWriteBuffer(err.to_string()))?;

    Ok(head)
}

#[inline]
fn read_var_u64_inline(buffer: &[u8]) -> JwstCodecResult<(&[u8], u64)> {
    let (tail, value) = read_var_u64(buffer).map_err(|err| err.map_input(|u| u.len()))?;

    Ok((tail, value))
}

#[inline]
fn read_var_string_inline(buffer: &[u8]) -> JwstCodecResult<(&[u8], String)> {
    let (tail, value) = read_var_string(buffer).map_err(|err| err.map_input(|u| u.len()))?;

    Ok((tail, value))
}
