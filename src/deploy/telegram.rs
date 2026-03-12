use crate::config::TelegramConfig;
use std::collections::HashMap;
use tracing::{error, info};

pub struct TelegramDeploy;

impl TelegramDeploy {
    pub fn new() -> Self {
        Self
    }

    pub async fn send(
        &self,
        config: &TelegramConfig,
        env_vars: &HashMap<String, String>,
    ) -> Result<String, String> {
        let text = match &config.template {
            Some(tpl) => substitute_vars(tpl, env_vars),
            None => default_template(env_vars),
        };

        // Auto-detect silent mode from trigger type if not explicitly set
        let silent = config.silent.unwrap_or_else(|| {
            env_vars
                .get("TRIGGER_TYPE")
                .map(|t| t != "on_error")
                .unwrap_or(true)
        });

        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            config.bot_token
        );

        let body = serde_json::json!({
            "chat_id": config.chat_id,
            "text": text,
            "parse_mode": "HTML",
            "disable_notification": silent,
        });

        let resp = reqwest::Client::new()
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("Telegram API error: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            error!(status = %status, "Telegram API error");
            return Err(format!("Telegram API returned {}: {}", status, body));
        }

        info!(
            chat_id = %config.chat_id,
            silent = silent,
            "Telegram message sent"
        );
        Ok("Telegram message sent".to_string())
    }
}

fn substitute_vars(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        result = result.replace(&format!("${{{}}}", key), value);
    }
    result
}

fn default_template(vars: &HashMap<String, String>) -> String {
    let name = vars.get("DEPLOY_NAME").map(|s| s.as_str()).unwrap_or("unknown");
    let status = vars
        .get("DEPLOY_STATUS")
        .map(|s| s.as_str())
        .unwrap_or("unknown");
    let agent = vars
        .get("AGENT_NAME")
        .map(|s| s.as_str())
        .unwrap_or("unknown");
    let error_msg = vars.get("DEPLOY_ERROR").map(|s| s.as_str()).unwrap_or("");

    let emoji = if status == "success" { "✅" } else { "❌" };

    let mut msg = format!(
        "{} <b>Deploy: {}</b>\nStatus: {}\nAgent: {}",
        emoji, name, status, agent
    );
    if !error_msg.is_empty() {
        msg.push_str(&format!("\nError: <code>{}</code>", error_msg));
    }
    msg
}
