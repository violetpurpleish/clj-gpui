//! Asynchronous editor providers. The UI thread only queues JSON requests;
//! Clojure returns standard LSP objects through an independent RPC reply.
use super::*;
use gpui::Task;
use gpui_kit::base::input::{
    CodeActionProvider, CompletionProvider, DefinitionProvider, DocumentColorProvider,
    DocumentRangeSemanticTokensProvider, HoverProvider, Lsp, Rope,
};
use serde::de::DeserializeOwned;
use std::time::Duration;

struct Provider {
    config: Value,
    tx: mpsc::Sender<Cmd>,
}
impl Provider {
    fn has(&self, method: &str) -> bool {
        self.config.get(method).and_then(Value::as_str).is_some()
    }
    fn request<T: DeserializeOwned + 'static>(
        &self,
        method: &str,
        params: Value,
        fallback: Value,
        cx: &App,
    ) -> Task<anyhow::Result<T>> {
        let Some(id) = self.config.get(method).and_then(Value::as_str) else {
            return Task::ready(serde_json::from_value(fallback).map_err(Into::into));
        };
        let (response, receiver) = async_channel::bounded(1);
        if self
            .tx
            .send(Cmd::Provider {
                id: id.to_string(),
                params,
                response,
            })
            .is_err()
        {
            return Task::ready(Err(anyhow::anyhow!("Clojure provider connection closed")));
        }
        cx.spawn(async move |_| {
            let value = receiver.recv().await?.map_err(anyhow::Error::msg)?;
            serde_json::from_value(value).map_err(Into::into)
        })
    }
}
impl CompletionProvider for Provider {
    fn completions(
        &self,
        text: &Rope,
        offset: usize,
        trigger: lsp_types::CompletionContext,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<lsp_types::CompletionResponse>> {
        self.request(
            "completions",
            json!({"text":text.to_string(),"offset":offset,"trigger":trigger}),
            json!([]),
            cx,
        )
    }
    fn inline_completion(
        &self,
        text: &Rope,
        offset: usize,
        trigger: lsp_types::InlineCompletionContext,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<lsp_types::InlineCompletionResponse>> {
        self.request(
            "inline-completions",
            json!({"text":text.to_string(),"offset":offset,"trigger":trigger}),
            json!([]),
            cx,
        )
    }
    fn inline_completion_debounce(&self) -> Duration {
        Duration::from_millis(
            self.config
                .get("inline-debounce-ms")
                .and_then(Value::as_u64)
                .unwrap_or(300),
        )
    }
    fn is_completion_trigger(&self, _: usize, new_text: &str, _: &mut App) -> bool {
        new_text.chars().last().is_some_and(|ch| {
            ch.is_alphanumeric()
                || ch == '_'
                || self
                    .config
                    .get("trigger-characters")
                    .and_then(Value::as_array)
                    .is_some_and(|chars| {
                        chars
                            .iter()
                            .any(|c| c.as_str() == Some(ch.to_string().as_str()))
                    })
        })
    }
}
impl HoverProvider for Provider {
    fn hover(
        &self,
        text: &Rope,
        offset: usize,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Option<lsp_types::Hover>>> {
        self.request(
            "hover",
            json!({"text":text.to_string(),"offset":offset}),
            Value::Null,
            cx,
        )
    }
}
impl DefinitionProvider for Provider {
    fn definitions(
        &self,
        text: &Rope,
        offset: usize,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Vec<lsp_types::LocationLink>>> {
        self.request(
            "definitions",
            json!({"text":text.to_string(),"offset":offset}),
            json!([]),
            cx,
        )
    }
}
impl DocumentColorProvider for Provider {
    fn document_colors(
        &self,
        text: &Rope,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Vec<lsp_types::ColorInformation>>> {
        self.request(
            "document-colors",
            json!({"text":text.to_string()}),
            json!([]),
            cx,
        )
    }
}
impl DocumentRangeSemanticTokensProvider for Provider {
    fn legend(&self) -> lsp_types::SemanticTokensLegend {
        serde_json::from_value(
            self.config
                .get("legend")
                .cloned()
                .unwrap_or(json!({"tokenTypes":[],"tokenModifiers":[]})),
        )
        .unwrap_or_default()
    }
    fn semantic_tokens(
        &self,
        text: &Rope,
        range: std::ops::Range<usize>,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<lsp_types::SemanticTokens>> {
        self.request(
            "semantic-tokens",
            json!({"text":text.to_string(),"range":{"start":range.start,"end":range.end}}),
            json!({"data":[]}),
            cx,
        )
    }
}
impl CodeActionProvider for Provider {
    fn id(&self) -> SharedString {
        "clojure".into()
    }
    fn code_actions(
        &self,
        state: Entity<EditorState>,
        range: std::ops::Range<usize>,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Vec<lsp_types::CodeAction>>> {
        self.request("code-actions", json!({"text":state.read(cx).value().to_string(),"range":{"start":range.start,"end":range.end}}), json!([]), cx)
    }
    fn perform_code_action(
        &self,
        state: Entity<EditorState>,
        action: lsp_types::CodeAction,
        push_to_history: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<()>> {
        let original = state.read(cx).value().to_string();
        let request = self.request::<Value>(
            "perform-code-action",
            json!({"text":original,"action":action,"push-to-history":push_to_history}),
            Value::Null,
            cx,
        );
        window.spawn(cx, async move |cx| {
            let result = request.await?;
            if let Some(edits) = result.get("edits") {
                let mut edits: Vec<lsp_types::TextEdit> = serde_json::from_value(edits.clone())?;
                // Every edit addresses the same original document.
                edits.sort_by_key(|edit| {
                    std::cmp::Reverse((edit.range.start.line, edit.range.start.character))
                });
                state.update_in(cx, |state, window, cx| {
                    anyhow::ensure!(
                        state.value().as_ref() == original,
                        "document changed while resolving code action"
                    );
                    state.apply_lsp_edits(&edits, window, cx);
                    cx.emit(InputEvent::Change);
                    Ok::<_, anyhow::Error>(())
                })??;
            }
            Ok(())
        })
    }
}

impl RootView {
    pub(super) fn sync_editor_providers(
        &mut self,
        key: &str,
        node: &Node,
        state: &Entity<EditorState>,
        cx: &mut Context<Self>,
    ) {
        let config = node.lsp.clone().unwrap_or(Value::Null);
        if self.parity.providers.get(key) == Some(&config)
            || (config.is_null() && !self.parity.providers.contains_key(key))
        {
            return;
        }
        let provider = Rc::new(Provider {
            config: config.clone(),
            tx: self.cmd_tx.clone(),
        });
        let mut lsp = Lsp::default();
        if provider.has("completions") || provider.has("inline-completions") {
            lsp.completion_provider = Some(provider.clone());
        }
        if provider.has("hover") {
            lsp.hover_provider = Some(provider.clone());
        }
        if provider.has("definitions") {
            lsp.definition_provider = Some(provider.clone());
        }
        if provider.has("document-colors") {
            lsp.document_color_provider = Some(provider.clone());
        }
        if provider.has("semantic-tokens") {
            lsp.semantic_tokens_provider = Some(provider.clone());
        }
        if provider.has("code-actions") {
            lsp.code_action_providers.push(provider.clone());
        }
        if let Some(width) = config
            .get("completion-menu-width")
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite() && *n > 0.)
        {
            lsp.completion_menu.max_width = px(width as f32);
        }
        if provider.has("show-document") {
            lsp.show_document = Some(Rc::new(move |params, _, cx| {
                provider
                    .request::<Value>("show-document", json!(params), Value::Null, cx)
                    .detach();
                provider
                    .config
                    .get("show-document-policy")
                    .and_then(Value::as_str)
                    == Some("consume")
            }));
        }
        state.update(cx, |state, cx| {
            *state.lsp_mut() = lsp;
            state.refresh(cx);
        });
        self.parity.providers.insert(key.to_string(), config);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui_kit::test]
    async fn provider_requests_decode_lsp_results_and_report_remote_errors(
        cx: &mut gpui::TestAppContext,
    ) {
        let (tx, rx) = mpsc::channel();
        let provider = Provider {
            config: json!({"completions":"complete","hover":"hover"}),
            tx,
        };
        let task = cx.update(|cx| {
            provider.request::<lsp_types::CompletionResponse>(
                "completions",
                json!({"text":"λx","offset":2}),
                json!([]),
                cx,
            )
        });
        let Cmd::Provider {
            id,
            params,
            response,
        } = rx.recv().unwrap()
        else {
            panic!("provider request")
        };
        assert_eq!(id, "complete");
        assert_eq!(params["offset"], 2);
        response
            .send(Ok(json!([{"label":"println","insertText":"println!()"}])))
            .await
            .unwrap();
        let lsp_types::CompletionResponse::Array(items) = task.await.unwrap() else {
            panic!("completion items")
        };
        assert_eq!(items[0].insert_text.as_deref(), Some("println!()"));
        let task = cx.update(|cx| {
            provider.request::<Option<lsp_types::Hover>>("hover", Value::Null, Value::Null, cx)
        });
        let Cmd::Provider { response, .. } = rx.recv().unwrap() else {
            panic!("provider request")
        };
        response.send(Err("provider failed".into())).await.unwrap();
        assert!(
            task.await
                .unwrap_err()
                .to_string()
                .contains("provider failed")
        );
        let missing = cx.update(|cx| {
            provider.request::<Vec<lsp_types::LocationLink>>(
                "definitions",
                Value::Null,
                json!([]),
                cx,
            )
        });
        assert!(missing.await.unwrap().is_empty());
        assert!(rx.try_recv().is_err());
    }
}
