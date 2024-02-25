use ffmpeg_next::{codec::Parameters, format::{self, context::{Input, Output}, input_with_dictionary}, media};
use ffmpeg_sys_next::{avformat_alloc_output_context2, AVFormatContext};
use ffmpeg_next::Error as AvError;

pub fn gen_input_sdp(ctx: &mut Input) -> String {
    unsafe {
        gen_sdp(ctx.as_mut_ptr())
    }
}

pub fn gen_output_sdp(ctx: &mut Output) -> String {
    unsafe {
        gen_sdp(ctx.as_mut_ptr())
    }
}

pub fn gen_av_only_sdp_from_file(file_url: &str) -> Result<(String, Output), AvError> {
    let ictx = input_with_dictionary(&file_url, Default::default())?;
    gen_av_only_sdp(&ictx)
}

pub fn gen_av_only_sdp(ictx: &Input) -> Result<(String, Output), AvError> {
    let format = ictx.format();
    let format = format_first(&format)?;
    // let format = "mp4";

    gen_av_only_sdp_with_format(ictx, format)
}

pub fn gen_av_only_sdp_with_format(ictx: &Input, format: &str) -> Result<(String, Output), AvError> {
    let mut octx = output_raw(format)?;
    copy_av_only_tracks(ictx, &mut octx)?;
    let sdp = gen_output_sdp(&mut octx);
    Ok((sdp, octx))
}

pub fn copy_av_only_tracks(ictx: &Input, octx: &mut Output) -> Result<(), AvError> {

    for ist in ictx.streams() {
        let param = ist.parameters();
        match param.medium() {
            media::Type::Video 
            | media::Type::Audio => {
                output_add_stream(octx, param)?;
            }
            _ => {}
        }
    }

    Ok(())
}

pub fn format_first(format: &format::Input) -> Result<&str, AvError> {
    let format = format.name();
    let format = format.split(',')
        .next()
        .ok_or_else(||AvError::Bug)?
        .trim();
    Ok(format)
}


fn output_raw(format: &str) -> Result<Output, AvError> {
    unsafe {
        let mut output_ptr = std::ptr::null_mut();
        let format = std::ffi::CString::new(format).unwrap();
        match avformat_alloc_output_context2(
            &mut output_ptr,
            std::ptr::null_mut(),
            format.as_ptr(),
            std::ptr::null(),
        ) {
            0 => Ok(Output::wrap(output_ptr)),
            e => Err(AvError::from(e)),
        }
    }
}

fn output_add_stream(
    output: &mut Output, 
    codec_parameters: Parameters,
) -> Result<usize, AvError> {

    let mut writer_stream = output
    .add_stream(ffmpeg_next::encoder::find(codec_parameters.id()))?;

    writer_stream.set_parameters(codec_parameters);

    Ok(writer_stream.index())
}



unsafe fn gen_sdp(mut ac: *mut AVFormatContext) -> String {
    let mut sdp_buffer = vec![0_u8; 8*1024];

    ffmpeg_sys_next::av_sdp_create(
        &mut ac, 
        1, 
        sdp_buffer.as_mut_ptr() as *mut i8, 
        sdp_buffer.len() as i32
    );

    String::from_utf8_unchecked(sdp_buffer)
}

#[test]
fn test_gen_sdp_medium() {

    let file_url = "/tmp/sample-data/sample.mp4";
    let medium = media::Type::Audio;

    let mut ictx = input_with_dictionary(&file_url, Default::default()).unwrap();
    let ictx = &mut ictx;

    let format = ictx.format();
    let format = format_first(&format).unwrap();

    let mut octx = output_raw(format).unwrap();

    for ist in ictx.streams() {
        let param = ist.parameters();
        if param.medium() == medium {
            output_add_stream(&mut octx, param).unwrap();
        }
    }

    let sdp = gen_output_sdp(&mut octx);

    println!("{sdp}");
}

#[test]
fn test_gen_sdp_av_only() {

    let file_url = "/tmp/sample-data/sample.mp4";
    
    let (sdp, _octx) = gen_av_only_sdp_from_file(file_url).unwrap();
    println!("{sdp}");
}


#[test]
fn test_gen_sdp_raw() {

    let file_url = "/tmp/sample-data/sample.mp4";
    let mut input = ffmpeg_next::format::input_with_dictionary(&file_url, Default::default()).unwrap();

    println!("{}", gen_input_sdp(&mut input));
}


