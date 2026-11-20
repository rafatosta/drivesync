//! Composition boundary for the three independent Analyzer stages.
use super::*;

/// The facade owns only the three stage ports. It has no Inventory, Operator,
/// Detector, queue, or policy dependency. Policy belongs to Decision.
pub struct AnalyzerEngine<C, D, A> {
    context: C,
    decision: D,
    apply: A,
}

impl<C: ContextService, D: DecisionService, A: ApplyService> AnalyzerEngine<C, D, A> {
    pub fn new(context: C, decision: D, apply: A) -> Self {
        Self { context, decision, apply }
    }
}

impl<C: ContextService, D: DecisionService, A: ApplyService> Analyzer for AnalyzerEngine<C, D, A> {
    fn analyze<'a>(&'a self, file: File, feedback: &'a dyn AnalysisFeedback) -> AnalysisFuture<'a> {
        Box::pin(async move {
            let _ = (&self.context, &self.decision, &self.apply, file, feedback);
            todo!("implement Analyzer composition after its behavioral contract is approved")
        })
    }
}
