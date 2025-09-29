use crate::services::ai_analysis_service::{
    ChangeAnalysis, ChangeCategory, ChangeType, ImpactLevel, RiskFactor,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum LlmProvider {
    OpenAi,
    Anthropic,
    Azure,
    OpenRouter,    // OpenRouter with multi-model access
    Local(String), // URL for local models
}

#[derive(Debug, Clone)]
pub enum AnalysisType {
    DocumentDiff,
    Chat,
    Summarization,
    RiskAssessment,
    Insights,
    Classification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    pub prompt: String,
    pub context: Option<String>,
    pub max_tokens: Option<i32>,
    pub temperature: Option<f32>,
    pub system_prompt: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: String,
    pub usage_tokens: Option<i32>,
    pub model_used: String,
    pub confidence_score: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentAnalysisPrompt {
    pub current_content: String,
    pub previous_content: Option<String>,
    pub document_type: String,
    pub analysis_focus: Vec<String>,
    pub context: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatPrompt {
    pub user_message: String,
    pub conversation_history: Vec<ChatMessage>,
    pub document_context: Option<String>,
    pub analysis_context: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub struct LlmService {
    client: Client,
    provider: LlmProvider,
    api_key: String,
    base_url: String,
    current_analysis_type: Option<AnalysisType>,
}

impl LlmService {
    pub fn new(provider: LlmProvider) -> Result<Self, Box<dyn std::error::Error>> {
        let client = Client::new();

        let (api_key, base_url) = match &provider {
            LlmProvider::OpenAi => {
                let key = env::var("OPENAI_API_KEY").map_err(|_| "OPENAI_API_KEY not set")?;
                (key, "https://api.openai.com/v1".to_string())
            }
            LlmProvider::Anthropic => {
                let key = env::var("ANTHROPIC_API_KEY").map_err(|_| "ANTHROPIC_API_KEY not set")?;
                (key, "https://api.anthropic.com/v1".to_string())
            }
            LlmProvider::Azure => {
                let key = env::var("AZURE_OPENAI_KEY").map_err(|_| "AZURE_OPENAI_KEY not set")?;
                let endpoint = env::var("AZURE_OPENAI_ENDPOINT")
                    .map_err(|_| "AZURE_OPENAI_ENDPOINT not set")?;
                (key, endpoint)
            }
            LlmProvider::OpenRouter => {
                let key =
                    env::var("OPENROUTER_API_KEY").map_err(|_| "OPENROUTER_API_KEY not set")?;
                (key, "https://openrouter.ai/api/v1".to_string())
            }
            LlmProvider::Local(url) => ("local".to_string(), url.clone()),
        };

        Ok(Self {
            client,
            provider,
            api_key,
            base_url,
            current_analysis_type: None,
        })
    }

    /// Analyze document differences using AI
    pub async fn analyze_document_changes(
        &mut self,
        prompt: DocumentAnalysisPrompt,
    ) -> Result<Vec<ChangeAnalysis>, Box<dyn std::error::Error>> {
        self.current_analysis_type = Some(AnalysisType::DocumentDiff);

        let analysis_prompt = self.build_document_analysis_prompt(&prompt)?;

        let llm_request = LlmRequest {
            prompt: analysis_prompt,
            context: None,
            max_tokens: Some(4000),
            temperature: Some(0.3), // Lower temperature for more consistent analysis
            system_prompt: Some(self.get_document_analysis_system_prompt()),
        };

        let response = self.call_llm_with_fallback(llm_request).await?;
        let changes = self.parse_analysis_response(&response, &prompt)?;

        Ok(changes)
    }

    /// Generate AI chat response
    pub async fn generate_chat_response(
        &mut self,
        prompt: ChatPrompt,
    ) -> Result<LlmResponse, Box<dyn std::error::Error>> {
        self.current_analysis_type = Some(AnalysisType::Chat);

        let chat_prompt = self.build_chat_prompt(&prompt)?;

        let llm_request = LlmRequest {
            prompt: chat_prompt,
            context: prompt.document_context,
            max_tokens: Some(1000),
            temperature: Some(0.7),
            system_prompt: Some(self.get_chat_system_prompt()),
        };

        self.call_llm_with_fallback(llm_request).await
    }

    /// Generate negotiation insights
    pub async fn generate_insights(
        &mut self,
        document_content: &str,
        recent_changes: &[ChangeAnalysis],
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        self.current_analysis_type = Some(AnalysisType::Insights);

        let insights_prompt = self.build_insights_prompt(document_content, recent_changes)?;

        let llm_request = LlmRequest {
            prompt: insights_prompt,
            context: None,
            max_tokens: Some(2000),
            temperature: Some(0.4),
            system_prompt: Some(self.get_insights_system_prompt()),
        };

        let response = self.call_llm_with_fallback(llm_request).await?;
        let insights = self.parse_insights_response(&response)?;

        Ok(insights)
    }

    // Private methods

    async fn call_llm(
        &self,
        request: LlmRequest,
    ) -> Result<LlmResponse, Box<dyn std::error::Error>> {
        match &self.provider {
            LlmProvider::OpenAi => self.call_openai(request).await,
            LlmProvider::Anthropic => self.call_anthropic(request).await,
            LlmProvider::Azure => self.call_azure_openai(request).await,
            LlmProvider::OpenRouter => self.call_openrouter(request).await,
            LlmProvider::Local(_) => self.call_local_model(request).await,
        }
    }

    /// Call LLM with intelligent fallback for OpenRouter
    async fn call_llm_with_fallback(
        &self,
        request: LlmRequest,
    ) -> Result<LlmResponse, Box<dyn std::error::Error>> {
        match &self.provider {
            LlmProvider::OpenRouter => {
                let analysis_type = self
                    .current_analysis_type
                    .as_ref()
                    .cloned()
                    .unwrap_or(AnalysisType::Chat);
                let models_to_try = self.get_fallback_models_for_type(analysis_type);

                for (i, model) in models_to_try.iter().enumerate() {
                    match self
                        .call_openrouter_with_model(request.clone(), model)
                        .await
                    {
                        Ok(response) => {
                            if i > 0 {
                                println!("Successfully failed over to model: {}", model);
                            }
                            return Ok(response);
                        }
                        Err(e) => {
                            eprintln!("Model {} failed: {}. Trying next...", model, e);
                            continue;
                        }
                    }
                }
                Err("All OpenRouter models failed".into())
            }
            _ => self.call_llm(request).await,
        }
    }

    async fn call_openai(
        &self,
        request: LlmRequest,
    ) -> Result<LlmResponse, Box<dyn std::error::Error>> {
        #[derive(Serialize)]
        struct OpenAiRequest {
            model: String,
            messages: Vec<OpenAiMessage>,
            max_tokens: Option<i32>,
            temperature: Option<f32>,
        }

        #[derive(Serialize, Deserialize)]
        struct OpenAiMessage {
            role: String,
            content: String,
        }

        #[derive(Deserialize)]
        struct OpenAiResponse {
            choices: Vec<OpenAiChoice>,
            usage: Option<OpenAiUsage>,
        }

        #[derive(Deserialize)]
        struct OpenAiChoice {
            message: OpenAiMessage,
        }

        #[derive(Deserialize)]
        struct OpenAiUsage {
            total_tokens: i32,
        }

        let mut messages = vec![];

        if let Some(system_prompt) = request.system_prompt {
            messages.push(OpenAiMessage {
                role: "system".to_string(),
                content: system_prompt,
            });
        }

        messages.push(OpenAiMessage {
            role: "user".to_string(),
            content: request.prompt,
        });

        let openai_request = OpenAiRequest {
            model: "gpt-4".to_string(),
            messages,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
        };

        let response = self
            .client
            .post(&format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&openai_request)
            .send()
            .await?;

        let openai_response: OpenAiResponse = response.json().await?;

        let content = openai_response
            .choices
            .first()
            .ok_or("No choices in response")?
            .message
            .content
            .clone();

        Ok(LlmResponse {
            content,
            usage_tokens: openai_response.usage.map(|u| u.total_tokens),
            model_used: "gpt-4".to_string(),
            confidence_score: 0.85, // Default confidence for GPT-4
        })
    }

    async fn call_anthropic(
        &self,
        request: LlmRequest,
    ) -> Result<LlmResponse, Box<dyn std::error::Error>> {
        #[derive(Serialize)]
        struct AnthropicRequest {
            model: String,
            max_tokens: i32,
            messages: Vec<AnthropicMessage>,
            system: Option<String>,
            temperature: Option<f32>,
        }

        #[derive(Serialize)]
        struct AnthropicMessage {
            role: String,
            content: String,
        }

        #[derive(Deserialize)]
        struct AnthropicResponse {
            content: Vec<AnthropicContent>,
            usage: Option<AnthropicUsage>,
        }

        #[derive(Deserialize)]
        struct AnthropicContent {
            text: String,
        }

        #[derive(Deserialize)]
        struct AnthropicUsage {
            input_tokens: i32,
            output_tokens: i32,
        }

        let anthropic_request = AnthropicRequest {
            model: "claude-3-sonnet-20240229".to_string(),
            max_tokens: request.max_tokens.unwrap_or(1000),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: request.prompt,
            }],
            system: request.system_prompt,
            temperature: request.temperature,
        };

        let response = self
            .client
            .post(&format!("{}/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&anthropic_request)
            .send()
            .await?;

        let anthropic_response: AnthropicResponse = response.json().await?;

        let content = anthropic_response
            .content
            .first()
            .ok_or("No content in response")?
            .text
            .clone();

        let total_tokens = anthropic_response
            .usage
            .map(|u| u.input_tokens + u.output_tokens);

        Ok(LlmResponse {
            content,
            usage_tokens: total_tokens,
            model_used: "claude-3-sonnet".to_string(),
            confidence_score: 0.88, // Default confidence for Claude
        })
    }

    async fn call_azure_openai(
        &self,
        request: LlmRequest,
    ) -> Result<LlmResponse, Box<dyn std::error::Error>> {
        // Similar to OpenAI but with Azure-specific endpoint and authentication
        // Implementation would be similar to call_openai but with Azure headers
        self.call_openai(request).await // Placeholder - would implement Azure-specific logic
    }

    async fn call_openrouter(
        &self,
        request: LlmRequest,
    ) -> Result<LlmResponse, Box<dyn std::error::Error>> {
        // For OpenRouter, we'll use the fallback mechanism by default
        let analysis_type = self
            .current_analysis_type
            .as_ref()
            .cloned()
            .unwrap_or(AnalysisType::Chat);
        let models = self.get_fallback_models_for_type(analysis_type);
        let primary_model = models
            .first()
            .ok_or("No models available for analysis type")?;

        self.call_openrouter_with_model(request, primary_model)
            .await
    }

    async fn call_openrouter_with_model(
        &self,
        request: LlmRequest,
        model: &str,
    ) -> Result<LlmResponse, Box<dyn std::error::Error>> {
        #[derive(Serialize)]
        struct OpenRouterRequest {
            model: String,
            messages: Vec<OpenRouterMessage>,
            temperature: Option<f32>,
            max_tokens: Option<i32>,
        }

        #[derive(Serialize)]
        struct OpenRouterMessage {
            role: String,
            content: String,
        }

        #[derive(Deserialize)]
        struct OpenRouterResponse {
            choices: Vec<OpenRouterChoice>,
            usage: Option<OpenRouterUsage>,
        }

        #[derive(Deserialize)]
        struct OpenRouterChoice {
            message: OpenRouterResponseMessage,
        }

        #[derive(Deserialize)]
        struct OpenRouterResponseMessage {
            role: String,
            content: String,
        }

        #[derive(Deserialize)]
        struct OpenRouterUsage {
            total_tokens: i32,
        }

        let site_url = std::env::var("OPENROUTER_SITE_URL")
            .unwrap_or_else(|_| "https://arda.credit".to_string());
        let site_name =
            std::env::var("OPENROUTER_SITE_NAME").unwrap_or_else(|_| "Arda Credit".to_string());

        let mut messages = vec![];

        if let Some(system_prompt) = request.system_prompt {
            messages.push(OpenRouterMessage {
                role: "system".to_string(),
                content: system_prompt,
            });
        }

        messages.push(OpenRouterMessage {
            role: "user".to_string(),
            content: request.prompt,
        });

        let openrouter_request = OpenRouterRequest {
            model: model.to_string(),
            messages,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
        };

        let response = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("HTTP-Referer", site_url)
            .header("X-Title", site_name)
            .header("Content-Type", "application/json")
            .json(&openrouter_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("OpenRouter API error: {}", error_text).into());
        }

        let openrouter_response: OpenRouterResponse = response.json().await?;

        let content = openrouter_response
            .choices
            .first()
            .ok_or("No choices in response")?
            .message
            .content
            .clone();

        Ok(LlmResponse {
            content,
            usage_tokens: openrouter_response.usage.map(|u| u.total_tokens),
            model_used: model.to_string(),
            confidence_score: 0.85, // Default confidence for OpenRouter models
        })
    }

    async fn call_local_model(
        &self,
        request: LlmRequest,
    ) -> Result<LlmResponse, Box<dyn std::error::Error>> {
        #[derive(Serialize)]
        struct LocalRequest {
            prompt: String,
            max_tokens: Option<i32>,
            temperature: Option<f32>,
            system_prompt: Option<String>,
        }

        #[derive(Deserialize)]
        struct LocalResponse {
            response: String,
            tokens_used: Option<i32>,
        }

        let local_request = LocalRequest {
            prompt: request.prompt,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            system_prompt: request.system_prompt,
        };

        let response = self
            .client
            .post(&format!("{}/generate", self.base_url))
            .header("Content-Type", "application/json")
            .json(&local_request)
            .send()
            .await?;

        let local_response: LocalResponse = response.json().await?;

        Ok(LlmResponse {
            content: local_response.response,
            usage_tokens: local_response.tokens_used,
            model_used: "local-model".to_string(),
            confidence_score: 0.75, // Lower confidence for local models
        })
    }

    fn build_document_analysis_prompt(
        &self,
        prompt: &DocumentAnalysisPrompt,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut analysis_prompt = String::new();

        analysis_prompt.push_str(
            "Analyze the following document changes and provide a structured analysis:\n\n",
        );

        analysis_prompt.push_str("CURRENT DOCUMENT:\n");
        analysis_prompt.push_str(&prompt.current_content);
        analysis_prompt.push('\n');

        if let Some(previous) = &prompt.previous_content {
            analysis_prompt.push_str("PREVIOUS VERSION:\n");
            analysis_prompt.push_str(previous);
            analysis_prompt.push('\n');
        }

        analysis_prompt.push_str(&format!("DOCUMENT TYPE: {}\n", prompt.document_type));

        if !prompt.analysis_focus.is_empty() {
            analysis_prompt.push_str("FOCUS AREAS: ");
            analysis_prompt.push_str(&prompt.analysis_focus.join(", "));
            analysis_prompt.push('\n');
        }

        analysis_prompt.push_str("\nProvide analysis in JSON format with the following structure for each change identified:\n");
        analysis_prompt.push_str(
            r#"
{
  "changes": [
    {
      "change_type": "addition|deletion|modification|restructure",
      "impact_level": "minor|moderate|significant|critical",
      "category": "financial|legal|operational|compliance",
      "summary": "Brief description of the change",
      "implications": ["List of implications"],
      "recommendations": ["List of recommendations"],
      "related_clauses": ["Related document sections"],
      "risk_factors": [
        {
          "risk_type": "Type of risk",
          "severity": "minor|moderate|significant|critical",
          "description": "Risk description",
          "mitigation_suggestions": ["Mitigation strategies"]
        }
      ],
      "confidence_score": 0.85
    }
  ]
}
"#,
        );

        Ok(analysis_prompt)
    }

    fn build_chat_prompt(&self, prompt: &ChatPrompt) -> Result<String, Box<dyn std::error::Error>> {
        let mut chat_prompt = String::new();

        chat_prompt
            .push_str("User is asking about document analysis. Provide helpful insights.\n\n");

        if let Some(doc_context) = &prompt.document_context {
            chat_prompt.push_str("DOCUMENT CONTEXT:\n");
            chat_prompt.push_str(doc_context);
            chat_prompt.push('\n');
        }

        if let Some(analysis_context) = &prompt.analysis_context {
            chat_prompt.push_str("RECENT ANALYSIS:\n");
            chat_prompt.push_str(analysis_context);
            chat_prompt.push('\n');
        }

        if !prompt.conversation_history.is_empty() {
            chat_prompt.push_str("CONVERSATION HISTORY:\n");
            for msg in &prompt.conversation_history {
                chat_prompt.push_str(&format!("{}: {}\n", msg.role, msg.content));
            }
            chat_prompt.push('\n');
        }

        chat_prompt.push_str("USER MESSAGE:\n");
        chat_prompt.push_str(&prompt.user_message);

        Ok(chat_prompt)
    }

    fn build_insights_prompt(
        &self,
        content: &str,
        changes: &[ChangeAnalysis],
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut insights_prompt = String::new();

        insights_prompt.push_str(
            "Generate negotiation insights for the following document and recent changes:\n\n",
        );

        insights_prompt.push_str("DOCUMENT CONTENT:\n");
        insights_prompt.push_str(content);
        insights_prompt.push('\n');

        if !changes.is_empty() {
            insights_prompt.push_str("RECENT CHANGES:\n");
            for change in changes {
                let category_str = match change.category {
                    ChangeCategory::Financial => "Financial",
                    ChangeCategory::Legal => "Legal",
                    ChangeCategory::Operational => "Operational",
                    ChangeCategory::Compliance => "Compliance",
                };
                insights_prompt.push_str(&format!(
                    "- {}: {} (Impact: {:?})\n",
                    category_str, change.summary, change.impact_level
                ));
            }
            insights_prompt.push('\n');
        }

        insights_prompt.push_str("Provide 3-5 key negotiation insights focusing on:\n");
        insights_prompt.push_str("1. Market comparisons\n");
        insights_prompt.push_str("2. Risk assessments\n");
        insights_prompt.push_str("3. Negotiation recommendations\n");
        insights_prompt.push_str("4. Strategic considerations\n");

        Ok(insights_prompt)
    }

    fn parse_analysis_response(
        &self,
        response: &LlmResponse,
        prompt: &DocumentAnalysisPrompt,
    ) -> Result<Vec<ChangeAnalysis>, Box<dyn std::error::Error>> {
        // Try to parse JSON response
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response.content) {
            if let Some(changes_array) = parsed.get("changes").and_then(|c| c.as_array()) {
                let mut changes = Vec::new();

                for change_obj in changes_array {
                    if let Ok(change) = self.parse_single_change(change_obj, prompt) {
                        changes.push(change);
                    }
                }

                return Ok(changes);
            }
        }

        // Fallback: parse as structured text
        self.parse_text_analysis_response(&response.content, prompt)
    }

    fn parse_single_change(
        &self,
        change_obj: &serde_json::Value,
        _prompt: &DocumentAnalysisPrompt,
    ) -> Result<ChangeAnalysis, Box<dyn std::error::Error>> {
        let change_type = match change_obj.get("change_type").and_then(|c| c.as_str()) {
            Some("addition") => ChangeType::Addition,
            Some("deletion") => ChangeType::Deletion,
            Some("modification") => ChangeType::Modification,
            Some("restructure") => ChangeType::Restructure,
            _ => ChangeType::Modification,
        };

        let impact_level = match change_obj.get("impact_level").and_then(|i| i.as_str()) {
            Some("minor") => ImpactLevel::Minor,
            Some("moderate") => ImpactLevel::Moderate,
            Some("significant") => ImpactLevel::Significant,
            Some("critical") => ImpactLevel::Critical,
            _ => ImpactLevel::Minor,
        };

        let category = match change_obj.get("category").and_then(|c| c.as_str()) {
            Some("financial") => ChangeCategory::Financial,
            Some("legal") => ChangeCategory::Legal,
            Some("operational") => ChangeCategory::Operational,
            Some("compliance") => ChangeCategory::Compliance,
            _ => ChangeCategory::Operational,
        };

        let summary = change_obj
            .get("summary")
            .and_then(|s| s.as_str())
            .unwrap_or("Unknown change")
            .to_string();

        let implications = change_obj
            .get("implications")
            .and_then(|i| i.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let recommendations = change_obj
            .get("recommendations")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let related_clauses = change_obj
            .get("related_clauses")
            .and_then(|rc| rc.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let confidence_score = change_obj
            .get("confidence_score")
            .and_then(|cs| cs.as_f64())
            .unwrap_or(0.7);

        // Parse risk factors
        let risk_factors =
            if let Some(rf_array) = change_obj.get("risk_factors").and_then(|rf| rf.as_array()) {
                rf_array
                    .iter()
                    .filter_map(|rf_obj| {
                        Some(RiskFactor {
                            risk_type: rf_obj.get("risk_type")?.as_str()?.to_string(),
                            severity: match rf_obj.get("severity")?.as_str()? {
                                "minor" => ImpactLevel::Minor,
                                "moderate" => ImpactLevel::Moderate,
                                "significant" => ImpactLevel::Significant,
                                "critical" => ImpactLevel::Critical,
                                _ => ImpactLevel::Minor,
                            },
                            description: rf_obj.get("description")?.as_str()?.to_string(),
                            mitigation_suggestions: rf_obj
                                .get("mitigation_suggestions")?
                                .as_array()?
                                .iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect(),
                        })
                    })
                    .collect()
            } else {
                vec![]
            };

        Ok(ChangeAnalysis {
            id: Uuid::new_v4(),
            document_id: Uuid::new_v4(), // Will be set by caller
            change_type,
            impact_level,
            category,
            summary,
            implications,
            recommendations,
            related_clauses,
            risk_factors,
            confidence_score,
            created_at: chrono::Utc::now(),
        })
    }

    fn parse_text_analysis_response(
        &self,
        content: &str,
        _prompt: &DocumentAnalysisPrompt,
    ) -> Result<Vec<ChangeAnalysis>, Box<dyn std::error::Error>> {
        // Fallback parser for non-JSON responses
        // This would implement text parsing logic for structured responses
        let mut changes = Vec::new();

        // Simple example - in production would be more sophisticated
        if content.contains("interest rate") {
            changes.push(ChangeAnalysis {
                id: Uuid::new_v4(),
                document_id: Uuid::new_v4(),
                change_type: ChangeType::Modification,
                impact_level: ImpactLevel::Significant,
                category: ChangeCategory::Financial,
                summary: "Interest rate modification detected".to_string(),
                implications: vec!["Affects loan cost".to_string()],
                recommendations: vec!["Review financial impact".to_string()],
                related_clauses: vec!["Interest Rate Section".to_string()],
                risk_factors: vec![],
                confidence_score: 0.8,
                created_at: chrono::Utc::now(),
            });
        }

        Ok(changes)
    }

    fn parse_insights_response(
        &self,
        response: &LlmResponse,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let content = &response.content;

        // Split by numbered points or bullet points
        let insights: Vec<String> = content
            .split('\n')
            .filter(|line| {
                let trimmed = line.trim();
                trimmed.starts_with(&['1', '2', '3', '4', '5'][..])
                    || trimmed.starts_with('-')
                    || trimmed.starts_with('•')
            })
            .map(|line| line.trim().to_string())
            .filter(|line| line.len() > 10) // Filter out very short lines
            .collect();

        Ok(insights)
    }

    fn get_document_analysis_system_prompt(&self) -> String {
        r#"You are an expert legal and financial document analyzer. Your role is to:

1. Identify changes between document versions with high accuracy
2. Assess the business and legal impact of each change
3. Categorize changes by type (financial, legal, operational, compliance)
4. Provide actionable recommendations for document review
5. Identify potential risks and mitigation strategies

Focus on practical implications for business stakeholders. Be precise and actionable in your analysis.
Always respond with valid JSON in the requested format."#.to_string()
    }

    fn get_chat_system_prompt(&self) -> String {
        r#"You are an AI assistant specializing in document analysis and negotiation support.

Your capabilities include:
- Explaining document changes in plain language
- Providing risk assessments and recommendations
- Comparing terms to market standards
- Offering negotiation strategies
- Answering questions about legal and financial implications

Be helpful, accurate, and concise. If you're uncertain about something, acknowledge it.
Focus on practical advice that helps users make informed decisions."#
            .to_string()
    }

    fn get_insights_system_prompt(&self) -> String {
        r#"You are a negotiation strategy expert with deep knowledge of legal and financial documents.

Generate insights that help users:
- Understand market positioning of their document terms
- Identify negotiation opportunities and risks
- Develop counter-proposal strategies
- Recognize red flags and problematic clauses

Provide specific, actionable insights backed by reasoning. Focus on strategic value for the user."#.to_string()
    }

    /// Get fallback models based on analysis type for intelligent model selection
    fn get_fallback_models_for_type(&self, analysis_type: AnalysisType) -> Vec<String> {
        match analysis_type {
            AnalysisType::DocumentDiff => vec![
                "anthropic/claude-3.5-sonnet".to_string(),
                "openai/gpt-4o".to_string(),
                "openai/gpt-4o-mini".to_string(),
            ],
            AnalysisType::Chat => vec![
                "openai/gpt-4o".to_string(),
                "anthropic/claude-3.5-sonnet".to_string(),
                "openai/gpt-4o-mini".to_string(),
            ],
            AnalysisType::Insights => vec![
                "anthropic/claude-3.5-sonnet".to_string(),
                "openai/gpt-4o".to_string(),
                "perplexity/llama-3.1-sonar-large-128k-online".to_string(),
            ],
            AnalysisType::Summarization => vec![
                "openai/gpt-4o-mini".to_string(),
                "anthropic/claude-3-haiku".to_string(),
                "openai/gpt-3.5-turbo".to_string(),
            ],
            AnalysisType::RiskAssessment => vec![
                "anthropic/claude-3.5-sonnet".to_string(),
                "openai/gpt-4o".to_string(),
                "openai/gpt-4o-mini".to_string(),
            ],
            AnalysisType::Classification => vec![
                "openai/gpt-4o-mini".to_string(),
                "openai/gpt-3.5-turbo".to_string(),
                "anthropic/claude-3-haiku".to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_analysis_prompt_building() {
        let llm_service =
            LlmService::new(LlmProvider::Local("http://localhost:8080".to_string())).unwrap();

        let prompt = DocumentAnalysisPrompt {
            current_content: "Current loan agreement content".to_string(),
            previous_content: Some("Previous loan agreement content".to_string()),
            document_type: "loan_agreement".to_string(),
            analysis_focus: vec!["financial".to_string(), "legal".to_string()],
            context: HashMap::new(),
        };

        let analysis_prompt = llm_service.build_document_analysis_prompt(&prompt).unwrap();

        assert!(analysis_prompt.contains("CURRENT DOCUMENT:"));
        assert!(analysis_prompt.contains("PREVIOUS VERSION:"));
        assert!(analysis_prompt.contains("loan_agreement"));
        assert!(analysis_prompt.contains("financial, legal"));
    }

    #[test]
    fn test_change_type_parsing() {
        let change_data = serde_json::json!({
            "change_type": "addition",
            "impact_level": "critical",
            "category": "financial",
            "summary": "New guarantee clause added",
            "implications": ["Increases liability"],
            "recommendations": ["Review with counsel"],
            "related_clauses": ["Section 8"],
            "confidence_score": 0.92
        });

        let llm_service =
            LlmService::new(LlmProvider::Local("http://localhost:8080".to_string())).unwrap();
        let prompt = DocumentAnalysisPrompt {
            current_content: "test".to_string(),
            previous_content: None,
            document_type: "test".to_string(),
            analysis_focus: vec![],
            context: HashMap::new(),
        };

        let change = llm_service
            .parse_single_change(&change_data, &prompt)
            .unwrap();

        assert_eq!(change.change_type, ChangeType::Addition);
        assert_eq!(change.impact_level, ImpactLevel::Critical);
        assert_eq!(change.category, ChangeCategory::Financial);
        assert_eq!(change.summary, "New guarantee clause added");
        assert_eq!(change.confidence_score, 0.92);
    }
}
