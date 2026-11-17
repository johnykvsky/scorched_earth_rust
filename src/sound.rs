//! Generated mono PC-speaker-style WAVs and a single-voice playback director.
//! Game code only posts events; the actual audio device lives in main's runtime.
use crate::gfx::H;
use crate::settings::FlightSound;
use macroquad::audio::{load_sound_from_bytes, play_sound, stop_sound, PlaySoundParams, Sound};

pub const SAMPLE_RATE: u32 = 44_100;
pub const FLIGHT_BINS: usize = 32;
const AMPLITUDE: f32 = 6000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Cue {
    Click,
    Fire,
    Riot,
    Explosion,
    Nuke,
    Death,
    ShieldHit,
    Split,
    Dig,
    Dirt,
    Disruptor,
    Thunder,
    Laser,
    Plasma,
    Splash,
    Burn,
}
impl Cue {
    pub const ALL: [Self; 16] = [
        Self::Click,
        Self::Fire,
        Self::Riot,
        Self::Explosion,
        Self::Nuke,
        Self::Death,
        Self::ShieldHit,
        Self::Split,
        Self::Dig,
        Self::Dirt,
        Self::Disruptor,
        Self::Thunder,
        Self::Laser,
        Self::Plasma,
        Self::Splash,
        Self::Burn,
    ];
    fn priority(self) -> u8 {
        match self {
            Self::Click => 0,
            Self::Split => 1,
            Self::Fire => 2,
            Self::Dig | Self::Dirt | Self::Burn => 3,
            Self::Riot | Self::Disruptor | Self::Laser | Self::Plasma | Self::Splash => 4,
            Self::Explosion => 5,
            Self::ShieldHit => 6,
            Self::Nuke | Self::Thunder => 7,
            Self::Death => 8,
        }
    }
    pub fn tones(self) -> Vec<Tone> {
        let sequence =
            |frequencies: &[u32], ms| frequencies.iter().map(|&hz| Tone { hz, ms }).collect();
        match self {
            Self::Splash => sequence(&[2400, 800, 1800, 400, 900], 18),
            Self::Burn => sequence(&[200, 450, 0, 300, 650, 150, 0, 400], 8),
            Self::Laser => sequence(&[8000, 6400, 4800, 3200, 1600], 25),
            Self::Plasma => sequence(&[300, 600, 1200, 2400, 1200, 600, 300], 45),
            Self::Thunder => sequence(&[170, 350, 90, 230, 70, 120, 50], 35),
            Self::Click => sequence(&[1000], 10),
            Self::Fire => sequence(&[1000, 2500, 4000, 5500, 7000, 8500, 10000], 7),
            Self::ShieldHit => sequence(&[1000], 30),
            Self::Split => sequence(&[2500, 5000, 7500], 4),
            Self::Dirt => sequence(&[1800, 1200, 800, 500, 300], 10),
            Self::Dig => (0..18)
                .map(|i| Tone {
                    hz: [300, 600, 0][i % 3],
                    ms: 8,
                })
                .collect(),
            Self::Disruptor => (0..200)
                .map(|i| Tone {
                    hz: if i % 2 == 0 { 1000 } else { 0 },
                    ms: 4,
                })
                .collect(),
            Self::Riot => (0..40)
                .map(|i| Tone {
                    hz: 200 + (if i < 20 { i } else { 39 - i }) * 100,
                    ms: 6,
                })
                .collect(),
            Self::Explosion => (0..32)
                .map(|i| Tone {
                    hz: 1800 - i * 50,
                    ms: 4,
                })
                .collect(),
            Self::Nuke => (0..48)
                .map(|i| Tone {
                    hz: 1500 - i * 30,
                    ms: 10,
                })
                .collect(),
            Self::Death => (0..16)
                .map(|i| Tone {
                    hz: 800 - i * 46,
                    ms: 20,
                })
                .collect(),
        }
    }
    fn duration(self) -> f32 {
        self.tones()
            .iter()
            .map(|tone| tone.ms as f32 / 1000.0)
            .sum()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tone {
    pub hz: u32,
    pub ms: u32,
}

fn wav(samples: &[i16]) -> Vec<u8> {
    let size = samples.len() as u32 * 2;
    let mut bytes = Vec::with_capacity(44 + size as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + size).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&1_u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&size.to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

pub fn synthesize(tones: &[Tone]) -> Vec<u8> {
    let mut samples = Vec::new();
    let mut phase = 0.0_f64;
    let ramp = (SAMPLE_RATE / 1000) as usize;
    for tone in tones {
        let length = (u64::from(SAMPLE_RATE) * u64::from(tone.ms) / 1000) as usize;
        for i in 0..length {
            let envelope = (i as f32 / ramp as f32)
                .min((length - 1 - i) as f32 / ramp as f32)
                .min(1.0);
            let value = if tone.hz == 0 {
                0.0
            } else {
                if phase < 0.5 {
                    AMPLITUDE
                } else {
                    -AMPLITUDE
                }
            };
            samples.push((value * envelope) as i16);
            phase = (phase + f64::from(tone.hz) / f64::from(SAMPLE_RATE)).fract();
        }
    }
    wav(&samples)
}

/// Integer even periods make every cached flight tone a seamless, zero-DC loop.
pub fn flight_wav(bin: usize) -> Vec<u8> {
    let hz = 220.0 + 1980.0 * bin.min(FLIGHT_BINS - 1) as f32 / (FLIGHT_BINS - 1) as f32;
    let period = ((SAMPLE_RATE as f32 / hz / 2.0).round() as usize * 2).max(4);
    wav(&(0..period * 8)
        .map(|i| if i % period < period / 2 { 2400 } else { -2400 })
        .collect::<Vec<_>>())
}

pub fn flight_bin(mode: FlightSound, y: f32, speed: f32) -> Option<usize> {
    let value = match mode {
        FlightSound::Off => return None,
        FlightSound::Position => (H as f32 - y) / H as f32,
        FlightSound::Velocity => speed / 1000.0,
    };
    Some((value.clamp(0.0, 1.0) * (FLIGHT_BINS - 1) as f32).round() as usize)
}

/// Deduplicate per-frame events and keep memory bounded even without a player/device.
#[derive(Default)]
pub struct Requests(u16);
impl Requests {
    pub fn post(&mut self, cue: Cue) {
        self.0 |= 1 << cue as u8;
    }
    pub fn take(&mut self) -> Vec<Cue> {
        let cues = Cue::ALL
            .into_iter()
            .filter(|cue| self.0 & (1 << *cue as u8) != 0)
            .collect();
        self.0 = 0;
        cues
    }
}
pub struct Frame {
    pub enabled: bool,
    pub flight: Option<usize>,
    pub cues: Vec<Cue>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Voice {
    Effect { cue: Cue, serial: u64 },
    Flight(usize),
}
#[derive(Default)]
pub struct Director {
    active: Option<(Cue, f32)>,
    serial: u64,
}
impl Director {
    pub fn update(&mut self, frame: &Frame, dt: f32) -> Option<Voice> {
        if !frame.enabled {
            self.active = None;
            return None;
        }
        if let Some((_, remaining)) = &mut self.active {
            *remaining -= dt.max(0.0);
        }
        if self.active.is_some_and(|(_, remaining)| remaining <= 0.0) {
            self.active = None;
        }
        if let Some(&cue) = frame.cues.iter().max_by_key(|cue| cue.priority()) {
            if self
                .active
                .is_none_or(|(active, _)| cue.priority() >= active.priority())
            {
                self.active = Some((cue, cue.duration()));
                self.serial = self.serial.wrapping_add(1);
            }
        }
        if let Some((cue, _)) = self.active {
            Some(Voice::Effect {
                cue,
                serial: self.serial,
            })
        } else {
            frame
                .flight
                .map(|bin| Voice::Flight(bin.min(FLIGHT_BINS - 1)))
        }
    }
}

pub struct Player {
    effects: Vec<Sound>,
    flight: Vec<Sound>,
    director: Director,
    playing: Option<Voice>,
}
impl Player {
    pub async fn load() -> Result<Self, String> {
        let mut effects = Vec::new();
        for cue in Cue::ALL {
            effects.push(
                load_sound_from_bytes(&synthesize(&cue.tones()))
                    .await
                    .map_err(|error| format!("Cannot load {cue:?} sound: {error}"))?,
            );
        }
        let mut flight = Vec::new();
        for bin in 0..FLIGHT_BINS {
            flight.push(
                load_sound_from_bytes(&flight_wav(bin))
                    .await
                    .map_err(|error| format!("Cannot load flight sound: {error}"))?,
            );
        }
        Ok(Self {
            effects,
            flight,
            director: Director::default(),
            playing: None,
        })
    }
    fn sample(&self, voice: Voice) -> &Sound {
        match voice {
            Voice::Effect { cue, .. } => &self.effects[cue as usize],
            Voice::Flight(bin) => &self.flight[bin],
        }
    }
    pub fn update(&mut self, frame: Frame, dt: f32) {
        let next = self.director.update(&frame, dt);
        if next == self.playing {
            return;
        }
        if let Some(voice) = self.playing {
            stop_sound(self.sample(voice));
        }
        if let Some(voice) = next {
            play_sound(
                self.sample(voice),
                PlaySoundParams {
                    looped: matches!(voice, Voice::Flight(_)),
                    volume: 1.0,
                },
            );
        }
        self.playing = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn samples(bytes: &[u8]) -> Vec<i16> {
        bytes[44..]
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect()
    }
    fn frame(enabled: bool, flight: Option<usize>, cues: &[Cue]) -> Frame {
        Frame {
            enabled,
            flight,
            cues: cues.to_vec(),
        }
    }

    #[test]
    fn every_effect_is_valid_bounded_mono_pcm_with_the_expected_duration() {
        for cue in Cue::ALL {
            let tones = cue.tones();
            let bytes = synthesize(&tones);
            assert_eq!(&bytes[..4], b"RIFF");
            assert_eq!(&bytes[8..16], b"WAVEfmt ");
            assert_eq!(
                u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize + 8,
                bytes.len()
            );
            assert_eq!(u16::from_le_bytes(bytes[20..22].try_into().unwrap()), 1);
            assert_eq!(u16::from_le_bytes(bytes[22..24].try_into().unwrap()), 1);
            assert_eq!(
                u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
                SAMPLE_RATE
            );
            assert_eq!(u16::from_le_bytes(bytes[34..36].try_into().unwrap()), 16);
            assert_eq!(&bytes[36..40], b"data");
            let pcm = samples(&bytes);
            let length: usize = tones
                .iter()
                .map(|tone| (u64::from(SAMPLE_RATE) * u64::from(tone.ms) / 1000) as usize)
                .sum();
            assert_eq!(pcm.len(), length);
            assert_eq!(pcm.first(), Some(&0));
            assert_eq!(pcm.last(), Some(&0));
            assert!(pcm.iter().all(|&sample| sample.abs() <= AMPLITUDE as i16));
            assert!(pcm.iter().any(|&sample| sample > 0) && pcm.iter().any(|&sample| sample < 0));
        }
        assert!(samples(&synthesize(&[Tone { hz: 0, ms: 20 }]))
            .iter()
            .all(|&sample| sample == 0));
    }

    #[test]
    fn firing_uses_the_recovered_chirp_and_the_shield_tone_is_one_kilohertz() {
        assert_eq!(
            Cue::Fire
                .tones()
                .iter()
                .map(|tone| tone.hz)
                .collect::<Vec<_>>(),
            vec![1000, 2500, 4000, 5500, 7000, 8500, 10000]
        );
        assert_eq!(Cue::ShieldHit.tones(), vec![Tone { hz: 1000, ms: 30 }]);
        let pcm = samples(&synthesize(&[Tone { hz: 1000, ms: 100 }]));
        let crossings = pcm
            .windows(2)
            .filter(|pair| i32::from(pair[0]) * i32::from(pair[1]) < 0)
            .count();
        let measured = crossings as f32 / (2.0 * pcm.len() as f32 / SAMPLE_RATE as f32);
        assert!((measured - 1000.0).abs() < 15.0);
        assert!(Cue::Riot.tones().first().unwrap().hz < Cue::Riot.tones()[19].hz);
        assert_eq!(
            Cue::Riot.tones().first().unwrap().hz,
            Cue::Riot.tones().last().unwrap().hz
        );
    }

    #[test]
    fn flight_tones_are_seamless_zero_dc_loops_and_pitch_follows_the_selected_quantity() {
        let mut previous_period = usize::MAX;
        for bin in 0..FLIGHT_BINS {
            let pcm = samples(&flight_wav(bin));
            let period = pcm.len() / 8;
            assert!(period <= previous_period);
            previous_period = period;
            assert_eq!(pcm.iter().map(|&sample| i64::from(sample)).sum::<i64>(), 0);
            for cycle in pcm.chunks_exact(period) {
                assert_eq!(cycle, &pcm[..period]);
            }
        }
        assert_eq!(flight_bin(FlightSound::Off, 100.0, 500.0), None);
        assert!(
            flight_bin(FlightSound::Position, 100.0, 0.0)
                > flight_bin(FlightSound::Position, 400.0, 0.0)
        );
        assert!(
            flight_bin(FlightSound::Velocity, 0.0, 800.0)
                > flight_bin(FlightSound::Velocity, 0.0, 100.0)
        );
        assert_eq!(
            flight_bin(FlightSound::Position, -1000.0, 0.0),
            Some(FLIGHT_BINS - 1)
        );
        assert_eq!(
            flight_bin(FlightSound::Velocity, 0.0, 2000.0),
            Some(FLIGHT_BINS - 1)
        );
        assert_eq!(
            flight_bin(FlightSound::Position, H as f32 + 100.0, 0.0),
            Some(0)
        );
    }

    #[test]
    fn director_is_monophonic_preempts_by_priority_and_resumes_the_flight_tone() {
        let mut director = Director::default();
        assert_eq!(
            director.update(&frame(true, Some(3), &[]), 0.01),
            Some(Voice::Flight(3))
        );
        let fire = director.update(&frame(true, Some(3), &[Cue::Fire]), 0.01);
        assert!(matches!(fire, Some(Voice::Effect { cue: Cue::Fire, .. })));
        assert_eq!(
            director.update(&frame(true, Some(3), &[Cue::Click]), 0.01),
            fire
        );
        let high = director.update(
            &frame(true, Some(3), &[Cue::ShieldHit, Cue::Nuke, Cue::Explosion]),
            0.01,
        );
        assert!(matches!(high, Some(Voice::Effect { cue: Cue::Nuke, .. })));
        assert_eq!(
            director.update(&frame(true, Some(9), &[]), 1.0),
            Some(Voice::Flight(9))
        );
        assert_eq!(director.update(&frame(true, None, &[]), 0.01), None);
    }

    #[test]
    fn muting_stops_effects_and_flight_and_discards_events_without_a_backlog() {
        let mut director = Director::default();
        director.update(&frame(true, Some(3), &[Cue::Death]), 0.01);
        assert_eq!(
            director.update(&frame(false, Some(9), &[Cue::Nuke]), 0.01),
            None
        );
        assert_eq!(director.update(&frame(true, None, &[]), 0.01), None);
        assert_eq!(
            director.update(&frame(true, Some(9), &[]), 0.01),
            Some(Voice::Flight(9))
        );
        assert_eq!(director.update(&frame(false, None, &[]), 0.01), None);
    }

    #[test]
    fn requests_deduplicate_bursts_and_repeated_effects_restart_the_single_voice() {
        let mut requests = Requests::default();
        for _ in 0..10000 {
            for cue in Cue::ALL {
                requests.post(cue);
            }
        }
        assert_eq!(requests.take(), Cue::ALL);
        assert!(requests.take().is_empty());
        let mut director = Director::default();
        let first = director.update(&frame(true, None, &[Cue::ShieldHit]), 0.0);
        let second = director.update(&frame(true, None, &[Cue::ShieldHit]), 0.01);
        assert_ne!(first, second);
        assert!(matches!(
            second,
            Some(Voice::Effect {
                cue: Cue::ShieldHit,
                ..
            })
        ));
    }

    #[test]
    fn export_generated_sound_previews_when_requested() {
        if let Ok(directory) = std::env::var("SCORCHED_SOUND_PREVIEW_DIR") {
            let mut demo = Vec::new();
            for cue in Cue::ALL {
                demo.extend(cue.tones());
                demo.push(Tone { hz: 0, ms: 100 });
            }
            std::fs::write(format!("{directory}/sound-demo.wav"), synthesize(&demo)).unwrap();
            for cue in Cue::ALL {
                std::fs::write(format!("{directory}/{cue:?}.wav"), synthesize(&cue.tones()))
                    .unwrap();
            }
            for bin in [0, FLIGHT_BINS / 2, FLIGHT_BINS - 1] {
                std::fs::write(format!("{directory}/flight-{bin}.wav"), flight_wav(bin)).unwrap();
            }
        }
    }
}
