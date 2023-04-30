extern crate ffmpeg_next as ffmpeg;

use std::time::Duration;

use ffmpeg::format::{input, Pixel};
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{context::Context, flag::Flags};
use ffmpeg::time::sleep;
use ffmpeg::util::frame::video::Video;
use virtualcam_rs::Camera;

#[derive(Debug)]
struct Error;

impl From<virtualcam_rs::Error> for Error {
    fn from(e: virtualcam_rs::Error) -> Self {
        e.into()
    }
}

impl From<ffmpeg::Error> for Error {
    fn from(e: ffmpeg::Error) -> Self {
        e.into()
    }
}

fn main() -> Result<(), Error> {
    let mut vcam = Camera::new(1920, 1080, "Unity Video Capture")?;

    ffmpeg::init().unwrap();
    loop {
        let start_frame: usize = 100;
        let end_frame: usize = 200;
        if let Ok(mut ictx) = input(&"C:/Users/Administrator/Desktop/CLHL/model/视频模型/003/012")
        {
            ictx.seek(
                (end_frame - start_frame) as i64,
                start_frame as i64..end_frame as i64,
            )?;

            let input = ictx
                .streams()
                .best(Type::Video)
                .ok_or(ffmpeg::Error::StreamNotFound)?;

            let video_stream_index = input.index();

            let context_decoder =
                ffmpeg::codec::context::Context::from_parameters(input.parameters())?;
            let mut decoder = context_decoder.decoder().video()?;

            let mut scaler = Context::get(
                decoder.format(),
                decoder.width(),
                decoder.height(),
                Pixel::RGBA,
                1920,
                1080,
                Flags::BILINEAR,
            )?;

            'outer: for (stream, packet) in ictx.packets() {
                if stream.index() == video_stream_index {
                    decoder.send_packet(&packet)?;
                    let mut decoded = Video::empty();
                    while decoder.receive_frame(&mut decoded).is_ok() {
                        let mut rgb_frame = Video::empty();
                        if decoded.coded_number() >= end_frame {
                            break 'outer;
                        }
                        scaler.run(&decoded, &mut rgb_frame)?;
                        let rgb_u8 = rgb_frame.data(0);
                        match vcam.send(rgb_u8.to_vec()) {
                            Ok(_) => {}
                            Err(e) => {
                                println!("send error: {:?}", e);
                            }
                        };

                        sleep(25000)?;
                    }
                }
            }
            decoder.send_eof()?;
        }
    }
}
