extern crate ffmpeg_next as ffmpeg;
use std::ops::Range;

use ffmpeg::{codec::{self, Parameters}, encoder, format, media, Rational};

#[test]
fn test_cut_off() {
    // ffmpeg::init().unwrap();
    // log::set_level(log::Level::Warning);

    // let ifile = "/tmp/sample-data/sample.mp4";
    // let ofile = "/tmp/output.mp4";
    // let range = 10_000..20_000;
    // cut_off(ifile, ofile, range.clone());
    // println!("output file [{ofile}], cut off [{range:?}] ms");

    let ifile = "/tmp/output.mp3";
    let ofile = "/tmp/cut.mp3";
    let range = 5_000..30_000;
    cut_off(ifile, ofile, range.clone());
    println!("output file [{ofile}], cut off [{range:?}] ms");
    print_packets(ofile, range.end + 12_000);
}

#[test]
fn test_print_packets() {
    // let ifile = "/tmp/sample-data/sample.mp4";
    // print_packet(ifile, 22_000);

    let ofile = "/tmp/output.wav";
    print_packets(ofile, 22_000);
}

pub fn cut_off (ifile: &str, ofile: &str, range: Range<i64>) {
    
    let mut ictx = ffmpeg_next::format::input(&ifile).unwrap();
    let mut octx = format::output(&ofile).unwrap();

    let mut stream_mapping = vec![0; ictx.nb_streams() as _];
    let mut ist_time_bases = vec![Rational(0, 1); ictx.nb_streams() as _];
    let mut ost_index = 0;
    for (ist_index, ist) in ictx.streams().enumerate() {
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
        let mut ost = octx.add_stream(encoder::find(codec::Id::None)).unwrap();
        ost.set_parameters(ist.parameters());
        // We need to set codec_tag to 0 lest we run into incompatible codec tag
        // issues when muxing into a different container format. Unfortunately
        // there's no high level API to do this (yet).
        unsafe {
            (*ost.parameters().as_mut_ptr()).codec_tag = 0;
        }
    }

    octx.set_metadata(ictx.metadata().to_owned());
    octx.write_header().unwrap();

    for (stream, mut packet) in ictx.packets() {
        let ist_index = stream.index();
        let ost_index = stream_mapping[ist_index];
        if ost_index < 0 {
            continue;
        }

        let time_base = ist_time_bases[ist_index];
        let timestamp = packet.dts().unwrap();
        let milli = (timestamp * time_base.numerator() as i64 * 1000) / time_base.denominator() as i64;

        if milli >= range.start && milli < range.end {
            continue;
        }
        
        let ost = octx.stream(ost_index as _).unwrap();
        packet.rescale_ts(ist_time_bases[ist_index], ost.time_base());
        packet.set_position(-1);
        packet.set_stream(ost_index as _);
        packet.write_interleaved(&mut octx).unwrap();
    }

    octx.write_trailer().unwrap();
}

// 

fn print_packets (file: &str, max_milli: i64) {
    
    let mut ictx = ffmpeg_next::format::input(&file).unwrap();
    let mut ist_time_bases = vec![Rational(0, 1); ictx.nb_streams() as _];
    let mut ist_params = vec![Parameters::default(); ictx.nb_streams() as _];

    for (ist_index, ist) in ictx.streams().enumerate() {
        ist_time_bases[ist_index] = ist.time_base();
        ist_params[ist_index] = ist.parameters();
    }

    for (num, (stream, packet)) in ictx.packets().enumerate() {

        let ist_index = stream.index();
        let time_base = ist_time_bases[ist_index];
        let pts = packet.pts();
        let dts = packet.dts();
        let dts_milli = dts
        .map(|x|(x * time_base.numerator() as i64 * 1000) / time_base.denominator() as i64);

        println!(
            "Packet No.{num}: dts_ms {dts_milli:?}, stream {ist_index}, codec {:?}, pts {pts:?}, dts {dts:?}", 
            ist_params[ist_index].id(),
        );

        if dts_milli.unwrap_or(0) >= max_milli {
            break;
        }
    }
}
