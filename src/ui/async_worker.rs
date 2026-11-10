use std::sync::Arc;
use std::thread;

use crate::ai::gemini::{query_gemini_or_ast_assistant, AiResponse};
use crate::ai::provider::{AiError, AiProvider, AiQueryRequest, AiQueryResponse};
use crate::core::pipeline::{
    BatchProjectDeobfReport, DeobfuscationConfig, DeobfuscationResult, DeobfuscatorEngine,
};

/// Asynchronous worker command types.
#[derive(Debug, Clone)]
pub enum WorkerCommand {
    RunDeobfuscation {
        code: String,
        config: DeobfuscationConfig,
    },
    QueryAi {
        api_key: String,
        model: String,
        prompt: String,
        filename: String,
        code_context: String,
        config: DeobfuscationConfig,
    },
    DispatchAiProvider {
        provider_name: String,
        request: AiQueryRequest,
    },
}

/// Spawns a background thread to execute long-running tasks.
pub fn spawn_worker_task<F>(task: F)
where
    F: FnOnce() + Send + 'static,
{
    thread::spawn(task);
}

/// Dispatches an AI query to a background thread using Gemini AI
/// and posts the response back to Slint's event loop without freezing the GUI.
pub fn execute_ai_query_async<F>(
    api_key: String,
    model: String,
    prompt: String,
    filename: String,
    code_context: String,
    config: DeobfuscationConfig,
    on_complete: F,
) where
    F: FnOnce(AiResponse) + Send + 'static,
{
    thread::spawn(move || {
        let response = query_gemini_or_ast_assistant(
            &api_key,
            &model,
            &prompt,
            &filename,
            &code_context,
            &config,
        );
        let _ = slint::invoke_from_event_loop(move || {
            on_complete(response);
        });
    });
}

/// Dispatches an arbitrary AiProvider query on a background thread and posts the structured
/// AiQueryResponse back to Slint's event loop.
pub fn dispatch_ai_provider_query_async<F>(
    provider: Arc<dyn AiProvider>,
    request: AiQueryRequest,
    on_complete: F,
) where
    F: FnOnce(Result<AiQueryResponse, AiError>) + Send + 'static,
{
    thread::spawn(move || {
        let result = provider.query(&request);
        let _ = slint::invoke_from_event_loop(move || {
            on_complete(result);
        });
    });
}

/// Dispatches a deobfuscation pipeline pass on a background thread.
pub fn execute_pipeline_async<F>(
    code: String,
    config: DeobfuscationConfig,
    on_complete: F,
) where
    F: FnOnce(DeobfuscationResult) + Send + 'static,
{
    thread::spawn(move || {
        let result = DeobfuscatorEngine::process(&code, &config);
        let _ = slint::invoke_from_event_loop(move || {
            on_complete(result);
        });
    });
}

/// Dispatches the 7-phase intelligent project analysis pipeline on a background thread.
/// Updates Slint via thread-safe progress callbacks without locking the GUI.
pub fn execute_project_analysis_async<P, C>(
    project_root: std::path::PathBuf,
    on_progress: P,
    on_complete: C,
) where
    P: Fn(crate::analysis::AnalysisProgressUpdate) + Send + Sync + 'static,
    C: FnOnce(crate::analysis::ProjectAnalysisReport) + Send + 'static,
{
    thread::spawn(move || {
        let progress_arc = Arc::new(on_progress);
        let cb_clone = Arc::clone(&progress_arc);
        let progress_cb = move |update: crate::analysis::AnalysisProgressUpdate| {
            let inner = Arc::clone(&cb_clone);
            let _ = slint::invoke_from_event_loop(move || {
                inner(update);
            });
        };

        let report = crate::analysis::ProjectAnalysisPipeline::run(
            &project_root,
            progress_cb,
        );

        let _ = slint::invoke_from_event_loop(move || {
            on_complete(report);
        });
    });
}

/// Dispatches full project-wide batch deobfuscation on a background thread.
/// Folds Allatori constants, decodes obfuscated names, purges bytecode comments,
/// and writes clean files to `<project>/desofuscado/`.
pub fn execute_batch_project_deobfuscate_async<C>(
    project_root: std::path::PathBuf,
    on_complete: C,
) where
    C: FnOnce(std::io::Result<BatchProjectDeobfReport>) + Send + 'static,
{
    thread::spawn(move || {
        let result = DeobfuscatorEngine::deobfuscate_project(&project_root);
        let _ = slint::invoke_from_event_loop(move || {
            on_complete(result);
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::provider::AiAction;
    use std::sync::mpsc;

    struct DummyProvider;

    impl AiProvider for DummyProvider {
        fn name(&self) -> &str {
            "DummyProvider"
        }

        fn query(&self, req: &AiQueryRequest) -> Result<AiQueryResponse, AiError> {
            Ok(AiQueryResponse {
                reply_text: format!("Answer to {}", req.prompt),
                suggested_code: Some("print('dummy')".to_string()),
                mode_badge: "Dummy".to_string(),
                model_used: req.model.clone(),
                token_count: Some(10),
            })
        }
    }

    #[test]
    fn test_worker_thread_isolation() {
        let provider = Arc::new(DummyProvider);
        let (tx, rx) = mpsc::channel();

        let req = AiQueryRequest {
            prompt: "Test query".into(),
            model: "dummy-model".into(),
            context_code: "".into(),
            action: AiAction::CustomQuery,
            api_key: "KEY".into(),
            filename: "test.py".into(),
        };

        thread::spawn(move || {
            let res = provider.query(&req);
            tx.send(res).unwrap();
        });

        let received = rx.recv().expect("Thread should finish");
        assert!(received.is_ok());
        let res = received.unwrap();
        assert_eq!(res.reply_text, "Answer to Test query");
    }
}
