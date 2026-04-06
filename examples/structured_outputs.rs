use openai_rust::resources::responses::{
    FunctionTool, ResponseFormatTextConfig, ResponseFormatTextJSONSchemaConfig,
    ResponseParseParams, ResponseTextConfig,
};
use serde_json::json;

fn main() {
    let params = ResponseParseParams {
        model: "gpt-4.1-mini".into(),
        input: Some(json!("Return {\"language\":\"rust\"}")),
        text: Some(ResponseTextConfig {
            format: Some(ResponseFormatTextConfig::JsonSchema(
                ResponseFormatTextJSONSchemaConfig {
                    name: "language".into(),
                    schema: json!({
                        "type": "object",
                        "properties": { "language": { "type": "string" } },
                        "required": ["language"],
                        "additionalProperties": false
                    }),
                    description: Some("Structured language output".into()),
                    strict: Some(true),
                },
            )),
            verbosity: None,
        }),
        tools: vec![FunctionTool {
            name: "lookup_language".into(),
            parameters: json!({
                "type": "object",
                "properties": { "language": { "type": "string" } },
                "required": ["language"],
                "additionalProperties": false
            }),
            strict: Some(true),
            description: Some("Example strict tool".into()),
            defer_loading: None,
        }],
        ..Default::default()
    };

    let schema_name = params
        .text
        .as_ref()
        .and_then(|text| text.format.as_ref())
        .map(|_| "language");
    println!("Structured output schema: {:?}", schema_name);
    println!("Strict tools configured: {}", params.tools.len());
}
