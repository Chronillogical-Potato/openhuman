//! Per-sender routing and runtime command handling.

use super::context::{
    clear_sender_history, conversation_history_key, ChannelRouteSelection, ChannelRuntimeContext,
};
use super::traits;
use super::{Channel, ChannelSendExt, SendMessage};
use crate::inference::provider;
use std::sync::Arc;
use tinychannels::routes::{
    build_models_help_response, build_providers_help_response, load_cached_model_preview,
    parse_runtime_command as parse_portable_runtime_command,
    resolve_provider_alias as resolve_portable_provider_alias,
    ChannelRuntimeCommand as PortableCommand, ProviderDescriptor,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum ChannelRuntimeCommand {
    Portable(PortableCommand),
    TelegramRemote(super::providers::telegram::TelegramRemoteCommand),
}

fn supports_telegram_remote_control(channel_name: &str) -> bool {
    channel_name == "telegram"
}

fn parse_runtime_command(channel_name: &str, content: &str) -> Option<ChannelRuntimeCommand> {
    let trimmed = content.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    if supports_telegram_remote_control(channel_name) {
        if let Some(remote) =
            super::providers::telegram::remote_control::parse_telegram_remote_command(content)
        {
            return Some(ChannelRuntimeCommand::TelegramRemote(remote));
        }
    }

    parse_portable_runtime_command(channel_name, trimmed).map(ChannelRuntimeCommand::Portable)
}

fn resolve_provider_alias(name: &str) -> Option<String> {
    resolve_portable_provider_alias(name, &provider_descriptors())
}

fn provider_descriptors() -> Vec<ProviderDescriptor> {
    provider::list_providers()
        .into_iter()
        .map(|provider| ProviderDescriptor {
            name: provider.name.to_string(),
            aliases: provider.aliases.iter().map(ToString::to_string).collect(),
        })
        .collect()
}

fn default_route_selection(ctx: &ChannelRuntimeContext) -> ChannelRouteSelection {
    ChannelRouteSelection {
        provider: ctx.default_provider.as_str().to_string(),
        model: ctx.model.as_str().to_string(),
    }
}

pub(crate) fn get_route_selection(
    ctx: &ChannelRuntimeContext,
    sender_key: &str,
) -> ChannelRouteSelection {
    ctx.route_overrides
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(sender_key)
        .cloned()
        .unwrap_or_else(|| default_route_selection(ctx))
}

fn set_route_selection(ctx: &ChannelRuntimeContext, sender_key: &str, next: ChannelRouteSelection) {
    let default_route = default_route_selection(ctx);
    let mut routes = ctx
        .route_overrides
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if next == default_route {
        routes.remove(sender_key);
    } else {
        routes.insert(sender_key.to_string(), next);
    }
}

pub(crate) async fn get_or_create_turn_model_source(
    ctx: &ChannelRuntimeContext,
    provider_name: &str,
) -> anyhow::Result<crate::agent::tinyagents::TurnModelSource> {
    if provider_name == ctx.default_provider.as_str() {
        return ctx.turn_model_source.as_ref().cloned().ok_or_else(|| {
            anyhow::anyhow!("no injected channel model source for '{provider_name}'")
        });
    }

    if let Some(existing) = ctx
        .turn_model_source_cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(provider_name)
        .cloned()
    {
        return Ok(existing);
    }

    anyhow::bail!(
        "no injected channel model source for '{provider_name}'; production routes use crate-native model sources"
    )
}

pub(crate) async fn handle_runtime_command_if_needed(
    ctx: &ChannelRuntimeContext,
    msg: &traits::ChannelMessage,
    target_channel: Option<&Arc<dyn Channel>>,
) -> bool {
    let Some(command) = parse_runtime_command(&msg.channel, &msg.content) else {
        return false;
    };

    let Some(channel) = target_channel else {
        return true;
    };

    let sender_key = conversation_history_key(msg);
    let mut current = get_route_selection(ctx, &sender_key);

    let response = match command {
        ChannelRuntimeCommand::TelegramRemote(remote) => {
            super::providers::telegram::remote_control::build_remote_command_response(
                ctx, msg, remote,
            )
            .await
        }
        ChannelRuntimeCommand::Portable(PortableCommand::ShowProviders) => {
            build_providers_help_response(&current, &provider_descriptors())
        }
        ChannelRuntimeCommand::Portable(PortableCommand::SetProvider(raw_provider)) => {
            match resolve_provider_alias(&raw_provider) {
                Some(provider_name) => {
                    tracing::debug!(
                        provider = %provider_name,
                        "[channels] validated crate-native provider route from catalog"
                    );
                    if provider_name != current.provider {
                        current.provider = provider_name.clone();
                        set_route_selection(ctx, &sender_key, current.clone());
                        clear_sender_history(ctx, &sender_key);
                    }
                    format!(
                        "Provider switched to `{provider_name}` for this sender session. Current model is `{}`.\nUse `/model <model-id>` to set a provider-compatible model.",
                        current.model
                    )
                }
                None => format!(
                    "Unknown provider `{raw_provider}`. Use `/models` to list valid providers."
                ),
            }
        }
        ChannelRuntimeCommand::Portable(PortableCommand::ShowModel) => {
            build_models_help_response(&current, ctx.workspace_dir.as_path())
        }
        ChannelRuntimeCommand::Portable(PortableCommand::SetModel(raw_model)) => {
            let model = raw_model.trim().trim_matches('`').to_string();
            if model.is_empty() {
                "Model ID cannot be empty. Use `/model <model-id>`.".to_string()
            } else {
                current.model = model.clone();
                set_route_selection(ctx, &sender_key, current.clone());
                clear_sender_history(ctx, &sender_key);

                format!(
                    "Model switched to `{model}` for provider `{}` in this sender session.",
                    current.provider
                )
            }
        }
    };

    if let Err(err) = channel
        .send_with_outbound_intent(
            &SendMessage::new(response, &msg.reply_target).in_thread(msg.thread_ts.clone()),
        )
        .await
    {
        tracing::warn!(
            "Failed to send runtime command response on {}: {err}",
            channel.name()
        );
    }

    true
}

#[cfg(test)]
#[path = "routes_tests.rs"]
mod tests;
