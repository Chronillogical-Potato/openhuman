//! SDK-owned Socket.IO transport that implements the core's socket port.

use openhuman_embed::__host::platform::socket::TokenProvider;

fn sdk_token_provider(
    provider: TokenProvider,
) -> impl Fn() -> Option<String> + Send + Sync + 'static {
    move || {
        let provider = provider.clone();
        openhuman_embed::__host::storage::block_on_anyhow(async move {
            provider()
                .await
                .map(Some)
                .map_err(anyhow::Error::msg)
        })
        .ok()
        .flatten()
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
