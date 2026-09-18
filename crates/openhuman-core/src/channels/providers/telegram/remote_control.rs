//! Telegram remote-control slash commands (phase 1: `/status`, `/sessions`, `/new`).

use super::session_store::{with_store, with_store_read, TelegramChatBinding};
use crate::channels::context::{
    clear_sender_history, conversation_history_key, ChannelRouteSelection, ChannelRuntimeContext,
};
use crate::channels::traits::ChannelMessage;
use crate::memory::conversations::{self as conversations, CreateConversationThread};
pub use tinychannels::providers::telegram::TelegramRemoteCommand;
use tinychannels::providers::telegram::{
    build_new_session_response as render_new_session_response, build_remote_help_response,
    build_status_response as render_status_response, format_session_line, SESSIONS_LIST_LIMIT,
};
pub use tinychannels::providers::telegram::parse_telegram_remote_command;

const LOG_PREFIX: &str = "[telegram-remote]";

pub(crate) async fn build_remote_command_response(
    ctx: &ChannelRuntimeContext,
    msg: &ChannelMessage,
    command: TelegramRemoteCommand,
) -> String {
    tracing::debug!(
        "{LOG_PREFIX} command={command:?} reply_target={} sender={}",
        msg.reply_target,
        msg.sender
    );
    match command {
        TelegramRemoteCommand::Status => build_status_response(ctx, msg).await,
        TelegramRemoteCommand::Sessions => build_sessions_response(ctx, msg).await,
        TelegramRemoteCommand::New => build_new_session_response(ctx, msg).await,
        TelegramRemoteCommand::Help => build_remote_help_response(),
    }
}

fn route_for_sender(ctx: &ChannelRuntimeContext, sender_key: &str) -> ChannelRouteSelection {
    ctx.route_overrides
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(sender_key)
        .cloned()
        .unwrap_or_else(|| ChannelRouteSelection {
            provider: ctx.default_provider.as_str().to_string(),
            model: ctx.model.as_str().to_string(),
        })
}

async fn build_status_response(ctx: &ChannelRuntimeContext, msg: &ChannelMessage) -> String {
    let sender_key = conversation_history_key(msg);
    let route = route_for_sender(ctx, &sender_key);
    let history_len = ctx
        .conversation_histories
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&sender_key)
        .map(|h| h.len())
        .unwrap_or(0);

    let workspace = ctx.workspace_dir.clone();
    let reply_target = msg.reply_target.clone();
    // Use with_store_read (no disk write) and spawn_blocking to keep the async
    // executor thread unblocked during mutex acquisition + file I/O.
    let (binding, busy) = tokio::task::spawn_blocking(move || {
        with_store_read(&workspace, |store| {
            Ok((
                store.binding(&reply_target).cloned(),
                store.is_busy(&reply_target),
            ))
        })
    })
    .await
    .unwrap_or_else(|join_err| {
        tracing::warn!("{LOG_PREFIX} status: join error reading session store: {join_err}");
        Ok((None, false))
    })
    .unwrap_or_else(|store_err| {
        tracing::warn!("{LOG_PREFIX} status: session store error: {store_err}");
        (None, false)
    });

    let thread_line = match binding {
        Some(TelegramChatBinding {
            ref thread_id,
            ref title,
            ..
        }) => {
            // Use the stored title (captured at /new time) — O(1), no disk read.
            let display_title = title
                .as_deref()
                .filter(|t| !t.trim().is_empty())
                .unwrap_or(thread_id);
            format!("Thread: `{display_title}` (`{thread_id}`)")
        }
        None => "Thread: _(none — send `/new` to bind a thread)_".to_string(),
    };

    render_status_response(
        &thread_line,
        &route.provider,
        &route.model,
        history_len,
        busy,
    )
}

async fn build_sessions_response(ctx: &ChannelRuntimeContext, msg: &ChannelMessage) -> String {
    let workspace = ctx.workspace_dir.clone();
    let reply_target = msg.reply_target.clone();
    // Read-only lookup — use with_store_read (no save) wrapped in spawn_blocking.
    let active_thread_id = tokio::task::spawn_blocking(move || {
        with_store_read(&workspace, |store| {
            Ok(store.binding(&reply_target).map(|b| b.thread_id.clone()))
        })
    })
    .await
    .ok()
    .and_then(|res| res.ok())
    .flatten();
    let workspace = ctx.workspace_dir.as_path();

    let threads = match conversations::list_threads(workspace.to_path_buf()) {
        Ok(list) => list,
        Err(error) => {
            tracing::warn!("{LOG_PREFIX} sessions: list_threads failed: {error}");
            return format!("Could not list sessions: {error}");
        }
    };

    if threads.is_empty() {
        return "No conversation threads yet. Send `/new` to create one.".to_string();
    }

    let mut sorted = threads;
    sorted.sort_by(|a, b| b.last_message_at.cmp(&a.last_message_at));

    let mut lines = vec![
        "**Recent sessions**".to_string(),
        format!("Showing up to {SESSIONS_LIST_LIMIT} threads:"),
        String::new(),
    ];

    for thread in sorted.into_iter().take(SESSIONS_LIST_LIMIT) {
        lines.push(format_session_line(
            &thread.title,
            &thread.id,
            thread.message_count,
            active_thread_id.as_deref() == Some(thread.id.as_str()),
        ));
    }

    lines.join("\n")
}

async fn build_new_session_response(ctx: &ChannelRuntimeContext, msg: &ChannelMessage) -> String {
    let workspace = ctx.workspace_dir.as_path();
    let sender_key = conversation_history_key(msg);
    let thread_id = format!("thread-{}", uuid::Uuid::new_v4());
    let now = chrono::Utc::now();
    let title = format!(
        "Telegram {} {}",
        now.format("%b %-d"),
        now.format("%-I:%M %p")
    );
    let created_at = now.to_rfc3339();

    if let Err(error) = conversations::ensure_thread(
        workspace.to_path_buf(),
        CreateConversationThread {
            id: thread_id.clone(),
            title: title.clone(),
            created_at,
            parent_thread_id: None,
            labels: Some(vec!["telegram".to_string(), "remote".to_string()]),
            personality_id: None,
        },
    ) {
        tracing::warn!("{LOG_PREFIX} new: ensure_thread failed: {error}");
        return format!("Failed to create session: {error}");
    }

    clear_sender_history(ctx, &sender_key);

    let workspace_dir = ctx.workspace_dir.clone();
    let reply_target_owned = msg.reply_target.clone();
    let thread_id_owned = thread_id.clone();
    let sender_key_owned = sender_key.clone();
    let title_owned = title.clone();
    // Write-back path — use with_store (saves) wrapped in spawn_blocking.
    let bind_result = tokio::task::spawn_blocking(move || {
        with_store(&workspace_dir, |store| {
            store.set_binding(
                &reply_target_owned,
                thread_id_owned,
                sender_key_owned,
                Some(title_owned),
            );
            Ok(())
        })
    })
    .await
    .unwrap_or_else(|e| Err(anyhow::anyhow!("join error: {e}")));

    if let Err(error) = bind_result {
        tracing::warn!("{LOG_PREFIX} new: persist binding failed: {error}");
        return format!(
            "Created thread `{thread_id}` but failed to persist Telegram binding: {error}"
        );
    }

    crate::web_chat::invalidate_thread_sessions(&thread_id).await;

    tracing::info!(
        "{LOG_PREFIX} new session thread_id={thread_id} reply_target={} sender_key={sender_key}",
        msg.reply_target
    );

    render_new_session_response(&title, &thread_id)
}
