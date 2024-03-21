use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::CODEC_TYPE_NULL;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;

pub fn decode_file(file_path: &Path) -> (u32, usize, Vec<f32>) {
    let file = Box::new(std::fs::File::open(file_path).unwrap());
    let mut hint = Hint::new();
    hint.with_extension(file_path.extension().unwrap().to_str().unwrap());

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            MediaSourceStream::new(file, Default::default()),
            &Default::default(),
            &Default::default(),
        )
        .expect("unsupported format");

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .expect("no supported audio tracks");

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &Default::default())
        .expect("unsupported codec");
    let track_id = track.id;
    let mut channels = 2;
    let mut sample_rate = 0;
    let mut samples = vec![];
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            _ => break,
        };
        while !format.metadata().is_latest() {
            format.metadata().pop();
        }

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                channels = decoded.spec().channels.count();
                sample_rate = decoded.spec().rate;
                let mut byte_buf =
                    SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
                byte_buf.copy_interleaved_ref(decoded);
                samples.append(&mut byte_buf.samples_mut().to_vec());
                continue;
            }
            _ => {
                // Handling any error as track skip
                continue;
            }
        }
    }
    (sample_rate, channels, samples)
}
