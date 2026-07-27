use objc2_avf_audio::{
    AVAudioSession, AVAudioSessionCategoryOptions, AVAudioSessionCategoryPlayAndRecord,
    AVAudioSessionModeVoiceChat,
};

pub fn activate_voice_session() {
    unsafe {
        let session = AVAudioSession::sharedInstance();
        if let (Some(category), Some(mode)) = (
            AVAudioSessionCategoryPlayAndRecord,
            AVAudioSessionModeVoiceChat,
        ) {
            let _ = session.setCategory_mode_options_error(
                category,
                mode,
                AVAudioSessionCategoryOptions(0),
            );
        }
        let _ = session.setActive_error(true);
    }
}

pub fn deactivate_session() {
    unsafe {
        let session = AVAudioSession::sharedInstance();
        let _ = session.setActive_error(false);
    }
}
