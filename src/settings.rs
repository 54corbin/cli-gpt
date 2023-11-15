use serde_derive::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub api_key: String,
    pub api_url: String,
}

/// `MyConfig` implements `Default`
impl ::std::default::Default for Settings {
    fn default() -> Self {
        Self {
            api_key: "<your api key>".into(),
            api_url: "https://api.openai.com/v1/chat/completions".into(),
        }
    }
}
