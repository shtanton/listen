extern crate cpal;
extern crate futures;
extern crate hound;
extern crate iced;
extern crate iced_native;
extern crate iced_wgpu;

mod volume;

use std::io::{self, BufRead};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use cpal::{
    traits::{DeviceTrait, EventLoopTrait, HostTrait},
    Sample,
};

use iced::futures::{
    channel::mpsc::{channel, Receiver},
    executor::ThreadPool,
    future,
    stream::{BoxStream, StreamExt},
};

use iced::{executor, Application, Column, Command, Element, Length, Settings, Subscription, Text};

use volume::Volume;

#[derive(Debug)]
enum Message {
    Sample(f32),
}

struct App {
    volume: f32,
    host: Arc<cpal::Host>,
    device: Arc<cpal::Device>,
    format: Arc<cpal::Format>,
    recording: Arc<AtomicBool>,
}

impl Application for App {
    type Executor = executor::Default;
    type Message = Message;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (App, Command<Self::Message>) {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .expect("Failed to get default input device");
        let format = device
            .default_input_format()
            .expect("Failed to get default input format");

        (
            App {
                volume: 0.,
                host: Arc::new(host),
                device: Arc::new(device),
                format: Arc::new(format),
                recording: Arc::new(AtomicBool::new(true)),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        "Recorder".to_string()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::Sample(sample) => {
                self.volume = sample;
                Command::none()
            }
        }
    }

    fn view(&mut self) -> Element<Self::Message> {
        Volume::new(self.volume).into()
    }

    fn subscription(&self) -> Subscription<Message> {
        audio_subscription(
            self.host.clone(),
            self.device.clone(),
            self.format.clone(),
            self.recording.clone(),
        )
        .map(Message::Sample)
    }
}

pub fn main() {
    App::run(Settings::default());
}

fn audio_subscription(
    host: Arc<cpal::Host>,
    device: Arc<cpal::Device>,
    format: Arc<cpal::Format>,
    recording: Arc<AtomicBool>,
) -> iced::Subscription<f32> {
    iced::Subscription::from_recipe(AudioIn {
        host,
        device,
        format,
        recording,
    })
}

struct AudioIn {
    host: Arc<cpal::Host>,
    device: Arc<cpal::Device>,
    format: Arc<cpal::Format>,
    recording: Arc<AtomicBool>,
}

impl<H, I> iced_native::subscription::Recipe<H, I> for AudioIn
where
    H: std::hash::Hasher,
{
    type Output = f32;

    fn hash(&self, state: &mut H) {
        use std::hash::Hash;
        std::any::TypeId::of::<AudioIn>().hash(state);
    }

    fn stream(self: Box<Self>, _input: BoxStream<'static, I>) -> BoxStream<'static, Self::Output> {
        Box::pin(
            record(
                &self.host,
                &self.device,
                &self.format,
                self.recording.clone(),
            )
            .unwrap()
            .scan((Instant::now(), 0.), |(started, largest), sample| {
                let so_far = sample.max(*largest);
                future::ready(if started.elapsed() > Duration::from_millis(33) {
                    *started += Duration::from_millis(33);
                    *largest = 0.;
                    Some((true, so_far))
                } else {
                    *largest = so_far;
                    Some((false, so_far))
                })
            })
            .filter_map(|(important, v)| {
                future::ready(if important { Some(v.min(1.)) } else { None })
            }),
        )
    }
}

pub fn do_record() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let path = &args[1];

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .expect("Failed to get default input device");
    let format = device
        .default_input_format()
        .expect("Failed to get default input format");

    let recording = Arc::new(AtomicBool::new(true));
    let samples = record(&host, &device, &format, recording.clone())?;

    let mut writer = hound::WavWriter::create(path, wav_spec_from_format(&format))?;

    let recording_task = async {
        samples
            .for_each(|sample| {
                writer.write_sample(sample).ok();
                if sample > 0. {
                    println!("Sample: {:#<1$}", "", (sample * 20.) as usize);
                }
                async {}
            })
            .await;
        writer.finalize().unwrap();
    };

    let pool = ThreadPool::new().unwrap();
    pool.spawn_ok(recording_task);

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
    recording.store(false, Ordering::Relaxed);

    Ok(())
}

fn record(
    host: &cpal::Host,
    device: &cpal::Device,
    format: &cpal::Format,
    recording: Arc<AtomicBool>,
) -> Result<Receiver<f32>, Box<dyn std::error::Error>> {
    let event_loop = host.event_loop();
    let stream_id = event_loop.build_input_stream(&device, format)?;
    event_loop.play_stream(stream_id)?;

    let (mut sender, receiver) = channel(1024);

    std::thread::spawn(move || {
        event_loop.run(move |id, event| {
            let data = match event {
                Ok(data) => data,
                Err(err) => {
                    eprintln!("an error occurred on stream {:?}: {}", id, err);
                    return;
                }
            };

            if !recording.load(Ordering::Relaxed) {
                sender.close_channel();
                return;
            }

            match data {
                cpal::StreamData::Input {
                    buffer: cpal::UnknownTypeInputBuffer::U16(buffer),
                } => {
                    for sample in buffer.iter() {
                        sender.try_send(sample.to_f32()).unwrap();
                    }
                }
                cpal::StreamData::Input {
                    buffer: cpal::UnknownTypeInputBuffer::I16(buffer),
                } => {
                    for &sample in buffer.iter() {
                        sender.try_send(sample.to_f32()).unwrap();
                    }
                }
                cpal::StreamData::Input {
                    buffer: cpal::UnknownTypeInputBuffer::F32(buffer),
                } => {
                    for &sample in buffer.iter() {
                        sender.try_send(sample).unwrap();
                    }
                }
                _ => (),
            }
        });
    });
    Ok(receiver)
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
