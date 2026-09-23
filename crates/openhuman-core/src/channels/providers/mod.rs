//! OpenHuman channel glue plus portable provider exports.
//!
//! Provider transports belong to `tinychannels`; only Telegram's OpenHuman
//! event-bus, approval, and remote-control integration remains local.

pub use tinychannels::providers::email_channel;
pub use tinychannels::providers::lark;
pub use tinychannels::providers::{
    dingtalk, discord, imessage, irc, linq, mattermost, qq, signal, slack, whatsapp, yuanbao,
};
pub mod telegram;
#[cfg(feature = "whatsapp-web")]
pub use tinychannels::providers::whatsapp_web;
