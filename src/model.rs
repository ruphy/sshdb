use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Host {
    pub name: String,
    #[serde(rename = "host")]
    pub address: String,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub key_path: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub remote_command: Option<String>,
    #[serde(default)]
    pub bastion: Option<String>,
    pub description: Option<String>,
}

impl Host {
    pub fn display_label(&self) -> String {
        if let Some(user) = &self.user {
            format!("{user}@{}", self.address)
        } else {
            self.address.clone()
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub version: u8,
    pub default_key: Option<String>,
    #[serde(default)]
    pub hosts: Vec<Host>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            default_key: None,
            hosts: Vec::new(),
        }
    }
}

impl Config {
    pub fn find_host(&self, name: &str) -> Option<&Host> {
        self.hosts.iter().find(|h| h.name == name)
    }

    #[cfg(test)]
    pub fn sample() -> Self {
        Self {
            version: 1,
            default_key: Some("~/.ssh/id_ed25519".to_string()),
            hosts: vec![
                Host {
                    name: "prod-web".to_string(),
                    address: "52.14.33.10".to_string(),
                    user: Some("deploy".to_string()),
                    port: Some(22),
                    key_path: Some("~/.ssh/prod_id_ed25519".to_string()),
                    tags: vec!["web".into(), "blue".into()],
                    options: Vec::new(),
                    remote_command: None,
                    description: Some("Payment frontend".into()),
                    bastion: None,
                },
                Host {
                    name: "staging-db".to_string(),
                    address: "35.12.2.4".to_string(),
                    user: Some("db".to_string()),
                    port: Some(2222),
                    key_path: None,
                    tags: vec!["db".into(), "green".into()],
                    options: Vec::new(),
                    remote_command: None,
                    description: Some("Staging database".into()),
                    bastion: Some("jump-eu".into()),
                },
                Host {
                    name: "jump-eu".to_string(),
                    address: "52.17.9.3".to_string(),
                    user: Some("ops".to_string()),
                    port: None,
                    key_path: Some("~/.ssh/jump".to_string()),
                    tags: vec!["jump".into()],
                    options: Vec::new(),
                    remote_command: None,
                    description: Some("Jump host EU".into()),
                    bastion: None,
                },
            ],
        }
    }
}
