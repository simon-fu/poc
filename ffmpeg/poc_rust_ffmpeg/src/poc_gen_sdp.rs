use ffmpeg_next::{codec::Parameters, format::{self, context::{Input, Output}, input_with_dictionary}, media};
use ffmpeg_sys_next::{avformat_alloc_output_context2, AVFormatContext};
use ffmpeg_next::Error as AvError;

pub fn gen_input_sdp(ctx: &mut Input) -> Result<String, AvError> {
    unsafe {
        gen_sdp(ctx.as_mut_ptr())
    }
}

pub fn gen_output_sdp(ctx: &mut Output) -> Result<String, AvError> {
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
    let sdp = gen_output_sdp(&mut octx)?;
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



unsafe fn gen_sdp(mut ac: *mut AVFormatContext) -> Result<String, AvError> {
    let mut sdp_buffer = vec![0_u8; 8*1024];

    let ret =ffmpeg_sys_next::av_sdp_create(
        &mut ac, 
        1, 
        sdp_buffer.as_mut_ptr() as *mut i8, 
        sdp_buffer.len() as i32
    );

    if ret == 0 {
        // let sdp = String::from_utf8_unchecked(sdp_buffer);
        // Ok(sdp)

        // let sdp_c_str = std::ffi::CString::from_vec_with_nul_unchecked(sdp_buffer);
        // let sdp = sdp_c_str.into_string()
        // .map_err(|_e|AvError::InvalidData)?;
        // Ok(sdp)

        let sdp_c_str = std::ffi::CStr::from_ptr(sdp_buffer.as_mut_ptr() as *mut i8);
        let c_slice = sdp_c_str.to_bytes();
        let c_len = c_slice.len();
        sdp_buffer.set_len(c_len);
        let sdp = String::from_utf8_unchecked(sdp_buffer);
        Ok(sdp)
        
    } else {
        Err(AvError::from(ret))
    }

}

// unsafe fn gen_sdp(mut ac: *mut AVFormatContext) -> Result<String, AvError> {
//     const BUF_SIZE: i32 = 8*1024;

//     let mut buf: [std::ffi::c_char; BUF_SIZE as usize] = [0; BUF_SIZE as usize];
//     let buf_ptr = &mut buf as *mut std::ffi::c_char;
//     let ret = ffmpeg_sys_next::av_sdp_create(
//         &mut ac, 
//         1, 
//         buf_ptr, 
//         BUF_SIZE,
//     );

//     if ret == 0 {
//         let sdp_c_str = std::ffi::CStr::from_ptr(buf_ptr);
//         let sdp = sdp_c_str.to_string_lossy().to_string();
//         Ok(sdp)
//     } else {
//         Err(AvError::from(ret))
//     }
// }

// unsafe fn make_sdp(mut ac: *mut AVFormatContext) -> String {
//     const BUF_SIZE: usize = 8*1024;
//     let mut buf: [std::ffi::c_char; BUF_SIZE as usize] = [0; BUF_SIZE as usize];
//     let buf_ptr = &mut buf as *mut std::ffi::c_char;
//     let mut output_format_context = output.as_ptr();
//     let output_format_context_ptr = &mut output_format_context as *mut *const AVFormatContext;
//     // WARNING! Casting from const ptr to mutable ptr here!
//     let output_format_context_ptr = output_format_context_ptr as *mut *mut AVFormatContext;
//     let ret = av_sdp_create(output_format_context_ptr, 1, buf_ptr, BUF_SIZE);

//     if ret == 0 {
//         let sdp_c_str = std::ffi::CStr::from_ptr(buf_ptr);
//         let sdp = sdp_c_str.to_string_lossy().to_string();
//         Ok(sdp)
//     } else {
//         Err(Error::from(ret))
//     }
// }


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

    let sdp = gen_output_sdp(&mut octx).unwrap();

    println!("sdp({} bytes): {sdp}", sdp.len());
}

#[test]
fn test_gen_sdp_av_only() {

    let file_url = "/tmp/sample-data/sample.mp4";
    
    let (sdp, _octx) = gen_av_only_sdp_from_file(file_url).unwrap();
    println!("sdp({} bytes): {sdp}", sdp.len());
}


#[test]
fn test_gen_sdp_raw() {

    let file_url = "/tmp/sample-data/sample.mp4";
    let mut input = ffmpeg_next::format::input_with_dictionary(&file_url, Default::default()).unwrap();

    let sdp = gen_input_sdp(&mut input).unwrap();
    println!("sdp({} bytes): {sdp}", sdp.len());
}


