use crate::config::model::{AppConfig, UserConfig};

const UPSTREAM_FALLBACK_USER_ID: &str = "test-user-123";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserRegistry;

impl UserRegistry {
    #[must_use]
    pub fn section_name() -> &'static str {
        "users"
    }
}

#[must_use]
pub fn enabled_users(config: &AppConfig) -> Vec<&UserConfig> {
    config.users.iter().filter(|user| user.enabled).collect()
}

#[must_use]
pub fn default_user(config: &AppConfig) -> Option<&UserConfig> {
    enabled_users(config).into_iter().next()
}

#[must_use]
pub fn effective_default_user_id(config: &AppConfig) -> String {
    default_user(config)
        .map_or_else(
            || UPSTREAM_FALLBACK_USER_ID.to_string(),
            |user| user.sub.clone(),
        )
}

#[must_use]
pub fn linked_enabled_users<'a>(config: &'a AppConfig, linked_user_ids: &[String]) -> Vec<&'a UserConfig> {
    config
        .users
        .iter()
        .filter(|user| user.enabled && linked_user_ids.iter().any(|linked| linked == &user.user_id))
        .collect()
}

#[must_use]
pub fn find_enabled_user_by_sub<'a>(config: &'a AppConfig, subject: &str) -> Option<&'a UserConfig> {
    config
        .users
        .iter()
        .find(|user| user.enabled && user.sub == subject)
}

#[must_use]
pub fn find_enabled_user_by_user_id<'a>(config: &'a AppConfig, user_id: &str) -> Option<&'a UserConfig> {
    config
        .users
        .iter()
        .find(|user| user.enabled && user.user_id == user_id)
}

#[cfg(test)]
mod tests {
    use super::{
        default_user, effective_default_user_id, enabled_users, find_enabled_user_by_user_id,
        linked_enabled_users, UserRegistry,
    };
    use crate::config::model::{AppConfig, UserConfig};

    #[test]
    fn exposes_expected_section_name() {
        assert_eq!(UserRegistry::section_name(), "users");
    }

    #[test]
    fn picks_first_enabled_user_as_default() {
        let config = AppConfig {
            users: vec![
                UserConfig {
                    user_id: "disabled".to_string(),
                    sub: "disabled-sub".to_string(),
                    enabled: false,
                    ..UserConfig::default()
                },
                UserConfig {
                    user_id: "demo-user".to_string(),
                    sub: "demo-sub".to_string(),
                    enabled: true,
                    ..UserConfig::default()
                },
            ],
            ..AppConfig::default()
        };

        let users = enabled_users(&config);
        assert_eq!(users.len(), 1);
        assert_eq!(default_user(&config).map(|user| user.user_id.as_str()), Some("demo-user"));
        assert_eq!(effective_default_user_id(&config), "demo-sub");
    }

    #[test]
    fn falls_back_to_upstream_default_when_no_user_is_enabled() {
        let config = AppConfig::default();

        assert_eq!(effective_default_user_id(&config), "test-user-123");
    }

    #[test]
    fn filters_linked_enabled_users() {
        let config = AppConfig {
            users: vec![
                UserConfig {
                    user_id: "alice".to_string(),
                    sub: "alice-sub".to_string(),
                    enabled: true,
                    ..UserConfig::default()
                },
                UserConfig {
                    user_id: "bob".to_string(),
                    sub: "bob-sub".to_string(),
                    enabled: false,
                    ..UserConfig::default()
                },
            ],
            ..AppConfig::default()
        };

        let linked = linked_enabled_users(&config, &[String::from("alice"), String::from("bob")]);
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].user_id, "alice");
    }

    #[test]
    fn finds_enabled_user_by_user_id() {
        let config = AppConfig {
            users: vec![UserConfig {
                user_id: "alice".to_string(),
                sub: "alice-sub".to_string(),
                enabled: true,
                ..UserConfig::default()
            }],
            ..AppConfig::default()
        };

        assert_eq!(
            find_enabled_user_by_user_id(&config, "alice").map(|user| user.sub.as_str()),
            Some("alice-sub")
        );
    }
}
