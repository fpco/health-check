use anyhow::{anyhow, Context, Result};
use reqwest::Url;

pub(crate) struct SlackApp {
    webhook: Url,
    app_info: AppDetail,
}

pub(crate) struct AppDetail {
    pub(crate) message: String,
    pub(crate) description: String,
    pub(crate) version: String,
    pub(crate) image_url: Option<String>,
}

fn readable_image_id(version: &str) -> &str {
    match version.split(':').last() {
        Some(last) => last,
        None => version,
    }
}

impl SlackApp {
    pub(crate) fn new(
        webhook: Url,
        message: String,
        description: String,
        version: String,
        image_url: Option<String>,
    ) -> SlackApp {
        SlackApp {
            webhook,
            app_info: AppDetail {
                message,
                description,
                version,
                image_url,
            },
        }
    }

    fn compute_description(&self) -> String {
        let version = readable_image_id(&self.app_info.version);
        // Handle newline so that Slack renders it properly
        let message = self.app_info.message.replace("\\n", "\n");
        format!(
            "{} \n *Application*: {} \n *Version*: {}",
            message, self.app_info.description, version
        )
    }

    pub(crate) fn send_notification(&self, message: &anyhow::Error) -> Result<()> {
        let description = self.compute_description();
        let mut value = serde_json::json!(
        {
            "text": "Health check alert",
            "blocks": [
                {
                    "type": "header",
                    "text": {
                        "type": "plain_text",
                        "text": message.to_string(),
                    }
                },
                {
                    "type": "section",
                    "block_id": "section567",
                    "text": {
                        "type": "mrkdwn",
                        "text": description
                    },
                }
            ]
        });
        if let Some(image_url) = &self.app_info.image_url {
            let object = value
                .as_object_mut()
                .context("JSON value should be an object")?;
            let blocks = object["blocks"]
                .as_array_mut()
                .context("Blocks field should be an array")?;
            let section = blocks[1]
                .as_object_mut()
                .context("Second block should be a section object")?;
            section.insert(
                "accessory".to_owned(),
                serde_json::json!(
                    {
                        "type": "image",
                        "image_url": image_url,
                        "alt_text": "Health check image".to_owned()
                    }
                ),
            );
        }
        let client = reqwest::blocking::Client::new();
        let response = client.post(self.webhook.clone()).json(&value).send()?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow!(
                "Slack notification POST request failed with code {}",
                response.status()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::slack::readable_image_id;

    #[test]
    fn guess_readable_image_id_works() {
        let image_id =
            readable_image_id("ghcr.io/fpco/some-app:d5def5afc6030dda860a79f231b295e2e412bc28");
        assert_eq!(image_id, "d5def5afc6030dda860a79f231b295e2e412bc28");

        let image_id = readable_image_id("d5def5afc6030dda860a79f231b295e2e412bc28");
        assert_eq!(image_id, "d5def5afc6030dda860a79f231b295e2e412bc28");
    }
}
