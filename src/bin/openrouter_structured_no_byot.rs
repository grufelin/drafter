#[cfg(not(feature = "llm"))]
fn main() {
    eprintln!("This binary requires `--features llm`.");
    std::process::exit(1);
}

#[cfg(feature = "llm")]
mod llm_main {
    use anyhow::{Context, Result};
    use async_openai::{
        config::OpenAIConfig,
        types::chat::{
            ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
            CreateChatCompletionRequestArgs, ResponseFormat, ResponseFormatJsonSchema,
        },
        Client,
    };
    use drafter::llm::{
        validate_phrase_alternatives, ParagraphRephraseOptions, PhraseAlternative, RewriteStrength,
        PARAGRAPH_REPHRASE_JSON_SCHEMA, PARAGRAPH_REPHRASE_SYSTEM_PROMPT,
    };
    use serde_json::Value;

    const OPENROUTER_API_BASE: &str = "https://openrouter.ai/api/v1";

    pub async fn run() -> Result<()> {
        dotenvy::dotenv().ok();

        let api_key =
            std::env::var("OPENROUTER_API_KEY").context("OPENROUTER_API_KEY is not set")?;
        let model = std::env::var("OPENROUTER_MODEL")
            .unwrap_or_else(|_| "google/gemini-3-flash-preview".to_string());

        let paragraph = "This is a small experiment to confirm whether OpenRouter structured outputs work without a structured_outputs flag.";

        let options = ParagraphRephraseOptions {
            max_suggestions: 4,
            strength: RewriteStrength::Subtle,
        };

        let schema: Value = serde_json::from_str(PARAGRAPH_REPHRASE_JSON_SCHEMA)
            .context("PARAGRAPH_REPHRASE_JSON_SCHEMA must be valid JSON")?;

        let response_format = ResponseFormat::JsonSchema {
            json_schema: ResponseFormatJsonSchema {
                name: "paragraph_phrase_alternatives".to_string(),
                description: Some("Array of {original, alternative} suggestions".to_string()),
                schema: Some(schema),
                strict: Some(true),
            },
        };

        let user_prompt = format!(
            "Input paragraph:\n{paragraph}\n\nConstraints:\n- Return up to {max} suggestions.\n- {strength}\n\nReturn ONLY the JSON array.",
            max = options.max_suggestions,
            strength = match options.strength {
                RewriteStrength::Subtle => {
                    "Make small phrasing changes only; keep structure very close."
                }
                RewriteStrength::Moderate => "Allow moderate rewrites, but keep meaning the same.",
                RewriteStrength::Dramatic => {
                    "Make more dramatic rewrites while keeping meaning the same."
                }
            }
        );

        let config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base(OPENROUTER_API_BASE);

        let config = config
            .with_header("HTTP-Referer", "https://github.com")
            .context("failed to set HTTP-Referer header")?;
        let config = config
            .with_header("X-Title", "drafter-openrouter-no-byot")
            .context("failed to set X-Title header")?;

        let client = Client::with_config(config);

        let request = CreateChatCompletionRequestArgs::default()
            .model(model.as_str())
            .messages([
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(PARAGRAPH_REPHRASE_SYSTEM_PROMPT)
                    .build()?
                    .into(),
                ChatCompletionRequestUserMessageArgs::default()
                    .content(user_prompt)
                    .build()?
                    .into(),
            ])
            .response_format(response_format)
            .temperature(0.0)
            .build()
            .context("failed to build chat completion request")?;

        let response = client
            .chat()
            .create(request)
            .await
            .context("chat completion request failed")?;

        let content = response
            .choices
            .get(0)
            .and_then(|c| c.message.content.as_deref())
            .context("missing choices[0].message.content")?;

        println!("Model: {model}\n");
        println!("Raw assistant content:\n{content}\n");

        let items: Vec<PhraseAlternative> =
            serde_json::from_str(content).context("assistant content is not valid JSON")?;

        println!("Parsed {} phrase alternatives", items.len());

        validate_phrase_alternatives(paragraph, &items).context("output failed validation")?;

        println!("Validation OK");

        Ok(())
    }
}

#[cfg(feature = "llm")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    llm_main::run().await
}
