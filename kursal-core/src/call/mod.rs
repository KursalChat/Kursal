pub mod apm;
pub mod audio;
pub mod crypto;
pub mod frame;
#[cfg(target_os = "ios")]
pub mod ios_audio;
pub mod jitter;
pub mod manager;
pub mod media;
pub mod opus;
pub mod signal;
pub mod video;

use crate::Result;
use crate::call::crypto::{CallKeys, derive_call_keys};
use crate::call::signal::HangupReason;
use rand::{TryRngCore, rngs::OsRng};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CallState {
    Idle,
    RingingOut,
    RingingIn,
    Connecting,
    Connected,
    Ended,
}

pub enum CallInput {
    StartCall,
    OfferRecv { random: [u8; 32] },
    AnswerRecv { random: [u8; 32] },
    Accept,
    Decline,
    HangupRecv { reason: HangupReason },
    LocalHangup,
    MediaUp,
    Timeout,
}

pub enum CallAction {
    SendOffer { random: [u8; 32] },
    SendAnswer { random: [u8; 32] },
    SendHangup { reason: HangupReason },
    OpenMedia,
    StartMedia,
    StopMedia,
    Emit(CallState),
}

pub struct CallEngine {
    pub state: CallState,
    pub is_caller: bool,
    pub my_random: Option<[u8; 32]>,
    pub their_random: Option<[u8; 32]>,
}

fn gen_random() -> Option<[u8; 32]> {
    let mut r = [0u8; 32];
    match OsRng.try_fill_bytes(&mut r) {
        Ok(()) => Some(r),
        Err(err) => {
            log::error!("[call] no system randomness available: {err}");
            None
        }
    }
}

impl CallEngine {
    pub fn new() -> Self {
        Self {
            state: CallState::Idle,
            is_caller: false,
            my_random: None,
            their_random: None,
        }
    }

    pub fn derive_keys(&self) -> Option<Result<CallKeys>> {
        match (self.my_random, self.their_random) {
            (Some(my), Some(their)) => Some(derive_call_keys(my, their, self.is_caller)),
            _ => None,
        }
    }

    fn active(&self) -> bool {
        matches!(
            self.state,
            CallState::RingingOut
                | CallState::RingingIn
                | CallState::Connecting
                | CallState::Connected
        )
    }

    pub fn step(&mut self, input: CallInput) -> Vec<CallAction> {
        match (self.state, input) {
            (CallState::Idle, CallInput::StartCall) => {
                let Some(r) = gen_random() else {
                    self.state = CallState::Ended;
                    return vec![CallAction::Emit(self.state)];
                };
                self.is_caller = true;
                self.my_random = Some(r);
                self.state = CallState::RingingOut;
                vec![
                    CallAction::SendOffer { random: r },
                    CallAction::Emit(self.state),
                ]
            }
            (CallState::Idle, CallInput::OfferRecv { random }) => {
                self.is_caller = false;
                self.their_random = Some(random);
                self.state = CallState::RingingIn;
                vec![CallAction::Emit(self.state)]
            }
            (_, CallInput::OfferRecv { .. }) => {
                vec![CallAction::SendHangup {
                    reason: HangupReason::Busy,
                }]
            }
            (CallState::RingingOut, CallInput::AnswerRecv { random }) => {
                self.their_random = Some(random);
                self.state = CallState::Connecting;
                vec![CallAction::OpenMedia, CallAction::Emit(self.state)]
            }
            (CallState::RingingIn, CallInput::Accept) => {
                let Some(r) = gen_random() else {
                    self.state = CallState::Ended;
                    return vec![
                        CallAction::SendHangup {
                            reason: HangupReason::Error,
                        },
                        CallAction::Emit(self.state),
                    ];
                };
                self.my_random = Some(r);
                self.state = CallState::Connecting;
                vec![
                    CallAction::SendAnswer { random: r },
                    CallAction::OpenMedia,
                    CallAction::Emit(self.state),
                ]
            }
            (CallState::RingingIn, CallInput::Decline) => {
                self.state = CallState::Ended;
                vec![
                    CallAction::SendHangup {
                        reason: HangupReason::Declined,
                    },
                    CallAction::Emit(self.state),
                ]
            }
            (CallState::Connecting, CallInput::MediaUp) => {
                self.state = CallState::Connected;
                vec![CallAction::StartMedia, CallAction::Emit(self.state)]
            }
            (_, CallInput::Timeout)
                if matches!(self.state, CallState::RingingOut | CallState::RingingIn) =>
            {
                self.state = CallState::Ended;
                vec![
                    CallAction::SendHangup {
                        reason: HangupReason::Timeout,
                    },
                    CallAction::Emit(self.state),
                ]
            }
            (_, CallInput::LocalHangup) if self.active() => {
                self.state = CallState::Ended;
                vec![
                    CallAction::SendHangup {
                        reason: HangupReason::Hangup,
                    },
                    CallAction::StopMedia,
                    CallAction::Emit(self.state),
                ]
            }
            (_, CallInput::HangupRecv { .. }) if self.active() => {
                self.state = CallState::Ended;
                vec![CallAction::StopMedia, CallAction::Emit(self.state)]
            }
            _ => vec![],
        }
    }
}

impl Default for CallEngine {
    fn default() -> Self {
        Self::new()
    }
}
