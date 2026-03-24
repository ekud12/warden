// ─── pipeline — composable middleware for hook processing ────────────────────
//
// Every hook invocation flows through a Pipeline of Middleware stages.
// Each stage is independent, toggleable, and panic-isolated.
// One bad stage cannot crash the pipeline — errors are caught and logged.
//
// Design:
//   - Stages run in order until a Deny/Allow short-circuits
//   - Each stage gets a mutable PipelineContext with shared state
//   - Panics in a stage are caught, logged, and skipped (fail-open)
//   - Per-stage timing is recorded for performance profiling
// ──────────────────────────────────────────────────────────────────────────────

pub mod context;

use context::PipelineContext;
use std::panic::AssertUnwindSafe;
use std::time::Instant;

/// Result of a single middleware stage
pub enum StageResult {
    /// Continue to next stage
    Continue,
    /// Short-circuit: deny the tool call with this message
    Deny(String),
    /// Short-circuit: allow, optionally with advisory message
    Allow(Option<String>),
    /// This stage doesn't apply (skipped, no effect)
    Skip,
}

/// A single stage in the hook processing pipeline
pub trait Middleware: Send + Sync {
    /// Unique name for logging and profiling
    fn name(&self) -> &'static str;

    /// Whether this stage is active given current config.
    /// Default: always enabled. Override to check config.tools.*, feature flags, etc.
    fn enabled(&self, ctx: &PipelineContext) -> bool {
        let _ = ctx;
        true
    }

    /// Process the hook event. Return StageResult to control pipeline flow.
    fn process(&self, ctx: &mut PipelineContext) -> StageResult;
}

/// The pipeline executor — runs stages in order with panic isolation
pub struct Pipeline {
    stages: Vec<Box<dyn Middleware>>,
}

impl Pipeline {
    pub fn new(stages: Vec<Box<dyn Middleware>>) -> Self {
        Self { stages }
    }

    /// Execute all stages in order. Populates ctx.decision on deny/allow.
    /// Returns the final decision (None = passthrough/no opinion).
    pub fn execute(&self, ctx: &mut PipelineContext) {
        for stage in &self.stages {
            // Skip disabled stages
            if !stage.enabled(ctx) {
                continue;
            }

            let stage_name = stage.name();
            let start = Instant::now();

            // Panic isolation: catch_unwind per stage
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                stage.process(ctx)
            }));

            let elapsed = start.elapsed();
            ctx.timings.push((stage_name, elapsed));

            match result {
                Ok(StageResult::Deny(msg)) => {
                    ctx.decision = Some(Decision::Deny(msg));
                    return;
                }
                Ok(StageResult::Allow(advisory)) => {
                    ctx.decision = Some(Decision::Allow(advisory));
                    return;
                }
                Ok(StageResult::Continue) => {}
                Ok(StageResult::Skip) => {}
                Err(_) => {
                    // Stage panicked — log and continue (fail-open)
                    ctx.log_error(&format!("stage '{}' panicked — skipped", stage_name));
                }
            }
        }
    }

    /// Number of stages in this pipeline
    pub fn len(&self) -> usize {
        self.stages.len()
    }

    /// Whether this pipeline has no stages
    pub fn is_empty(&self) -> bool {
        self.stages.is_empty()
    }
}

/// Final decision after pipeline execution
#[derive(Debug, Clone)]
pub enum Decision {
    Deny(String),
    Allow(Option<String>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use context::PipelineContext;

    struct AlwaysContinue;
    impl Middleware for AlwaysContinue {
        fn name(&self) -> &'static str { "always-continue" }
        fn process(&self, _ctx: &mut PipelineContext) -> StageResult { StageResult::Continue }
    }

    struct AlwaysDeny;
    impl Middleware for AlwaysDeny {
        fn name(&self) -> &'static str { "always-deny" }
        fn process(&self, _ctx: &mut PipelineContext) -> StageResult {
            StageResult::Deny("blocked".to_string())
        }
    }

    struct PanicStage;
    impl Middleware for PanicStage {
        fn name(&self) -> &'static str { "panic-stage" }
        fn process(&self, _ctx: &mut PipelineContext) -> StageResult { panic!("boom") }
    }

    struct DisabledStage;
    impl Middleware for DisabledStage {
        fn name(&self) -> &'static str { "disabled" }
        fn enabled(&self, _ctx: &PipelineContext) -> bool { false }
        fn process(&self, _ctx: &mut PipelineContext) -> StageResult {
            StageResult::Deny("should not reach".to_string())
        }
    }

    #[test]
    fn pipeline_continues_through_stages() {
        let pipeline = Pipeline::new(vec![
            Box::new(AlwaysContinue),
            Box::new(AlwaysContinue),
        ]);
        let mut ctx = PipelineContext::test_default();
        pipeline.execute(&mut ctx);
        assert!(ctx.decision.is_none(), "no decision = passthrough");
        assert_eq!(ctx.timings.len(), 2, "both stages timed");
    }

    #[test]
    fn pipeline_short_circuits_on_deny() {
        let pipeline = Pipeline::new(vec![
            Box::new(AlwaysContinue),
            Box::new(AlwaysDeny),
            Box::new(AlwaysContinue), // should not run
        ]);
        let mut ctx = PipelineContext::test_default();
        pipeline.execute(&mut ctx);
        assert!(matches!(ctx.decision, Some(Decision::Deny(_))));
        assert_eq!(ctx.timings.len(), 2, "third stage skipped");
    }

    #[test]
    fn pipeline_survives_panic() {
        let pipeline = Pipeline::new(vec![
            Box::new(PanicStage),
            Box::new(AlwaysContinue),
        ]);
        let mut ctx = PipelineContext::test_default();
        pipeline.execute(&mut ctx);
        // Pipeline should survive the panic and run the second stage
        assert!(ctx.decision.is_none());
        assert_eq!(ctx.timings.len(), 2);
    }

    #[test]
    fn pipeline_skips_disabled_stages() {
        let pipeline = Pipeline::new(vec![
            Box::new(DisabledStage),
            Box::new(AlwaysContinue),
        ]);
        let mut ctx = PipelineContext::test_default();
        pipeline.execute(&mut ctx);
        assert!(ctx.decision.is_none(), "disabled deny should not fire");
        assert_eq!(ctx.timings.len(), 1, "only enabled stage timed");
    }
}
