use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::{
    config::model::{AppConfig, ClientConfig, UserConfig},
    registry::{clients::find_enabled_client_by_id, users::find_enabled_user_by_sub},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimMergePolicy {
    precedence: Vec<&'static str>,
}

impl Default for ClaimMergePolicy {
    fn default() -> Self {
        Self {
            precedence: vec!["server", "client", "user"],
        }
    }
}

impl ClaimMergePolicy {
    #[must_use]
    pub fn precedence(&self) -> &[&'static str] {
        &self.precedence
    }
}

#[must_use]
pub fn claims_for_subject(
    config: &AppConfig,
    client_id: &str,
    subject: &str,
) -> Map<String, Value> {
    let client = find_enabled_client_by_id(config, client_id);
    let user = find_enabled_user_by_sub(config, subject);
    merged_claims(config, client, user)
}

#[must_use]
pub fn merged_claims(
    config: &AppConfig,
    client: Option<&ClientConfig>,
    user: Option<&UserConfig>,
) -> Map<String, Value> {
    let mut claims = Map::new();

    if let Some(client) = client {
        apply_templates(&mut claims, config, &client.claims_template_refs);
        apply_claims(&mut claims, &client.custom_claims);
    }

    if let Some(user) = user {
        apply_standard_user_claims(&mut claims, user);
        apply_templates(&mut claims, config, &user.claims_template_refs);
        apply_claims(&mut claims, &user.custom_claims);
    }

    claims
}

fn apply_templates(
    claims: &mut Map<String, Value>,
    config: &AppConfig,
    template_refs: &[String],
) {
    for template_ref in template_refs {
        if let Some(template_claims) = config.claims_templates.get(template_ref) {
            apply_claims(claims, template_claims);
        }
    }
}

fn apply_claims(claims: &mut Map<String, Value>, values: &BTreeMap<String, Value>) {
    for (key, value) in values {
        claims.insert(key.clone(), value.clone());
    }
}

fn apply_standard_user_claims(claims: &mut Map<String, Value>, user: &UserConfig) {
    if !user.display_name.is_empty() {
        claims.insert("name".to_string(), Value::String(user.display_name.clone()));
    }
    if !user.email.is_empty() {
        claims.insert("email".to_string(), Value::String(user.email.clone()));
    }
    if !user.username.is_empty() {
        claims.insert(
            "preferred_username".to_string(),
            Value::String(user.username.clone()),
        );
    }
    if !user.roles.is_empty() {
        claims.insert(
            "roles".to_string(),
            Value::Array(
                user.roles
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect::<Vec<_>>(),
            ),
        );
    }
    if !user.groups.is_empty() {
        claims.insert(
            "groups".to_string(),
            Value::Array(
                user.groups
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect::<Vec<_>>(),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{claims_for_subject, ClaimMergePolicy};
    use crate::config::model::{AppConfig, ClientConfig, UserConfig};

    #[test]
    fn default_precedence_matches_plan() {
        let policy = ClaimMergePolicy::default();

        assert_eq!(policy.precedence(), ["server", "client", "user"]);
    }

    #[test]
    fn merges_template_client_and_user_claims() {
        let config = AppConfig {
            clients: vec![ClientConfig {
                client_id: "demo-client".to_string(),
                client_name: "Demo".to_string(),
                claims_template_refs: vec!["client-default".to_string()],
                custom_claims: std::collections::BTreeMap::from([(
                    "tenant".to_string(),
                    json!("sandbox"),
                )]),
                ..ClientConfig::default()
            }],
            users: vec![UserConfig {
                user_id: "demo-user".to_string(),
                sub: "demo-sub".to_string(),
                username: "demo".to_string(),
                email: "demo@example.com".to_string(),
                display_name: "Demo User".to_string(),
                roles: vec!["USER".to_string()],
                claims_template_refs: vec!["user-default".to_string()],
                custom_claims: std::collections::BTreeMap::from([(
                    "authorizations".to_string(),
                    json!([]),
                )]),
                ..UserConfig::default()
            }],
            claims_templates: std::collections::BTreeMap::from([
                (
                    "client-default".to_string(),
                    std::collections::BTreeMap::from([("audience".to_string(), json!("internal"))]),
                ),
                (
                    "user-default".to_string(),
                    std::collections::BTreeMap::from([("department".to_string(), json!("engineering"))]),
                ),
            ]),
            ..AppConfig::default()
        };

        let claims = claims_for_subject(&config, "demo-client", "demo-sub");

        assert_eq!(claims["tenant"], json!("sandbox"));
        assert_eq!(claims["department"], json!("engineering"));
        assert_eq!(claims["authorizations"], json!([]));
        assert_eq!(claims["preferred_username"], json!("demo"));
        assert_eq!(claims["email"], json!("demo@example.com"));
    }
}
