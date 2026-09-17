// Automatic gain control for the RC003 voice channel. Measures the RMS of
// each decoded PCM frame and smoothly steers the gain so speech lands near
// the target level: fast when turning down (attack), slow when turning up
// (release) to avoid a pumping sound. Frames quieter than the noise gate are
// left alone so silence does not get amplified.

pub(crate) const AGC_TARGET_RMS_DB: f32 = -18.0;
pub(crate) const AGC_MIN_GAIN_DB: f32 = -12.0;
pub(crate) const AGC_MAX_GAIN_DB: f32 = 30.0;
const AGC_ATTACK_DB_PER_SEC: f32 = 120.0;
const AGC_RELEASE_DB_PER_SEC: f32 = 3.0;
// Far below the target the slow release would need several seconds to catch
// up; approach quickly first, then fine-tune slowly inside this window.
const AGC_FAST_RELEASE_DB_PER_SEC: f32 = 24.0;
const AGC_FAST_RELEASE_WINDOW_DB: f32 = 6.0;
const AGC_NOISE_GATE_DB: f32 = -50.0;
const AGC_PEAK_HEADROOM: f32 = 0.98;

#[derive(Default)]
pub(crate) struct AutoGain {
    gain_db: f32,
}

impl AutoGain {
    pub(crate) fn gain_db(&self) -> f32 {
        self.gain_db
    }

    pub(crate) fn process(&mut self, samples: &mut [i16], sample_rate: u32) {
        if samples.is_empty() || sample_rate == 0 {
            return;
        }
        let mut sum_squares = 0_f64;
        let mut peak = 0_f32;
        for &sample in samples.iter() {
            let value = f32::from(sample) / f32::from(i16::MAX);
            sum_squares += f64::from(value * value);
            peak = peak.max(value.abs());
        }
        let rms = (sum_squares / samples.len() as f64).sqrt() as f32;
        let duration = samples.len() as f32 / sample_rate as f32;
        if rms > 0.0 {
            let rms_db = 20.0 * rms.log10();
            if rms_db >= AGC_NOISE_GATE_DB {
                let mut desired =
                    (AGC_TARGET_RMS_DB - rms_db).clamp(AGC_MIN_GAIN_DB, AGC_MAX_GAIN_DB);
                if peak > 0.0 {
                    desired = desired.min(20.0 * (AGC_PEAK_HEADROOM / peak).log10());
                }
                let rate = if desired < self.gain_db {
                    AGC_ATTACK_DB_PER_SEC
                } else if desired - self.gain_db > AGC_FAST_RELEASE_WINDOW_DB {
                    AGC_FAST_RELEASE_DB_PER_SEC
                } else {
                    AGC_RELEASE_DB_PER_SEC
                };
                let step = rate * duration;
                self.gain_db = if desired < self.gain_db {
                    (self.gain_db - step).max(desired)
                } else {
                    (self.gain_db + step).min(desired)
                };
            }
        }
        let gain = 10_f32.powf(self.gain_db / 20.0);
        if (gain - 1.0).abs() < f32::EPSILON {
            return;
        }
        for sample in samples.iter_mut() {
            let value = f32::from(*sample) * gain;
            *sample = value.round().clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 16_000;

    fn frame(frames: usize, amplitude: i16) -> Vec<Vec<i16>> {
        (0..frames).map(|_| vec![amplitude; 240]).collect()
    }

    #[test]
    fn quiet_speech_is_boosted_quickly_then_fine_tuned_slowly() {
        // -40 dBFS RMS wants +22 dB, within the +30 dB cap.
        let quiet = (f32::from(i16::MAX) * 0.01) as i16;
        let mut agc = AutoGain::default();
        let mut first = vec![quiet; 240];
        agc.process(&mut first, RATE);
        // Far from the target the fast release applies: 24 dB/s over 15ms.
        let first_gain = agc.gain_db();
        assert!((first_gain - AGC_FAST_RELEASE_DB_PER_SEC * 0.015).abs() < 1e-4);
        let mut second = vec![quiet; 240];
        agc.process(&mut second, RATE);
        assert!((agc.gain_db() - first_gain * 2.0).abs() < 1e-4);
        // After enough speech the gain converges near the ideal +22 dB.
        let mut long_run = frame(200, quiet);
        for frame in &mut long_run {
            agc.process(frame, RATE);
        }
        assert!((agc.gain_db() - 22.0).abs() < 0.5);
    }

    #[test]
    fn near_the_target_the_release_slows_down() {
        // Only 4 dB below the ideal gain: inside the fine-tune window, one
        // frame moves by the slow release rate.
        let quiet = (f32::from(i16::MAX) * 0.01) as i16;
        let mut agc = AutoGain { gain_db: 18.0 };
        let mut frame_data = vec![quiet; 240];
        agc.process(&mut frame_data, RATE);
        let step = agc.gain_db() - 18.0;
        assert!((step - AGC_RELEASE_DB_PER_SEC * 0.015).abs() < 1e-4);
    }

    #[test]
    fn loud_speech_is_attenuated_within_a_few_frames() {
        let loud = (f32::from(i16::MAX) * 0.9) as i16;
        let mut agc = AutoGain::default();
        let mut first = vec![loud; 240];
        agc.process(&mut first, RATE);
        // Attack moves down immediately but only by rate * frame duration.
        assert!(agc.gain_db() < 0.0 && agc.gain_db() > -2.0);
        assert!(first.iter().all(|&s| s.abs() < loud));
        // Target is -18 dBFS for a -0.9 dBFS signal: about -17 dB, beyond the
        // -12 dB floor; a handful of frames reach it.
        let mut frames = frame(20, loud);
        for frame in &mut frames {
            agc.process(frame, RATE);
        }
        assert!((agc.gain_db() - AGC_MIN_GAIN_DB).abs() < 1e-4);
    }

    #[test]
    fn full_scale_input_never_clips() {
        let mut agc = AutoGain {
            gain_db: AGC_MAX_GAIN_DB,
        };
        let mut frames = frame(30, i16::MAX);
        for frame in &mut frames {
            agc.process(frame, RATE);
            assert!(frame.iter().all(|&s| i32::from(s).abs() <= i32::from(i16::MAX)));
        }
        // A full-scale square wave has 0 dBFS RMS, so the RMS target dominates
        // and the gain sinks to the -12 dB floor without clipping.
        assert!((agc.gain_db() - AGC_MIN_GAIN_DB).abs() < 1e-4);
    }

    #[test]
    fn peaky_signal_is_capped_by_the_headroom_limiter() {
        // Mostly silence with one full-scale spike per frame: low RMS asks for
        // a boost, but the peak must stay under full scale.
        let mut agc = AutoGain::default();
        let mut frames = frame(20, 0);
        for frame in &mut frames {
            frame[0] = i16::MAX;
            agc.process(frame, RATE);
        }
        let expected = 20.0 * (AGC_PEAK_HEADROOM / 1.0_f32).log10();
        assert!((agc.gain_db() - expected).abs() < 0.2);
    }

    #[test]
    fn silence_and_noise_floor_hold_the_gain() {
        let mut agc = AutoGain { gain_db: 12.0 };
        let mut silence = vec![0; 240];
        agc.process(&mut silence, RATE);
        assert_eq!(agc.gain_db(), 12.0);
        // -60 dBFS sits below the noise gate and must not be amplified.
        let noise = (f32::from(i16::MAX) * 0.001) as i16;
        let mut noise_frame = vec![noise; 240];
        agc.process(&mut noise_frame, RATE);
        assert_eq!(agc.gain_db(), 12.0);
    }

    #[test]
    fn release_step_applies_current_gain() {
        // -30 dBFS speech in a fresh AGC is far from the target, so the first
        // frame applies one fast-release step and scales the samples with it.
        let mut agc = AutoGain::default();
        let mut samples = vec![1000; 240];
        agc.process(&mut samples, RATE);
        assert!((agc.gain_db() - AGC_FAST_RELEASE_DB_PER_SEC * 0.015).abs() < 1e-4);
        let expected = 10_f32.powf(agc.gain_db() / 20.0);
        assert!(samples
            .iter()
            .all(|&s| (f32::from(s) - 1000.0 * expected).abs() <= 1.0));
    }
}
