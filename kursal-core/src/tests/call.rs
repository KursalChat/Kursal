mod crypto {
    use crate::call::crypto::{derive_call_keys, derive_video_keys};
    use crate::crypto::stream::{stream_decrypt, stream_encrypt};

    #[test]
    fn caller_tx_matches_callee_rx() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let caller = derive_call_keys(a, b, true).unwrap();
        let callee = derive_call_keys(b, a, false).unwrap();
        assert_eq!(caller.tx, callee.rx);
        assert_eq!(caller.rx, callee.tx);
        assert_ne!(caller.tx, caller.rx);
    }

    #[test]
    fn seal_open_roundtrip() {
        let keys = derive_call_keys([3u8; 32], [4u8; 32], true).unwrap();
        let pt = b"opus-frame-bytes";
        let ct = stream_encrypt(&keys.tx, pt).unwrap();
        assert_ne!(ct.as_slice(), pt);
        let back = stream_decrypt(&keys.tx, &ct).unwrap();
        assert_eq!(back, pt);
    }

    #[test]
    fn wrong_key_fails() {
        let k1 = derive_call_keys([5u8; 32], [6u8; 32], true).unwrap();
        let k2 = derive_call_keys([7u8; 32], [8u8; 32], true).unwrap();
        let ct = stream_encrypt(&k1.tx, b"x").unwrap();
        assert!(stream_decrypt(&k2.tx, &ct).is_err());
    }

    #[test]
    fn video_keys_symmetric() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let caller = derive_video_keys(a, b, true).unwrap();
        let callee = derive_video_keys(b, a, false).unwrap();
        assert_eq!(caller.tx, callee.rx);
        assert_eq!(caller.rx, callee.tx);
        assert_ne!(caller.tx, caller.rx);
    }

    #[test]
    fn video_keys_distinct_from_audio() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let audio = derive_call_keys(a, b, true).unwrap();
        let video = derive_video_keys(a, b, true).unwrap();
        assert_ne!(audio.tx, video.tx);
        assert_ne!(audio.rx, video.rx);
    }
}

mod signal {
    use crate::{
        call::signal::{
            HangupPayload, HangupReason, OfferPayload, VideoStartPayload, VideoStopPayload,
            VideoStopReason, decode, encode,
        },
        storage::valid_call_sample_rate,
    };

    #[test]
    fn offer_roundtrip() {
        let o = OfferPayload {
            random: [9u8; 32],
            sample_rate: 48000,
            channels: 1,
        };
        let bytes = encode(&o).unwrap();
        let back: OfferPayload = decode(&bytes).unwrap();
        assert_eq!(back.random, o.random);
        assert_eq!(back.sample_rate, 48000);
        assert_eq!(back.channels, 1);
    }

    #[test]
    fn hangup_reason_roundtrip() {
        let h = HangupPayload {
            reason: HangupReason::Timeout,
        };
        let back: HangupPayload = decode(&encode(&h).unwrap()).unwrap();
        assert!(matches!(back.reason, HangupReason::Timeout));
    }

    #[test]
    fn call_sample_rate_validation() {
        assert_eq!(valid_call_sample_rate(16000), 16000);
        assert_eq!(valid_call_sample_rate(24000), 24000);
        assert_eq!(valid_call_sample_rate(48000), 48000);
        assert_eq!(valid_call_sample_rate(0), 48000);
        assert_eq!(valid_call_sample_rate(44100), 48000);
        assert_eq!(valid_call_sample_rate(u32::MAX), 48000);
    }

    #[test]
    fn video_start_roundtrip() {
        let v = VideoStartPayload {
            codec: "avc1.42E01F".to_string(),
            width: 854,
            height: 480,
        };
        let back: VideoStartPayload = decode(&encode(&v).unwrap()).unwrap();
        assert_eq!(back.codec, "avc1.42E01F");
        assert_eq!(back.width, 854);
        assert_eq!(back.height, 480);
    }

    #[test]
    fn video_stop_roundtrip() {
        let v = VideoStopPayload {
            reason: VideoStopReason::Unsupported,
        };
        let back: VideoStopPayload = decode(&encode(&v).unwrap()).unwrap();
        assert_eq!(back.reason, VideoStopReason::Unsupported);
    }
}

mod frame {
    use crate::call::frame::{decode_frame, encode_frame};

    #[test]
    fn frame_roundtrip() {
        let body = [1u8, 2, 3, 4];
        let f = encode_frame(513, &body);
        let (seq, payload) = decode_frame(&f).unwrap();
        assert_eq!(seq, 513);
        assert_eq!(payload, &body);
    }

    #[test]
    fn short_frame_errors() {
        assert!(decode_frame(&[0u8]).is_err());
    }
}

mod jitter {
    use crate::call::jitter::{JitterBuffer, JitterOut};

    #[test]
    fn waits_for_target_then_emits_in_order() {
        let mut jb = JitterBuffer::new(2);
        jb.push(1, vec![1]);
        assert!(matches!(jb.pop(), JitterOut::Empty));
        jb.push(2, vec![2]);
        assert!(matches!(jb.pop(), JitterOut::Frame(f) if f == vec![1]));
        assert!(matches!(jb.pop(), JitterOut::Frame(f) if f == vec![2]));
    }

    #[test]
    fn reorders_late_frame() {
        let mut jb = JitterBuffer::new(2);
        jb.push(2, vec![2]);
        jb.push(1, vec![1]);
        assert!(matches!(jb.pop(), JitterOut::Frame(f) if f == vec![1]));
        assert!(matches!(jb.pop(), JitterOut::Frame(f) if f == vec![2]));
    }

    #[test]
    fn missing_frame_conceals() {
        let mut jb = JitterBuffer::new(2);
        jb.push(1, vec![1]);
        jb.push(3, vec![3]);
        assert!(matches!(jb.pop(), JitterOut::Frame(f) if f == vec![1]));
        assert!(matches!(jb.pop(), JitterOut::Conceal));
        assert!(matches!(jb.pop(), JitterOut::Frame(f) if f == vec![3]));
    }
}

mod engine {
    use crate::call::signal::HangupReason;
    use crate::call::{CallAction, CallEngine, CallInput, CallState};

    #[test]
    fn caller_flow_to_connected() {
        let mut e = CallEngine::new();
        let a = e.step(CallInput::StartCall);
        assert!(matches!(e.state, CallState::RingingOut));
        assert!(a.iter().any(|x| matches!(x, CallAction::SendOffer { .. })));

        e.step(CallInput::AnswerRecv { random: [2u8; 32] });
        assert!(matches!(e.state, CallState::Connecting));

        let a = e.step(CallInput::MediaUp);
        assert!(matches!(e.state, CallState::Connected));
        assert!(a.iter().any(|x| matches!(x, CallAction::StartMedia)));
    }

    #[test]
    fn callee_decline_sends_hangup() {
        let mut e = CallEngine::new();
        e.step(CallInput::OfferRecv { random: [1u8; 32] });
        assert!(matches!(e.state, CallState::RingingIn));
        let a = e.step(CallInput::Decline);
        assert!(matches!(e.state, CallState::Ended));
        assert!(a.iter().any(|x| matches!(
            x,
            CallAction::SendHangup {
                reason: HangupReason::Declined
            }
        )));
    }

    #[test]
    fn ring_timeout_ends() {
        let mut e = CallEngine::new();
        e.step(CallInput::StartCall);
        let a = e.step(CallInput::Timeout);
        assert!(matches!(e.state, CallState::Ended));
        assert!(a.iter().any(|x| matches!(
            x,
            CallAction::SendHangup {
                reason: HangupReason::Timeout
            }
        )));
    }
}

mod opus {
    use crate::call::audio::to_i16_sample;
    use crate::call::opus::OpusCodec;

    #[test]
    fn encode_then_decode_roundtrip() {
        let mut c = OpusCodec::new(48000).unwrap();
        let pcm: Vec<i16> = (0..960)
            .map(|i| to_i16_sample((i as f32 * 0.1).sin()))
            .collect();
        let pkt = c.encode(&pcm).unwrap();
        assert!(!pkt.is_empty());
        let mut out = vec![0i16; 960];
        let n = c.decode(Some(&pkt), &mut out).unwrap();
        assert_eq!(n, 960);
    }

    #[test]
    fn plc_decode_produces_frame() {
        let mut c = OpusCodec::new(48000).unwrap();
        let mut out = vec![0i16; 960];
        let n = c.decode(None, &mut out).unwrap();
        assert_eq!(n, 960);
    }
}

mod manager {
    use crate::call::manager::{CALL_SAMPLE_RATE, build_signal};
    use crate::call::{CallAction, CallEngine, CallInput};
    use crate::messaging::enums::{KursalMessage, MessageId};

    #[test]
    fn offer_action_builds_call_signal() {
        let mut e = CallEngine::new();
        let actions = e.step(CallInput::StartCall);
        let offer = actions
            .iter()
            .find(|a| matches!(a, CallAction::SendOffer { .. }))
            .unwrap();
        let msg = build_signal(MessageId::new(), offer, CALL_SAMPLE_RATE)
            .unwrap()
            .unwrap();
        assert!(matches!(msg, KursalMessage::CallSignal(_)));
    }

    #[test]
    fn emit_action_builds_nothing() {
        let none =
            build_signal(MessageId::new(), &CallAction::StartMedia, CALL_SAMPLE_RATE).unwrap();
        assert!(none.is_none());
    }
}

mod replay {
    use crate::call::media::ReplayWindow;

    #[test]
    fn accepts_fresh_rejects_replay() {
        let mut w = ReplayWindow::new();
        assert!(w.accept(0));
        assert!(!w.accept(0));
        assert!(w.accept(1));
        assert!(!w.accept(1));
        assert!(!w.accept(0));
    }

    #[test]
    fn accepts_reorder_within_window() {
        let mut w = ReplayWindow::new();
        assert!(w.accept(10));
        assert!(w.accept(12));
        assert!(w.accept(11));
        assert!(!w.accept(11));
        assert!(w.accept(9));
        assert!(!w.accept(9));
    }

    #[test]
    fn rejects_too_old() {
        let mut w = ReplayWindow::new();
        assert!(w.accept(0));
        assert!(w.accept(100));
        assert!(!w.accept(36));
        assert!(w.accept(37));
        assert!(!w.accept(37));
    }

    #[test]
    fn big_jump_resets_bitmap() {
        let mut w = ReplayWindow::new();
        assert!(w.accept(5));
        assert!(w.accept(500));
        assert!(!w.accept(500));
        assert!(w.accept(499));
        assert!(!w.accept(5));
    }

    #[test]
    fn first_seq_can_be_nonzero() {
        let mut w = ReplayWindow::new();
        assert!(w.accept(42));
        assert!(!w.accept(42));
        assert!(w.accept(43));
    }
}

mod frame_wide_seq {
    use crate::call::frame::{decode_frame, encode_frame};

    #[test]
    fn seq_beyond_u16_survives() {
        let body = [7u8; 3];
        let seq: u64 = 70_000;
        let wire = encode_frame(seq, &body);
        let (got, payload) = decode_frame(&wire).unwrap();
        assert_eq!(got, seq);
        assert_eq!(payload, &body);

        let big: u64 = u64::MAX - 1;
        let (got, _) = decode_frame(&encode_frame(big, &[1])).unwrap();
        assert_eq!(got, big);
    }

    #[test]
    fn seven_byte_frame_errors() {
        assert!(decode_frame(&[0u8; 7]).is_err());
    }
}

mod video_signal {
    use crate::call::manager::{build_video_start, build_video_stop};
    use crate::call::signal::{VideoStartPayload, VideoStopPayload, VideoStopReason, decode};
    use crate::messaging::enums::{CallSignalKind, KursalMessage, MessageId};

    #[test]
    fn video_start_signal_roundtrip() {
        let id = MessageId::new();
        let msg = build_video_start(id, "vp8".to_string(), 640, 360).unwrap();
        let KursalMessage::CallSignal(cs) = msg else {
            panic!("expected CallSignal");
        };
        assert_eq!(cs.call_id, id);
        assert!(matches!(cs.kind, CallSignalKind::VideoStart));
        let p: VideoStartPayload = decode(&cs.payload).unwrap();
        assert_eq!(p.codec, "vp8");
        assert_eq!(p.width, 640);
        assert_eq!(p.height, 360);
    }

    #[test]
    fn video_stop_signal_roundtrip() {
        let id = MessageId::new();
        let msg = build_video_stop(id, VideoStopReason::Error).unwrap();
        let KursalMessage::CallSignal(cs) = msg else {
            panic!("expected CallSignal");
        };
        assert!(matches!(cs.kind, CallSignalKind::VideoStop));
        let p: VideoStopPayload = decode(&cs.payload).unwrap();
        assert_eq!(p.reason, VideoStopReason::Error);
    }
}

mod video_wire {
    use crate::call::crypto::derive_video_keys;
    use crate::call::video::{MAX_VIDEO_FRAME_BYTES, open_frame, seal_frame};

    #[test]
    fn seal_open_roundtrip() {
        let keys = derive_video_keys([3u8; 32], [4u8; 32], true).unwrap();
        let peer = derive_video_keys([4u8; 32], [3u8; 32], false).unwrap();
        let chunk = vec![7u8; 1024];
        let wire = seal_frame(&keys.tx, 42, &chunk).unwrap();
        let (seq, back) = open_frame(&peer.rx, &wire).unwrap();
        assert_eq!(seq, 42);
        assert_eq!(back, chunk);
    }

    #[test]
    fn tampered_seq_fails() {
        let keys = derive_video_keys([5u8; 32], [6u8; 32], true).unwrap();
        let mut wire = seal_frame(&keys.tx, 1, b"data").unwrap();
        wire[7] = 9;
        assert!(open_frame(&keys.tx, &wire).is_err());
    }

    #[test]
    fn max_frame_fits_keyframes() {
        const {
            assert!(MAX_VIDEO_FRAME_BYTES >= 256 * 1024);
        }
    }

    #[test]
    fn video_quality_validation() {
        use crate::storage::valid_video_quality;
        assert_eq!(valid_video_quality(360), 360);
        assert_eq!(valid_video_quality(480), 480);
        assert_eq!(valid_video_quality(720), 720);
        assert_eq!(valid_video_quality(1080), 480);
        assert_eq!(valid_video_quality(0), 480);
    }
}
