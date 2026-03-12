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
    let name = vars
        .get("DEPLOY_NAME")
        .map(|s| s.as_str())
        .unwrap_or("unknown");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substitute_vars_basic() {
        let mut vars = HashMap::new();
        vars.insert("NAME".to_string(), "api".to_string());
        vars.insert("STATUS".to_string(), "success".to_string());

        let result = substitute_vars("Deploy ${NAME}: ${STATUS}", &vars);
        assert_eq!(result, "Deploy api: success");
    }

    #[test]
    fn test_substitute_vars_missing() {
        let vars = HashMap::new();
        let result = substitute_vars("Deploy ${NAME}: ${STATUS}", &vars);
        assert_eq!(result, "Deploy ${NAME}: ${STATUS}");
    }

    #[test]
    fn test_substitute_vars_empty() {
        let vars = HashMap::new();
        let result = substitute_vars("no vars here", &vars);
        assert_eq!(result, "no vars here");
    }

    #[test]
    fn test_default_template_success() {
        let mut vars = HashMap::new();
        vars.insert("DEPLOY_NAME".to_string(), "api".to_string());
        vars.insert("DEPLOY_STATUS".to_string(), "success".to_string());
        vars.insert("AGENT_NAME".to_string(), "server-1".to_string());

        let result = default_template(&vars);
        assert!(result.contains("✅"));
        assert!(result.contains("api"));
        assert!(result.contains("success"));
        assert!(result.contains("server-1"));
        assert!(!result.contains("Error:"));
    }

    #[test]
    fn test_default_template_error() {
        let mut vars = HashMap::new();
        vars.insert("DEPLOY_NAME".to_string(), "api".to_string());
        vars.insert("DEPLOY_STATUS".to_string(), "error".to_string());
        vars.insert("AGENT_NAME".to_string(), "server-1".to_string());
        vars.insert("DEPLOY_ERROR".to_string(), "connection refused".to_string());

        let result = default_template(&vars);
        assert!(result.contains("❌"));
        assert!(result.contains("Error:"));
        assert!(result.contains("connection refused"));
    }

    #[test]
    fn test_default_template_empty_vars() {
        let vars = HashMap::new();
        let result = default_template(&vars);
        assert!(result.contains("unknown"));
    }

    #[test]
    fn test_silent_auto_detect_on_error() {
        let config = TelegramConfig {
            bot_token: "test".to_string(),
            chat_id: "123".to_string(),
            template: None,
            silent: None,
        };
        let mut env = HashMap::new();
        env.insert("TRIGGER_TYPE".to_string(), "on_error".to_string());

        // silent=None + TRIGGER_TYPE=on_error → silent=false
        let silent = config.silent.unwrap_or_else(|| {
            env.get("TRIGGER_TYPE")
                .map(|t| t != "on_error")
                .unwrap_or(true)
        });
        assert!(!silent);
    }

    #[test]
    fn test_silent_auto_detect_on_success() {
        let config = TelegramConfig {
            bot_token: "test".to_string(),
            chat_id: "123".to_string(),
            template: None,
            silent: None,
        };
        let mut env = HashMap::new();
        env.insert("TRIGGER_TYPE".to_string(), "on_success".to_string());

        let silent = config.silent.unwrap_or_else(|| {
            env.get("TRIGGER_TYPE")
                .map(|t| t != "on_error")
                .unwrap_or(true)
        });
        assert!(silent);
    }

    #[test]
    fn test_silent_explicit_override() {
        let config = TelegramConfig {
            bot_token: "test".to_string(),
            chat_id: "123".to_string(),
            template: None,
            silent: Some(false), // explicitly loud
        };
        let mut env = HashMap::new();
        env.insert("TRIGGER_TYPE".to_string(), "on_success".to_string());

        // Explicit silent=false overrides auto-detection
        let silent = config.silent.unwrap_or_else(|| {
            env.get("TRIGGER_TYPE")
                .map(|t| t != "on_error")
                .unwrap_or(true)
        });
        assert!(!silent);
    }

    #[test]
    fn test_silent_no_trigger_type_defaults_silent() {
        let config = TelegramConfig {
            bot_token: "test".to_string(),
            chat_id: "123".to_string(),
            template: None,
            silent: None,
        };
        let env: HashMap<String, String> = HashMap::new();

        // No TRIGGER_TYPE → default silent=true
        let silent = config.silent.unwrap_or_else(|| {
            env.get("TRIGGER_TYPE")
                .map(|t| t != "on_error")
                .unwrap_or(true)
        });
        assert!(silent);
    }
}
