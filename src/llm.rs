use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};

/// System prompt for an LLM that proposes paragraph-local alternative phrasing.
///
/// The typing simulator will:
/// 1) Type the paragraph, sometimes using `alternative` in place of `original`.
/// 2) Later edit those spans back so the final editor text matches the input paragraph.
pub const PARAGRAPH_REPHRASE_SYSTEM_PROMPT: &str = r#"You are a helper for a human-like typing simulator.

Goal
- Given a single paragraph of final-draft text, propose a small set of alternative wordings.
- The simulator will temporarily type `alternative` in place of `original`, then later replace `alternative` back to `original`.
- The final text after all edits must match the input paragraph exactly.

Output format (STRICT)
- Output ONLY valid JSON. No markdown, no surrounding prose, no code fences.
- Output MUST be a JSON array (possibly empty).
- Each array element MUST be an object with exactly these keys:
  - "original": string
  - "alternative": string
- No additional keys are allowed.

Hard constraints
- `original` MUST be a contiguous substring copied verbatim from the input paragraph.
- `original` MUST occur exactly once in the input paragraph (unique match). If not, expand the span to make it unique, or omit it.
- `original` MUST NOT start or end with whitespace.
- All `original` spans MUST be non-overlapping.
- `alternative` MUST be different from `original`.
- `alternative` MUST NOT start or end with whitespace.
- Each suggestion MUST be usable as a direct substring replacement: do not require changing any text outside the span.

Character set (typing safety)
- ONLY use characters that are typeable by a US-QWERTY keyboard with ASCII input:
  - Allowed: ASCII printable characters, space, newline, and smart quotes ’ ‘ ” “.
  - Disallowed: tabs, carriage returns, and any other Unicode characters.

Quality guidance
- Prefer replacements that read naturally in context.
- Keep meaning similar unless the user explicitly asks for more dramatic rewrites.
- Return fewer items rather than violating constraints.
"#;

/// JSON Schema for `PARAGRAPH_REPHRASE_SYSTEM_PROMPT` output.
///
/// Many LLM APIs can enforce this schema via structured outputs.
pub const PARAGRAPH_REPHRASE_JSON_SCHEMA: &str = r#"{
  "type": "array",
  "items": {
    "type": "object",
    "additionalProperties": false,
    "required": ["original", "alternative"],
    "properties": {
      "original": { "type": "string" },
      "alternative": { "type": "string" }
    }
  }
}"#;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhraseAlternative {
    pub original: String,
    pub alternative: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewriteStrength {
    Subtle,
    Moderate,
    Dramatic,
}

impl RewriteStrength {
    #[cfg(feature = "llm")]
    fn user_prompt_hint(self) -> &'static str {
        match self {
            RewriteStrength::Subtle => {
                "Make small phrasing changes only; keep structure very close."
            }
            RewriteStrength::Moderate => "Allow moderate rewrites, but keep meaning the same.",
            RewriteStrength::Dramatic => {
                "Make more dramatic rewrites while keeping meaning the same."
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParagraphRephraseOptions {
    pub max_suggestions: usize,
    pub strength: RewriteStrength,
}

impl Default for ParagraphRephraseOptions {
    fn default() -> Self {
        Self {
            max_suggestions: 4,
            strength: RewriteStrength::Subtle,
        }
    }
}

fn is_supported_text(text: &str) -> bool {
    text.chars()
        .all(|c| crate::keyboard::typed_char_for_output_char(c).is_some())
}

pub fn validate_phrase_alternatives(paragraph: &str, items: &[PhraseAlternative]) -> Result<()> {
    ensure!(
        is_supported_text(paragraph),
        "paragraph contains unsupported characters"
    );

    let mut ranges: Vec<(usize, usize)> = Vec::with_capacity(items.len());

    for item in items {
        ensure!(!item.original.is_empty(), "original must not be empty");
        ensure!(
            item.original.trim() == item.original,
            "original must not start or end with whitespace"
        );

        ensure!(
            !item.alternative.is_empty(),
            "alternative must not be empty"
        );
        ensure!(
            item.alternative.trim() == item.alternative,
            "alternative must not start or end with whitespace"
        );

        ensure!(
            item.original != item.alternative,
            "original and alternative must differ"
        );

        ensure!(
            is_supported_text(&item.original),
            "original contains unsupported characters"
        );
        ensure!(
            is_supported_text(&item.alternative),
            "alternative contains unsupported characters"
        );

        let occurrences = paragraph.match_indices(&item.original).count();
        ensure!(
            occurrences == 1,
            "original must occur exactly once in the paragraph"
        );

        let start = paragraph
            .find(&item.original)
            .context("original not found in paragraph")?;
        let end = start + item.original.len();
        ranges.push((start, end));
    }

    ranges.sort_by_key(|(start, _end)| *start);

    for window in ranges.windows(2) {
        let (_prev_start, prev_end) = window[0];
        let (next_start, _next_end) = window[1];
        ensure!(
            prev_end <= next_start,
            "original spans must be non-overlapping"
        );
    }

    Ok(())
}

#[cfg(feature = "llm")]
pub mod openrouter {
    use super::*;

    use anyhow::{anyhow, Context, Result};
    use async_openai::{
        config::OpenAIConfig,
        types::chat::{
            ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
            CreateChatCompletionRequestArgs, CreateChatCompletionResponse, ResponseFormat,
            ResponseFormatJsonSchema,
        },
        Client,
    };
    use futures_util::stream::FuturesUnordered;
    use futures_util::StreamExt;
    use serde::de::DeserializeOwned;
    use serde_json::Value;
    use std::time::Duration;
    use tokio::time::sleep;

    pub const DEFAULT_MODEL: &str = "google/gemini-3-flash-preview";

    const MAX_ACTIVE_REQUESTS: usize = 10;
    const OPENROUTER_API_KEY_ENV: &str = "OPENROUTER_API_KEY";
    const OPENROUTER_API_BASE: &str = "https://openrouter.ai/api/v1";

    #[derive(Debug, Clone)]
    pub struct OpenRouterParagraphRephraseClient {
        client: Client<OpenAIConfig>,
        model: String,
        max_concurrency: usize,
        response_format: ResponseFormat,
    }

    impl OpenRouterParagraphRephraseClient {
        pub fn from_env() -> Result<Self> {
            dotenvy::dotenv().ok();
            let api_key = std::env::var(OPENROUTER_API_KEY_ENV)
                .with_context(|| format!("{OPENROUTER_API_KEY_ENV} is not set"))?;
            Self::new(api_key)
        }

        pub fn new(api_key: impl Into<String>) -> Result<Self> {
            let schema: Value = serde_json::from_str(PARAGRAPH_REPHRASE_JSON_SCHEMA)
                .context("PARAGRAPH_REPHRASE_JSON_SCHEMA must be valid JSON")?;

            let config = OpenAIConfig::new()
                .with_api_key(api_key.into())
                .with_api_base(OPENROUTER_API_BASE);

            // OpenRouter encourages these headers; set them to your app.
            let config = config
                .with_header("HTTP-Referer", "https://github.com")
                .context("failed to set HTTP-Referer header")?;
            let config = config
                .with_header("X-Title", "drafter")
                .context("failed to set X-Title header")?;

            let response_format = ResponseFormat::JsonSchema {
                json_schema: ResponseFormatJsonSchema {
                    name: "paragraph_phrase_alternatives".to_string(),
                    description: None,
                    schema: Some(schema),
                    strict: Some(true),
                },
            };

            Ok(Self {
                client: Client::with_config(config),
                model: DEFAULT_MODEL.to_string(),
                max_concurrency: MAX_ACTIVE_REQUESTS,
                response_format,
            })
        }

        pub fn with_model(mut self, model: impl Into<String>) -> Self {
            self.model = model.into();
            self
        }

        pub fn with_max_concurrency(mut self, max_concurrency: usize) -> Self {
            self.max_concurrency = max_concurrency.clamp(1, MAX_ACTIVE_REQUESTS);
            self
        }

        pub async fn rephrase_paragraph(
            &self,
            paragraph: &str,
            options: ParagraphRephraseOptions,
        ) -> Result<Vec<PhraseAlternative>> {
            request_phrase_alternatives_with_retry(self, paragraph, &options).await
        }

        pub async fn rephrase_paragraphs(
            &self,
            paragraphs: &[String],
            options: ParagraphRephraseOptions,
        ) -> Result<Vec<Vec<PhraseAlternative>>> {
            let mut results: Vec<Option<Vec<PhraseAlternative>>> = vec![None; paragraphs.len()];
            let mut in_flight: FuturesUnordered<_> = FuturesUnordered::new();

            let max_in_flight = self.max_concurrency.min(MAX_ACTIVE_REQUESTS).max(1);
            let mut next_index = 0usize;

            let initial = max_in_flight.min(paragraphs.len());
            for idx in 0..initial {
                in_flight.push(run_one(self, &paragraphs[idx], idx, options.clone()));
                next_index = idx + 1;
            }

            while let Some((idx, res)) = in_flight.next().await {
                let items =
                    res.with_context(|| format!("LLM request failed for paragraph {idx}"))?;
                results[idx] = Some(items);

                if next_index < paragraphs.len() {
                    in_flight.push(run_one(
                        self,
                        &paragraphs[next_index],
                        next_index,
                        options.clone(),
                    ));
                    next_index += 1;
                }
            }

            results
                .into_iter()
                .enumerate()
                .map(|(idx, maybe)| {
                    maybe.ok_or_else(|| anyhow!("missing result for paragraph {idx}"))
                })
                .collect()
        }
    }

    async fn run_one(
        client: &OpenRouterParagraphRephraseClient,
        paragraph: &str,
        idx: usize,
        options: ParagraphRephraseOptions,
    ) -> (usize, Result<Vec<PhraseAlternative>>) {
        let res = request_phrase_alternatives_with_retry(client, paragraph, &options).await;
        (idx, res)
    }

    async fn request_phrase_alternatives_with_retry(
        client: &OpenRouterParagraphRephraseClient,
        paragraph: &str,
        options: &ParagraphRephraseOptions,
    ) -> Result<Vec<PhraseAlternative>> {
        let retry_delays = [Duration::from_secs(0), Duration::from_secs(10)];

        let mut attempt = 0usize;
        loop {
            match request_phrase_alternatives_once(client, paragraph, options).await {
                Ok(items) => return Ok(items),
                Err(err) => {
                    if attempt >= retry_delays.len() {
                        return Err(err).context("LLM request failed after retries");
                    }

                    let delay = retry_delays[attempt];
                    attempt += 1;
                    if delay > Duration::from_secs(0) {
                        sleep(delay).await;
                    }
                }
            }
        }
    }

    async fn request_phrase_alternatives_once(
        client: &OpenRouterParagraphRephraseClient,
        paragraph: &str,
        options: &ParagraphRephraseOptions,
    ) -> Result<Vec<PhraseAlternative>> {
        let user_prompt = build_user_prompt(paragraph, options);

        let mut items =
            request_phrase_alternatives_once_typed(client, user_prompt.as_str()).await?;

        if items.len() > options.max_suggestions {
            items.truncate(options.max_suggestions);
        }

        validate_phrase_alternatives(paragraph, &items).context("LLM output failed validation")?;

        Ok(items)
    }

    async fn request_phrase_alternatives_once_typed(
        client: &OpenRouterParagraphRephraseClient,
        user_prompt: &str,
    ) -> Result<Vec<PhraseAlternative>> {
        let request = CreateChatCompletionRequestArgs::default()
            .model(client.model.as_str())
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
            .response_format(client.response_format.clone())
            .temperature(0.0)
            .build()
            .context("failed to build OpenRouter request")?;

        let response = client
            .client
            .chat()
            .create(request)
            .await
            .context("OpenRouter chat completion request failed")?;

        parse_chat_completion_json(&response).context("failed to parse structured output")
    }

    fn parse_chat_completion_json<T: DeserializeOwned>(
        response: &CreateChatCompletionResponse,
    ) -> Result<T> {
        let content = response
            .choices
            .get(0)
            .and_then(|c| c.message.content.as_deref())
            .context("missing choices[0].message.content")?;

        serde_json::from_str::<T>(content.trim()).context("assistant content is not valid JSON")
    }

    fn build_user_prompt(paragraph: &str, options: &ParagraphRephraseOptions) -> String {
        format!(
            "Input paragraph:\n{paragraph}\n\nConstraints:\n- Return up to {max} suggestions.\n- {strength}\n\nReturn ONLY the JSON array.",
            max = options.max_suggestions,
            strength = options.strength.user_prompt_hint(),
        )
    }
}

#[cfg(not(feature = "llm"))]
pub mod openrouter {
    use super::*;

    use anyhow::{anyhow, Result};

    pub const DEFAULT_MODEL: &str = "google/gemini-3-flash-preview";

    #[derive(Debug, Clone)]
    pub struct OpenRouterParagraphRephraseClient;

    impl OpenRouterParagraphRephraseClient {
        pub fn from_env() -> Result<Self> {
            Err(anyhow!(
                "LLM support is disabled (build with --features llm)"
            ))
        }

        pub fn new(_api_key: impl Into<String>) -> Result<Self> {
            Err(anyhow!(
                "LLM support is disabled (build with --features llm)"
            ))
        }

        pub fn with_model(self, _model: impl Into<String>) -> Self {
            self
        }

        pub fn with_max_concurrency(self, _max_concurrency: usize) -> Self {
            self
        }

        pub async fn rephrase_paragraph(
            &self,
            _paragraph: &str,
            _options: ParagraphRephraseOptions,
        ) -> Result<Vec<PhraseAlternative>> {
            Err(anyhow!(
                "LLM support is disabled (build with --features llm)"
            ))
        }

        pub async fn rephrase_paragraphs(
            &self,
            _paragraphs: &[String],
            _options: ParagraphRephraseOptions,
        ) -> Result<Vec<Vec<PhraseAlternative>>> {
            Err(anyhow!(
                "LLM support is disabled (build with --features llm)"
            ))
        }
    }
}
