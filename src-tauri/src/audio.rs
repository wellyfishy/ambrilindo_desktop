use std::io::Cursor;
use std::sync::mpsc::{channel, Sender};
use std::thread;
use tracing::{error, info};

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum SoundType {
    Warning,
    Finish,
}

#[derive(Clone)]
pub struct AudioService {
    sender: Sender<SoundType>,
}

impl AudioService {
    pub fn new() -> Self {
        let (tx, rx) = channel::<SoundType>();

        thread::Builder::new()
            .name("arena-audio-worker".into())
            .spawn(move || {
                let stream_res = rodio::OutputStream::try_default();
                let (_stream, stream_handle) = match stream_res {
                    Ok(res) => res,
                    Err(e) => {
                        error!("Gagal menginisialisasi output audio OS: {}", e);
                        return;
                    }
                };

                info!("Arena Native Audio Service initialized successfully");

                let mut active_sinks: Vec<rodio::Sink> = Vec::new();

                while let Ok(sound) = rx.recv() {
                    // Clean up finished sinks
                    active_sinks.retain(|s| !s.empty());

                    let bytes: &'static [u8] = match sound {
                        SoundType::Warning => include_bytes!("../resources/sound/warning.mp3"),
                        SoundType::Finish => include_bytes!("../resources/sound/finish.mp3"),
                    };

                    let cursor = Cursor::new(bytes);
                    match rodio::Decoder::new(cursor) {
                        Ok(source) => {
                            match rodio::Sink::try_new(&stream_handle) {
                                Ok(sink) => {
                                    sink.append(source);
                                    active_sinks.push(sink);
                                    info!("Playing sound: {:?}", sound);
                                }
                                Err(e) => {
                                    error!("Gagal membuat audio sink: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Gagal decode audio file: {}", e);
                        }
                    }
                }
            })
            .expect("Failed to spawn audio worker thread");

        Self { sender: tx }
    }

    pub fn play(&self, sound: SoundType) -> Result<(), String> {
        self.sender
            .send(sound)
            .map_err(|e| format!("Audio channel error: {}", e))
    }
}
