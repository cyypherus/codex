use codex_core::protocol_config_types::ReasoningEffort;
use codex_protocol::mcp_protocol::AuthMode;

/// A simple preset pairing a model slug with a reasoning effort.
#[derive(Debug, Clone, Copy)]
pub struct ModelPreset {
    /// Stable identifier for the preset.
    pub id: &'static str,
    /// Display label shown in UIs.
    pub label: &'static str,
    /// Short human description shown next to the label in UIs.
    pub description: &'static str,
    /// Model slug (e.g., "gpt-5").
    pub model: &'static str,
    /// Reasoning effort to apply for this preset.
    pub effort: Option<ReasoningEffort>,
    /// Provider this model belongs to (e.g., "openai", "githubcopilot").
    pub provider: Option<&'static str>,
}

const OPENAI_PRESETS: &[ModelPreset] = &[
    ModelPreset {
        id: "gpt-5-codex-low",
        label: "gpt-5-codex low",
        description: "",
        model: "gpt-5-codex",
        effort: Some(ReasoningEffort::Low),
        provider: Some("openai"),
    },
    ModelPreset {
        id: "gpt-5-codex-medium",
        label: "gpt-5-codex medium",
        description: "",
        model: "gpt-5-codex",
        effort: None,
        provider: Some("openai"),
    },
    ModelPreset {
        id: "gpt-5-codex-high",
        label: "gpt-5-codex high",
        description: "",
        model: "gpt-5-codex",
        effort: Some(ReasoningEffort::High),
        provider: Some("openai"),
    },
    ModelPreset {
        id: "gpt-5-minimal",
        label: "gpt-5 minimal",
        description: "— fastest responses with limited reasoning; ideal for coding, instructions, or lightweight tasks",
        model: "gpt-5",
        effort: Some(ReasoningEffort::Minimal),
        provider: Some("openai"),
    },
    ModelPreset {
        id: "gpt-5-low",
        label: "gpt-5 low",
        description: "— balances speed with some reasoning; useful for straightforward queries and short explanations",
        model: "gpt-5",
        effort: Some(ReasoningEffort::Low),
        provider: Some("openai"),
    },
    ModelPreset {
        id: "gpt-5-medium",
        label: "gpt-5 medium",
        description: "— default setting; provides a solid balance of reasoning depth and latency for general-purpose tasks",
        model: "gpt-5",
        effort: Some(ReasoningEffort::Medium),
        provider: Some("openai"),
    },
    ModelPreset {
        id: "gpt-5-high",
        label: "gpt-5 high",
        description: "— maximizes reasoning depth for complex or ambiguous problems",
        model: "gpt-5",
        effort: Some(ReasoningEffort::High),
        provider: Some("openai"),
    },
];

const COPILOT_PRESETS: &[ModelPreset] = &[
    ModelPreset {
        id: "copilot-gpt-5-codex",
        label: "GPT-5-Codex",
        description: "— OpenAI's latest coding model (Public preview)",
        model: "gpt-5-codex",
        effort: None,
        provider: Some("githubcopilot"),
    },
    ModelPreset {
        id: "copilot-gpt-5",
        label: "GPT-5",
        description: "— OpenAI's most capable model (GA)",
        model: "gpt-5",
        effort: None,
        provider: Some("githubcopilot"),
    },
    ModelPreset {
        id: "copilot-gpt-5-mini",
        label: "GPT-5 mini",
        description: "— Fast and efficient model for lightweight tasks (GA)",
        model: "gpt-5-mini",
        effort: None,
        provider: Some("githubcopilot"),
    },
    ModelPreset {
        id: "copilot-gpt-4.1",
        label: "GPT-4.1",
        description: "— Previous generation high-intelligence model (GA)",
        model: "gpt-4.1",
        effort: None,
        provider: Some("githubcopilot"),
    },
    ModelPreset {
        id: "copilot-claude-sonnet-4.5",
        label: "Claude Sonnet 4.5",
        description: "— Anthropic's latest Sonnet model (Public preview)",
        model: "claude-sonnet-4.5",
        effort: None,
        provider: Some("githubcopilot"),
    },
    ModelPreset {
        id: "copilot-claude-opus-4.1",
        label: "Claude Opus 4.1",
        description: "— Anthropic's most powerful model (GA)",
        model: "claude-opus-4.1",
        effort: None,
        provider: Some("githubcopilot"),
    },
    ModelPreset {
        id: "copilot-claude-sonnet-4",
        label: "Claude Sonnet 4",
        description: "— Anthropic's balanced Sonnet model (GA)",
        model: "claude-sonnet-4",
        effort: None,
        provider: Some("githubcopilot"),
    },
    ModelPreset {
        id: "copilot-claude-sonnet-3.5",
        label: "Claude Sonnet 3.5",
        description: "— Anthropic's proven Sonnet model (GA)",
        model: "claude-sonnet-3.5",
        effort: None,
        provider: Some("githubcopilot"),
    },
    ModelPreset {
        id: "copilot-gemini-2.5-pro",
        label: "Gemini 2.5 Pro",
        description: "— Google's most capable model (GA)",
        model: "gemini-2.5-pro",
        effort: None,
        provider: Some("githubcopilot"),
    },
    ModelPreset {
        id: "copilot-grok-code-fast-1",
        label: "Grok Code Fast 1",
        description: "— xAI's fast coding model (Public preview)",
        model: "grok-code-fast-1",
        effort: None,
        provider: Some("githubcopilot"),
    },
];

pub fn builtin_model_presets(_auth_mode: Option<AuthMode>) -> Vec<ModelPreset> {
    OPENAI_PRESETS.to_vec()
}

pub fn builtin_model_presets_for_provider(provider: &str) -> Vec<ModelPreset> {
    match provider.to_lowercase().as_str() {
        "githubcopilot" => COPILOT_PRESETS.to_vec(),
        "openai" => OPENAI_PRESETS.to_vec(),
        _ => OPENAI_PRESETS.to_vec(),
    }
}
