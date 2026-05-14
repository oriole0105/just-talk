//! Phase 8 integration tests — App state machine.
//!
//! `apply_transition` is a pure function, so all tests here run without
//! audio hardware, network, or a display server.

use just_talk::app::{apply_transition, AppEvent, AppState};
use just_talk::error::JustTalkError;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn hotkey() -> AppEvent {
    AppEvent::HotkeyPressed
}
fn tdone(s: &str) -> AppEvent {
    AppEvent::TranscribeDone(s.into())
}
fn rdone(s: &str) -> AppEvent {
    AppEvent::RefineDone(s.into())
}
fn err() -> AppEvent {
    AppEvent::Error(JustTalkError::audio("mock error"))
}

// ---------------------------------------------------------------------------
// Happy-path
// ---------------------------------------------------------------------------

#[test]
fn full_cycle_state_sequence() {
    let steps: &[(AppState, AppEvent, AppState)] = &[
        (AppState::Idle, hotkey(), AppState::Recording),
        (AppState::Recording, hotkey(), AppState::Transcribing),
        (AppState::Transcribing, tdone("raw"), AppState::Refining),
        (AppState::Refining, rdone("refined"), AppState::Injecting),
        (AppState::Injecting, AppEvent::OutputDone, AppState::Idle),
    ];

    let mut state = AppState::Idle;
    for (from, event, expected) in steps {
        let next = apply_transition(from, event);
        assert_eq!(next, Some(expected.clone()), "from={from:?}");
        state = next.unwrap();
    }
    assert_eq!(state, AppState::Idle, "cycle returns to Idle");
}

// ---------------------------------------------------------------------------
// Error recovery
// ---------------------------------------------------------------------------

#[test]
fn error_resets_to_idle_from_every_state() {
    let all_states = [
        AppState::Idle,
        AppState::Recording,
        AppState::Transcribing,
        AppState::Refining,
        AppState::Injecting,
    ];
    for s in all_states {
        assert_eq!(
            apply_transition(&s, &err()),
            Some(AppState::Idle),
            "Error from {s:?} should → Idle"
        );
    }
}

// ---------------------------------------------------------------------------
// Events that produce no state change
// ---------------------------------------------------------------------------

#[test]
fn quit_returns_none() {
    for s in [AppState::Idle, AppState::Recording, AppState::Transcribing] {
        assert_eq!(apply_transition(&s, &AppEvent::Quit), None);
    }
}

#[test]
fn reload_config_returns_none() {
    for s in [AppState::Idle, AppState::Injecting] {
        assert_eq!(apply_transition(&s, &AppEvent::ReloadConfig), None);
    }
}

// ---------------------------------------------------------------------------
// Out-of-order events are ignored
// ---------------------------------------------------------------------------

#[test]
fn hotkey_ignored_in_transcribing_and_injecting() {
    assert_eq!(apply_transition(&AppState::Transcribing, &hotkey()), None);
    assert_eq!(apply_transition(&AppState::Injecting, &hotkey()), None);
}

#[test]
fn transcribe_done_ignored_outside_transcribing() {
    for s in [AppState::Idle, AppState::Recording, AppState::Injecting] {
        assert_eq!(apply_transition(&s, &tdone("x")), None, "state={s:?}");
    }
}

#[test]
fn refine_done_ignored_outside_refining() {
    for s in [AppState::Idle, AppState::Transcribing, AppState::Injecting] {
        assert_eq!(apply_transition(&s, &rdone("x")), None, "state={s:?}");
    }
}

#[test]
fn output_done_ignored_outside_injecting() {
    for s in [AppState::Idle, AppState::Recording, AppState::Refining] {
        assert_eq!(
            apply_transition(&s, &AppEvent::OutputDone),
            None,
            "state={s:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Consecutive cycles work correctly
// ---------------------------------------------------------------------------

#[test]
fn two_full_cycles() {
    let cycle: &[(AppEvent, AppState)] = &[
        (hotkey(), AppState::Recording),
        (hotkey(), AppState::Transcribing),
        (tdone("a"), AppState::Refining),
        (rdone("A"), AppState::Injecting),
        (AppEvent::OutputDone, AppState::Idle),
    ];

    let mut state = AppState::Idle;
    for _ in 0..2 {
        for (event, expected) in cycle {
            state = apply_transition(&state, event).unwrap_or(state.clone());
            assert_eq!(&state, expected);
        }
    }
}
