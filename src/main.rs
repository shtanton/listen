extern crate cpal;
extern crate hound;

use std::io::{self, BufRead};
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use std::time::{Instant};

use cpal::traits::{DeviceTrait, EventLoopTrait, HostTrait};


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let path = &args[1];

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .expect("Failed to get default input device");
    let format = device
        .default_input_format()
        .expect("Failed to get default input format");

    let (recording, writer) = record(path, host, device, format)?;

    let start_time = Instant::now();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let response = match line.as_str() {
            "time" => start_time.elapsed().as_millis().to_string(),
            "stop" => break,
            _ => continue,
        };
        println!("{}", response);
    }

    recording.store(false, std::sync::atomic::Ordering::Relaxed);
    writer.lock().unwrap().take().unwrap().finalize()?;
    Ok(())
}

fn record(
    path: &str,
    host: cpal::Host,
    device: cpal::Device,
    format: cpal::Format,
) -> Result<
    (
        Arc<AtomicBool>,
        Arc<Mutex<Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>>>,
    ),
    Box<dyn std::error::Error>,
> {
    let event_loop = host.event_loop();
    let stream_id = event_loop.build_input_stream(&device, &format)?;
    event_loop.play_stream(stream_id)?;

    let spec = wav_spec_from_format(&format);
    let writer = hound::WavWriter::create(path, spec)?;
    let writer = Arc::new(Mutex::new(Some(writer)));

    let recording = Arc::new(AtomicBool::new(true));

    let writer_2 = writer.clone();
    let recording_2 = recording.clone();
    std::thread::spawn(move || {
        event_loop.run(move |id, event| {
            let data = match event {
                Ok(data) => data,
                Err(err) => {
                    eprintln!("an error occurred on stream {:?}: {}", id, err);
                    return;
                }
            };

            // If we're done recording, return early.
            if !recording_2.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            // Otherwise write to the wav writer.
            match data {
                cpal::StreamData::Input {
                    buffer: cpal::UnknownTypeInputBuffer::U16(buffer),
                } => {
                    if let Ok(mut guard) = writer_2.try_lock() {
                        if let Some(writer) = guard.as_mut() {
                            for sample in buffer.iter() {
                                let sample = cpal::Sample::to_i16(sample);
                                writer.write_sample(sample).ok();
                            }
                        }
                    }
                }
                cpal::StreamData::Input {
                    buffer: cpal::UnknownTypeInputBuffer::I16(buffer),
                } => {
                    if let Ok(mut guard) = writer_2.try_lock() {
                        if let Some(writer) = guard.as_mut() {
                            for &sample in buffer.iter() {
                                writer.write_sample(sample).ok();
                            }
                        }
                    }
                }
                cpal::StreamData::Input {
                    buffer: cpal::UnknownTypeInputBuffer::F32(buffer),
                } => {
                    if let Ok(mut guard) = writer_2.try_lock() {
                        if let Some(writer) = guard.as_mut() {
                            for &sample in buffer.iter() {
                                writer.write_sample(sample).ok();
                            }
                        }
                    }
                }
                _ => (),
            }
        });
    });
    Ok((recording, writer))
}

fn sample_format(format: cpal::SampleFormat) -> hound::SampleFormat {
    match format {
        cpal::SampleFormat::U16 => hound::SampleFormat::Int,
        cpal::SampleFormat::I16 => hound::SampleFormat::Int,
        cpal::SampleFormat::F32 => hound::SampleFormat::Float,
    }
}

fn wav_spec_from_format(format: &cpal::Format) -> hound::WavSpec {
    hound::WavSpec {
        channels: format.channels as _,
        sample_rate: format.sample_rate.0 as _,
        bits_per_sample: (format.data_type.sample_size() * 8) as _,
        sample_format: sample_format(format.data_type),
    }
}
