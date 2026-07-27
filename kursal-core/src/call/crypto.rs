use crate::Result;
use crate::crypto::derive_key;

pub struct CallKeys {
    pub tx: [u8; 32],
    pub rx: [u8; 32],
}

fn derive_directional_keys(
    my_random: [u8; 32],
    their_random: [u8; 32],
    is_caller: bool,
    c2a: &[u8],
    a2c: &[u8],
) -> Result<CallKeys> {
    let (caller_r, callee_r) = if is_caller {
        (my_random, their_random)
    } else {
        (their_random, my_random)
    };
    let combined = [caller_r, callee_r].concat();
    let caller_to_callee = derive_key(&combined, c2a)?;
    let callee_to_caller = derive_key(&combined, a2c)?;
    if is_caller {
        Ok(CallKeys {
            tx: caller_to_callee,
            rx: callee_to_caller,
        })
    } else {
        Ok(CallKeys {
            tx: callee_to_caller,
            rx: caller_to_callee,
        })
    }
}

pub fn derive_call_keys(
    my_random: [u8; 32],
    their_random: [u8; 32],
    is_caller: bool,
) -> Result<CallKeys> {
    derive_directional_keys(
        my_random,
        their_random,
        is_caller,
        b"kursal-call-c2a",
        b"kursal-call-a2c",
    )
}

pub fn derive_video_keys(
    my_random: [u8; 32],
    their_random: [u8; 32],
    is_caller: bool,
) -> Result<CallKeys> {
    derive_directional_keys(
        my_random,
        their_random,
        is_caller,
        b"kursal-call-vid-c2a",
        b"kursal-call-vid-a2c",
    )
}
