//! Lightweight callable + pipeline primitives.
//!
//! These mirror the shape of `atomr_agents::Callable` / `Pipeline`
//! but do not depend on the agents crate, so an extract pipeline
//! can be composed without pulling in the full agent runtime. An
//! adapter in the umbrella facade lifts these into
//! `atomr_agents::Callable` when the `agents` feature is enabled.

use async_trait::async_trait;
use std::sync::Arc;

use crate::backend::BackendError;

/// A typed async unit of extraction work.
#[async_trait]
pub trait Callable<I, O>: Send + Sync {
    /// Run the callable.
    async fn call(&self, input: I) -> Result<O, BackendError>;
}

/// Convenience wrapper turning a closure into a [`Callable`].
pub struct Fn1<F>(pub F);

#[async_trait]
impl<I, O, F, Fut> Callable<I, O> for Fn1<F>
where
    I: Send + 'static,
    O: Send + 'static,
    F: Fn(I) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<O, BackendError>> + Send,
{
    async fn call(&self, input: I) -> Result<O, BackendError> {
        (self.0)(input).await
    }
}

/// Two-stage chain: `Pipeline { first: A→B, second: B→C }`.
pub struct Pipeline<A, B, C> {
    first: Arc<dyn Callable<A, B>>,
    second: Arc<dyn Callable<B, C>>,
}

impl<A, B, C> Pipeline<A, B, C>
where
    A: Send + 'static,
    B: Send + 'static,
    C: Send + 'static,
{
    /// Compose two callables.
    pub fn new(first: Arc<dyn Callable<A, B>>, second: Arc<dyn Callable<B, C>>) -> Self {
        Self { first, second }
    }

    /// Append another stage, producing a new pipeline.
    pub fn then<D>(self, next: Arc<dyn Callable<C, D>>) -> Pipeline<A, C, D>
    where
        D: Send + 'static,
    {
        Pipeline::new(Arc::new(PipelineAB { first: self.first, second: self.second }), next)
    }
}

#[async_trait]
impl<A, B, C> Callable<A, C> for Pipeline<A, B, C>
where
    A: Send + 'static,
    B: Send + 'static,
    C: Send + 'static,
{
    async fn call(&self, input: A) -> Result<C, BackendError> {
        let mid = self.first.call(input).await?;
        self.second.call(mid).await
    }
}

struct PipelineAB<A, B, C> {
    first: Arc<dyn Callable<A, B>>,
    second: Arc<dyn Callable<B, C>>,
}

#[async_trait]
impl<A, B, C> Callable<A, C> for PipelineAB<A, B, C>
where
    A: Send + 'static,
    B: Send + 'static,
    C: Send + 'static,
{
    async fn call(&self, input: A) -> Result<C, BackendError> {
        let mid = self.first.call(input).await?;
        self.second.call(mid).await
    }
}

/// Enumeration of the seven default pipeline stages, used by the
/// auto-extract driver to emit phase-labeled activities.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ExtractStage {
    /// Stage 1 — load corpus.
    Ingest,
    /// Stage 2 — surface-term extraction.
    Terms,
    /// Stage 3 — entity resolution.
    Entities,
    /// Stage 4 — concept formation.
    Concepts,
    /// Stage 5 — taxonomy induction.
    Taxonomy,
    /// Stage 6 — relation extraction.
    Relations,
    /// Stage 7 — validate and commit.
    Commit,
}

impl ExtractStage {
    /// Stable string label for tracing.
    pub fn label(&self) -> &'static str {
        match self {
            ExtractStage::Ingest => "ingest",
            ExtractStage::Terms => "terms",
            ExtractStage::Entities => "entities",
            ExtractStage::Concepts => "concepts",
            ExtractStage::Taxonomy => "taxonomy",
            ExtractStage::Relations => "relations",
            ExtractStage::Commit => "commit",
        }
    }
}
