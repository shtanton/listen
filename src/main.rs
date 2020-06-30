extern crate cpal;
extern crate futures;
extern crate hound;
extern crate iced;
extern crate iced_native;
extern crate iced_wgpu;

mod rpc;
mod time;
mod volume;

use std::sync::Arc;
use std::time::{Duration, Instant};

use cpal::{
    traits::{DeviceTrait, EventLoopTrait, HostTrait},
    Sample,
};

use iced::futures::{
    channel::mpsc::{channel, Receiver},
    stream::BoxStream,
};

use iced::{
    button, executor, Align, Application, Button, Column, Command, Element, Length, Settings,
    Subscription, Text,
};

use rpc::Rpc;
use volume::Volume;

#[derive(Debug, Clone)]
enum Message {
    Buffer(Vec<f32>),
    NextRecordStatus,
    UpdateVolume,
    RpcMessage(rpc::Receive),
}

enum RecordStatus {
    NotStarted,
    Recording(Instant, hound::WavWriter<std::io::BufWriter<std::fs::File>>),
    Finished,
}

struct App {
    volume: f32,
    max_volume: f32,
    host: Arc<cpal::Host>,
    device: Arc<cpal::Device>,
    format: Arc<cpal::Format>,
    button: button::State,
    recording: RecordStatus,
    path: String,
    rpc: Rpc,
}

impl Application for App {
    type Executor = executor::Default;
    type Message = Message;
    type Flags = String;

    fn new(flags: Self::Flags) -> (App, Command<Self::Message>) {
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
                max_volume: 0.,
                host: Arc::new(host),
                device: Arc::new(device),
                format: Arc::new(format),
                button: button::State::new(),
                recording: RecordStatus::NotStarted,
                path: flags,
                rpc: Rpc::new(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        "Recorder".to_string()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::Buffer(buffer) => {
                for sample in buffer.into_iter() {
                    self.max_volume = self.max_volume.max(sample);
                    if let RecordStatus::Recording(_, writer) = &mut self.recording {
                        writer.write_sample(sample).ok();
                    }
                }
                Command::none()
            }
            Message::NextRecordStatus => {
                use RecordStatus::*;
                match &self.recording {
                    NotStarted => {
                        self.recording = Recording(
                            Instant::now(),
                            hound::WavWriter::create(
                                &self.path,
                                wav_spec_from_format(&self.format),
                            )
                            .unwrap(),
                        );
                    }
                    Recording(_, _) => {
                        if let Recording(_, writer) =
                            std::mem::replace(&mut self.recording, Finished)
                        {
                            writer.finalize().unwrap();
                        }
                    }
                    Finished => {
                        self.recording = Finished;
                    }
                };
                Command::none()
            }
            Message::UpdateVolume => {
                self.volume = self.max_volume;
                self.max_volume = 0.;
                Command::none()
            }
            Message::RpcMessage(rpc::Receive::Time) => {
                if let RecordStatus::Recording(started, _) = self.recording {
                    self.rpc.send(started.elapsed().as_millis());
                } else {
                    self.rpc.send("null");
                }
                Command::none()
            }
        }
    }

    fn view(&mut self) -> Element<Self::Message> {
        let col = Column::new().align_items(Align::Center).width(Length::Fill).spacing(30).padding(30);
        match self.recording {
            RecordStatus::Finished => col.push(Button::new(
                &mut self.button,
                Text::new("Finished Recording"),
            )),
            RecordStatus::Recording(start_time, _) => col
                .push(
                    Button::new(&mut self.button, Text::new("Stop Recording"))
                        .on_press(Message::NextRecordStatus),
                )
                .push(Text::new(display_duration(start_time.elapsed()))),
            RecordStatus::NotStarted => col.push(
                Button::new(&mut self.button, Text::new("Record"))
                    .on_press(Message::NextRecordStatus),
            ),
        }
        .push(Volume::new(self.volume).width(Length::Units(200)))
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            audio_subscription(self.host.clone(), self.device.clone(), self.format.clone())
                .map(Message::Buffer),
            time::every(Duration::from_millis(33)).map(|()| Message::UpdateVolume),
            self.rpc.receive().map(Message::RpcMessage),
        ])
    }
}

pub fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args[1].clone();

    App::run(Settings::with_flags(path));
}

pub fn display_duration<'a>(time: Duration) -> String {
    let seconds = time.as_secs();
    let sub_seconds = seconds % 60;
    let minutes = seconds / 60;
    let sub_minutes = minutes % 60;
    let hours = minutes / 60;
    return format!("{:02}:{:02}:{:02}", hours, sub_minutes, sub_seconds);
}

fn audio_subscription(
    host: Arc<cpal::Host>,
    device: Arc<cpal::Device>,
    format: Arc<cpal::Format>,
) -> iced::Subscription<Vec<f32>> {
    iced::Subscription::from_recipe(AudioIn {
        host,
        device,
        format,
    })
}

struct AudioIn {
    host: Arc<cpal::Host>,
    device: Arc<cpal::Device>,
    format: Arc<cpal::Format>,
}

impl<H, I> iced_native::subscription::Recipe<H, I> for AudioIn
where
    H: std::hash::Hasher,
{
    type Output = Vec<f32>;

    fn hash(&self, state: &mut H) {
        use std::hash::Hash;
        std::any::TypeId::of::<AudioIn>().hash(state);
    }

    fn stream(self: Box<Self>, _input: BoxStream<'static, I>) -> BoxStream<'static, Self::Output> {
        Box::pin(record(&self.host, &self.device, &self.format).unwrap())
    }
}

fn record(
    host: &cpal::Host,
    device: &cpal::Device,
    format: &cpal::Format,
) -> Result<Receiver<Vec<f32>>, Box<dyn std::error::Error>> {
    let event_loop = host.event_loop();
    let stream_id = event_loop.build_input_stream(&device, format)?;
    event_loop.play_stream(stream_id)?;

    let (mut sender, receiver) = channel(256);

    std::thread::spawn(move || {
        event_loop.run(move |id, event| {
            let data = match event {
                Ok(data) => data,
                Err(err) => {
                    eprintln!("an error occurred on stream {:?}: {}", id, err);
                    return;
                }
            };

            if sender.is_closed() {
                return;
            }

            match data {
                cpal::StreamData::Input {
                    buffer: cpal::UnknownTypeInputBuffer::U16(buffer),
                } => {
                    sender
                        .try_send(buffer.iter().map(|s| s.to_f32()).collect())
                        .ok();
                }
                cpal::StreamData::Input {
                    buffer: cpal::UnknownTypeInputBuffer::I16(buffer),
                } => {
                    sender
                        .try_send(buffer.iter().map(|s| s.to_f32()).collect())
                        .ok();
                }
                cpal::StreamData::Input {
                    buffer: cpal::UnknownTypeInputBuffer::F32(buffer),
                } => {
                    sender.try_send(buffer.iter().map(|s| *s).collect()).ok();
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
