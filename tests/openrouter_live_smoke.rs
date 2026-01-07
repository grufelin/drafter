#![cfg(feature = "llm")]

use anyhow::{Context, Result};
use async_openai::{
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs, ResponseFormat, ResponseFormatJsonSchema,
    },
    Client,
};
use serde::Deserialize;
use serde_json::Value;

const OPENROUTER_API_BASE: &str = "https://openrouter.ai/api/v1";

#[derive(Debug, Deserialize)]
struct TwoResults {
    result1: i64,
    result2: i64,
}

async fn request_two_results(
    client: &Client<OpenAIConfig>,
    model: &str,
    schema: Value,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<TwoResults> {
    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages([
            ChatCompletionRequestSystemMessageArgs::default()
                .content(system_prompt)
                .build()?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(user_prompt)
                .build()?
                .into(),
        ])
        .response_format(ResponseFormat::JsonSchema {
            json_schema: ResponseFormatJsonSchema {
                name: "two_calculations".to_string(),
                description: None,
                schema: Some(schema),
                strict: Some(true),
            },
        })
        .temperature(0.0)
        .build()
        .context("failed to build OpenRouter request")?;

    let response = client
        .chat()
        .create(request)
        .await
        .context("OpenRouter request failed")?;

    let content = response
        .choices
        .get(0)
        .and_then(|c| c.message.content.as_deref())
        .context("missing choices[0].message.content")?;

    serde_json::from_str(content.trim()).context("assistant content is not valid JSON")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn openrouter_two_concurrent_requests_return_json() -> Result<()> {
    dotenvy::dotenv().ok();

    let api_key = std::env::var("OPENROUTER_API_KEY").context("OPENROUTER_API_KEY is not set")?;

    let http_referer = "https://github.com";
    let x_title = "drafter-openrouter-live-smoke";
    let model = "google/gemini-3-flash-preview";

    let config = OpenAIConfig::new()
        .with_api_key(api_key)
        .with_api_base(OPENROUTER_API_BASE);

    let config = config
        .with_header("HTTP-Referer", http_referer)
        .context("failed to set HTTP-Referer header")?;
    let config = config
        .with_header("X-Title", x_title)
        .context("failed to set X-Title header")?;

    let client = Client::with_config(config);

    let schema = serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["result1", "result2"],
        "properties": {
            "result1": {"type": "integer"},
            "result2": {"type": "integer"}
        }
    });

    let system_prompt = r#"You are a calculator.

Return ONLY valid JSON matching this schema:
{
  "type": "object",
  "additionalProperties": false,
  "required": ["result1", "result2"],
  "properties": {
    "result1": {"type": "integer"},
    "result2": {"type": "integer"}
  }
}

Rules:
- Compute both expressions exactly.
- "result1" MUST be the value of expression1.
- "result2" MUST be the value of expression2.
- Output ONLY the JSON object. No prose, no markdown.
"#;

    let user_prompt_1 = "expression1: 123 + 456\nexpression2: 27 * 19";
    let user_prompt_2 = "expression1: 1001 - 7\nexpression2: 144 / 12";

    let fut1 = request_two_results(&client, model, schema.clone(), system_prompt, user_prompt_1);
    let fut2 = request_two_results(&client, model, schema, system_prompt, user_prompt_2);

    let (res1, res2) = tokio::join!(fut1, fut2);
    let res1 = res1.context("request 1 failed")?;
    let res2 = res2.context("request 2 failed")?;

    assert_eq!(res1.result1, 579);
    assert_eq!(res1.result2, 513);

    assert_eq!(res2.result1, 994);
    assert_eq!(res2.result2, 12);

    Ok(())
}
