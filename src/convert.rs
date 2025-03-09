use ffmpeg_next::{Rational, codec, encoder, format, media};
use std::path::Path;

pub fn to_aiff(input_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the ffmpeg library
    ffmpeg_next::init()?;

    let output_path = input_path.with_extension("aiff");

    let mut input_context = format::input(input_path).expect("Input file opened");
    let mut output_context = format::output(&output_path).expect("Output file opened");

    let mut stream_mapping = vec![0; input_context.nb_streams() as _];
    let mut ist_time_bases = vec![Rational(0, 1); input_context.nb_streams() as _];
    let mut ost_index = 0;

    for (ist_index, ist) in input_context.streams().enumerate() {
        let ist_medium = ist.parameters().medium();
        if ist_medium != media::Type::Audio
            && ist_medium != media::Type::Video
            && ist_medium != media::Type::Subtitle
        {
            stream_mapping[ist_index] = -1;
            continue;
        }

        stream_mapping[ist_index] = ost_index;
        ist_time_bases[ist_index] = ist.time_base();
        ost_index += 1;
        let mut ost = output_context
            .add_stream(encoder::find(codec::Id::None))
            .unwrap();
        ost.set_parameters(ist.parameters());
        // We need to set codec_tag to 0 lest we run into incompatible codec tag
        // issues when muxing into a different container format. Unfortunately
        // there's no high level API to do this (yet).
        unsafe {
            (*ost.parameters().as_mut_ptr()).codec_tag = 0;
        }
    }

    output_context.set_metadata(input_context.metadata().to_owned());
    output_context.write_header().unwrap();

    for (stream, mut packet) in input_context.packets() {
        let ist_index = stream.index();
        let ost_index = stream_mapping[ist_index];
        if ost_index < 0 {
            continue;
        }
        let ost = output_context.stream(ost_index as _).unwrap();
        packet.rescale_ts(ist_time_bases[ist_index], ost.time_base());
        packet.set_position(-1);
        packet.set_stream(ost_index as _);
        packet.write_interleaved(&mut output_context).unwrap();
    }

    output_context.write_trailer().unwrap();

    Ok(())
}
