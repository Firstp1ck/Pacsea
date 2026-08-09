//! Deterministic logical-scan attempt, correction, fallback, and cancellation control.
//!
//! This module deliberately separates policy from pipe I/O. The runtime drives the
//! returned actions through the correlated RPC codec, but it cannot accidentally issue
//! a correction or model fallback after cancellation because the controller suppresses
//! every later transition once cancelled.

use std::fmt;

use super::{has_forbidden_control, limits};

/// What: One user-confirmed provider/model choice in fallback order.
///
/// Inputs: Constructed from confirmed scanner setup state.
///
/// Output: Consumed by [`ScanAttemptController`].
///
/// Details:
/// - Values are passed as RPC fields, never converted into slash commands or shell text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelChoice {
    /// Exact provider identifier.
    pub provider: String,
    /// Exact model identifier.
    pub model: String,
}

/// What: A single policy action the RPC runtime must execute.
///
/// Inputs: Returned by controller transitions.
///
/// Output: Converted to a correlated Pi RPC command by the runtime.
///
/// Details:
/// - `SelectModel` must settle successfully before [`ScanAttemptController::model_selected`]
///   is called and a fresh full prompt is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionAction {
    /// Send the original full scan prompt for the current model.
    SendFullPrompt {
        /// Zero-based index in the confirmed model order.
        model_index: usize,
    },
    /// Send the one bounded correction for the current model.
    SendCorrection {
        /// Zero-based index in the confirmed model order.
        model_index: usize,
    },
    /// Select the next confirmed fallback with in-session `set_model`.
    SelectModel {
        /// Zero-based index in the confirmed model order.
        model_index: usize,
        /// Exact provider identifier.
        provider: String,
        /// Exact model identifier.
        model: String,
    },
    /// Stop Pi's internal provider retry loop with `abort_retry`.
    AbortRetry,
    /// Abort the active Pi request with `abort`.
    Abort,
}

/// What: Outcome of handling an attempt failure.
///
/// Inputs: Produced by validation/provider failure transitions.
///
/// Output: An action to execute, terminal exhaustion, or cancellation suppression.
///
/// Details:
/// - `Suppressed` is sticky after cancellation and never carries a prompt/fallback action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureDecision {
    /// Execute the contained policy action.
    Action(SessionAction),
    /// No confirmed model/correction remains.
    Exhausted,
    /// Cancellation suppressed correction and fallback.
    Suppressed,
}

/// What: Invalid controller construction or out-of-order transition.
///
/// Inputs: Produced by [`ScanAttemptController`].
///
/// Output: Implements `Display`/`Error` for fail-closed runtime handling.
///
/// Details:
/// - Transition errors are protocol bugs and must terminate the logical scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// No primary model was configured.
    NoModels,
    /// The confirmed model list exceeds the compiled attempt maximum.
    TooManyModels {
        /// Observed model count.
        observed: usize,
        /// Compiled maximum.
        limit: usize,
    },
    /// A model/provider identifier is empty or control-bearing.
    InvalidModelChoice {
        /// Zero-based offending entry.
        index: usize,
    },
    /// The requested transition does not match the current state.
    InvalidTransition {
        /// Short static operation label.
        operation: &'static str,
    },
}

impl fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoModels => write!(formatter, "at least one confirmed Pi model is required"),
            Self::TooManyModels { observed, limit } => write!(
                formatter,
                "{observed} Pi models exceed the {limit}-attempt logical-scan limit"
            ),
            Self::InvalidModelChoice { index } => write!(
                formatter,
                "confirmed Pi model entry {index} has an empty or control-bearing identifier"
            ),
            Self::InvalidTransition { operation } => write!(
                formatter,
                "Pi scan session received out-of-order transition {operation:?}"
            ),
        }
    }
}

impl std::error::Error for SessionError {}

/// Internal phase of one logical scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// No prompt has been sent.
    Ready,
    /// Waiting for validation of a full answer.
    FullAnswer,
    /// Waiting for validation of the one corrected answer.
    CorrectedAnswer,
    /// Waiting for `set_model` correlation to settle.
    SelectingModel,
    /// A validated answer completed the logical scan.
    Completed,
    /// No eligible correction or fallback remains.
    Exhausted,
    /// User cancellation is sticky and terminal.
    Cancelled,
}

/// What: Fail-closed policy controller for one logical Pi scan.
///
/// Inputs: A confirmed ordered primary/fallback model list.
///
/// Output: Ordered prompt, correction, `set_model`, and cancellation actions.
///
/// Details:
/// - At most three models are accepted, each gets one full answer and at most one
///   correction, and fallback always requires a settled in-session `set_model` first.
/// - Cancellation is terminal: every later failure transition returns `Suppressed`.
#[derive(Debug, Clone)]
pub struct ScanAttemptController {
    /// Confirmed model order.
    models: Vec<ModelChoice>,
    /// Current model index.
    model_index: usize,
    /// Current policy phase.
    phase: Phase,
}

impl ScanAttemptController {
    /// What: Validate and create a logical-scan policy controller.
    ///
    /// Inputs:
    /// - `models`: Confirmed primary followed by eligible fallbacks.
    ///
    /// Output:
    /// - A ready controller.
    ///
    /// Details:
    /// - The list is bounded by [`limits::MAX_MODEL_ATTEMPTS`]. Identifiers remain exact.
    ///
    /// # Errors
    /// - Returns `Err` for an empty/oversized list or invalid identifier.
    pub fn new(models: Vec<ModelChoice>) -> Result<Self, SessionError> {
        if models.is_empty() {
            return Err(SessionError::NoModels);
        }
        let maximum = limits::MAX_MODEL_ATTEMPTS as usize;
        if models.len() > maximum {
            return Err(SessionError::TooManyModels {
                observed: models.len(),
                limit: maximum,
            });
        }
        if let Some(index) = models.iter().position(|choice| {
            choice.provider.is_empty()
                || choice.model.is_empty()
                || has_forbidden_control(&choice.provider)
                || has_forbidden_control(&choice.model)
        }) {
            return Err(SessionError::InvalidModelChoice { index });
        }
        Ok(Self {
            models,
            model_index: 0,
            phase: Phase::Ready,
        })
    }

    /// What: Begin the primary model's fresh full validation pass.
    ///
    /// Inputs: None.
    ///
    /// Output: A full-prompt action for model zero.
    ///
    /// Details:
    /// - May be called exactly once.
    ///
    /// # Errors
    /// - Returns `Err` when the controller is not ready.
    pub fn begin(&mut self) -> Result<SessionAction, SessionError> {
        if self.phase != Phase::Ready {
            return Err(SessionError::InvalidTransition { operation: "begin" });
        }
        self.phase = Phase::FullAnswer;
        Ok(SessionAction::SendFullPrompt { model_index: 0 })
    }

    /// What: Handle a strict output/schema/evidence validation failure.
    ///
    /// Inputs: None; validation detail is kept outside this policy state.
    ///
    /// Output: One correction, a fallback selection, exhaustion, or suppression.
    ///
    /// Details:
    /// - A full-answer failure gets exactly one correction. A corrected-answer failure
    ///   selects the next model in-session or exhausts the confirmed list.
    ///
    /// # Errors
    /// - Returns `Err` when no model answer is awaiting validation.
    pub fn validation_failed(&mut self) -> Result<FailureDecision, SessionError> {
        if self.phase == Phase::Cancelled {
            return Ok(FailureDecision::Suppressed);
        }
        match self.phase {
            Phase::FullAnswer => {
                self.phase = Phase::CorrectedAnswer;
                Ok(FailureDecision::Action(SessionAction::SendCorrection {
                    model_index: self.model_index,
                }))
            }
            Phase::CorrectedAnswer => Ok(self.select_fallback()),
            _ => Err(SessionError::InvalidTransition {
                operation: "validation_failed",
            }),
        }
    }

    /// What: Handle an eligible provider/model failure without spending a correction.
    ///
    /// Inputs: None; the runtime determines eligibility under the approved policy.
    ///
    /// Output: A fallback selection, exhaustion, or cancellation suppression.
    ///
    /// Details:
    /// - Provider failures move directly to the next confirmed model because a schema
    ///   correction cannot repair a failed provider request.
    ///
    /// # Errors
    /// - Returns `Err` when no model answer is active.
    pub fn provider_failed(&mut self) -> Result<FailureDecision, SessionError> {
        if self.phase == Phase::Cancelled {
            return Ok(FailureDecision::Suppressed);
        }
        match self.phase {
            Phase::FullAnswer | Phase::CorrectedAnswer => Ok(self.select_fallback()),
            _ => Err(SessionError::InvalidTransition {
                operation: "provider_failed",
            }),
        }
    }

    /// What: Continue after the correlated `set_model` response succeeds.
    ///
    /// Inputs: None.
    ///
    /// Output: A fresh full-prompt action for the newly selected model.
    ///
    /// Details:
    /// - This explicit barrier prevents sending the hostile-data prompt before Pi confirms
    ///   the in-session fallback selection.
    ///
    /// # Errors
    /// - Returns `Err` unless a fallback selection is currently pending.
    pub fn model_selected(&mut self) -> Result<SessionAction, SessionError> {
        if self.phase != Phase::SelectingModel {
            return Err(SessionError::InvalidTransition {
                operation: "model_selected",
            });
        }
        self.phase = Phase::FullAnswer;
        Ok(SessionAction::SendFullPrompt {
            model_index: self.model_index,
        })
    }

    /// What: Mark the current answer as strictly validated.
    ///
    /// Inputs: None.
    ///
    /// Output: No action; the controller becomes terminal.
    ///
    /// Details:
    /// - Both an original and a corrected answer may complete the attempt.
    ///
    /// # Errors
    /// - Returns `Err` when no answer is awaiting validation.
    pub const fn validated(&mut self) -> Result<(), SessionError> {
        match self.phase {
            Phase::FullAnswer | Phase::CorrectedAnswer => {
                self.phase = Phase::Completed;
                Ok(())
            }
            _ => Err(SessionError::InvalidTransition {
                operation: "validated",
            }),
        }
    }

    /// What: Cancel the whole logical scan and suppress every later correction/fallback.
    ///
    /// Inputs: None.
    ///
    /// Output: `abort_retry` followed by `abort` on the first cancellation, otherwise empty.
    ///
    /// Details:
    /// - The runtime sends these correlated RPC controls, clears outstanding correlation,
    ///   waits the five-second abort grace, then uses process-group termination/reaping.
    #[must_use]
    pub fn cancel(&mut self) -> Vec<SessionAction> {
        if matches!(
            self.phase,
            Phase::Completed | Phase::Exhausted | Phase::Cancelled
        ) {
            return Vec::new();
        }
        self.phase = Phase::Cancelled;
        vec![SessionAction::AbortRetry, SessionAction::Abort]
    }

    /// What: Report the current confirmed model.
    ///
    /// Inputs: None.
    ///
    /// Output: Exact provider/model choice.
    ///
    /// Details:
    /// - Used for attempt provenance and RPC `set_model` field construction.
    #[must_use]
    pub fn current_model(&self) -> &ModelChoice {
        &self.models[self.model_index]
    }

    /// Select the next confirmed fallback or mark the scan exhausted.
    fn select_fallback(&mut self) -> FailureDecision {
        let Some(next_index) = self.model_index.checked_add(1) else {
            self.phase = Phase::Exhausted;
            return FailureDecision::Exhausted;
        };
        let Some(choice) = self.models.get(next_index) else {
            self.phase = Phase::Exhausted;
            return FailureDecision::Exhausted;
        };
        self.model_index = next_index;
        self.phase = Phase::SelectingModel;
        FailureDecision::Action(SessionAction::SelectModel {
            model_index: next_index,
            provider: choice.provider.clone(),
            model: choice.model.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{FailureDecision, ModelChoice, ScanAttemptController, SessionAction, SessionError};

    /// Build a three-model confirmed fallback order.
    fn models() -> Vec<ModelChoice> {
        (1..=3)
            .map(|index| ModelChoice {
                provider: format!("provider-{index}"),
                model: format!("model-{index}"),
            })
            .collect()
    }

    /// Verify one correction per model and settled set-model barriers between full prompts.
    #[test]
    fn correction_and_fallback_order_is_bounded() {
        let mut controller = ScanAttemptController::new(models()).expect("valid models");
        assert_eq!(
            controller.begin().expect("begins"),
            SessionAction::SendFullPrompt { model_index: 0 }
        );
        for index in 0..3 {
            assert_eq!(
                controller.validation_failed().expect("correction"),
                FailureDecision::Action(SessionAction::SendCorrection { model_index: index })
            );
            let fallback = controller.validation_failed().expect("fallback decision");
            if index == 2 {
                assert_eq!(fallback, FailureDecision::Exhausted);
                break;
            }
            assert_eq!(
                fallback,
                FailureDecision::Action(SessionAction::SelectModel {
                    model_index: index + 1,
                    provider: format!("provider-{}", index + 2),
                    model: format!("model-{}", index + 2),
                })
            );
            assert_eq!(
                controller.model_selected().expect("selection settled"),
                SessionAction::SendFullPrompt {
                    model_index: index + 1
                }
            );
        }
    }

    /// Verify an eligible provider failure skips correction and falls back in-session.
    #[test]
    fn provider_failure_moves_directly_to_fallback() {
        let mut controller = ScanAttemptController::new(models()).expect("valid models");
        controller.begin().expect("begins");
        assert!(matches!(
            controller.provider_failed().expect("fallback"),
            FailureDecision::Action(SessionAction::SelectModel { model_index: 1, .. })
        ));
        assert_eq!(controller.current_model().model, "model-2");
    }

    /// Verify cancellation is sticky and suppresses correction and fallback.
    #[test]
    fn cancellation_suppresses_all_later_attempts() {
        for cancel_after_correction in [false, true] {
            let mut controller = ScanAttemptController::new(models()).expect("valid models");
            controller.begin().expect("begins");
            if cancel_after_correction {
                controller.validation_failed().expect("correction action");
            }
            assert_eq!(
                controller.cancel(),
                vec![SessionAction::AbortRetry, SessionAction::Abort]
            );
            assert!(controller.cancel().is_empty(), "cancel is idempotent");
            assert_eq!(
                controller.validation_failed().expect("suppressed"),
                FailureDecision::Suppressed
            );
            assert_eq!(
                controller.provider_failed().expect("suppressed"),
                FailureDecision::Suppressed
            );
            assert!(controller.model_selected().is_err());
            assert!(controller.validated().is_err());
        }
    }

    /// Verify model count and identifier validation fail closed.
    #[test]
    fn model_order_is_validated() {
        assert_eq!(
            ScanAttemptController::new(Vec::new()).expect_err("empty must fail"),
            SessionError::NoModels
        );
        assert!(matches!(
            ScanAttemptController::new(vec![
                ModelChoice {
                    provider: "p".to_string(),
                    model: "m".to_string(),
                };
                4
            ]),
            Err(SessionError::TooManyModels { .. })
        ));
        assert!(matches!(
            ScanAttemptController::new(vec![ModelChoice {
                provider: "p".to_string(),
                model: "bad\nmodel".to_string(),
            }]),
            Err(SessionError::InvalidModelChoice { index: 0 })
        ));
    }

    /// Verify success is terminal and cannot trigger later correction/fallback.
    #[test]
    fn validated_result_is_terminal() {
        let mut controller = ScanAttemptController::new(models()).expect("valid models");
        controller.begin().expect("begins");
        controller.validated().expect("validates");
        assert!(controller.cancel().is_empty());
        assert!(controller.validation_failed().is_err());
        assert!(controller.provider_failed().is_err());
    }
}
