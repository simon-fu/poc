
/// refer from openh264 -> DecodedYUV


pub fn yuv420_to_rgb8(
    width: usize,
    height: usize,
    y_data: &[u8], 
    u_data: &[u8], 
    v_data: &[u8],
    y_stride: usize,
    u_stride: usize,
    v_stride: usize,
    target: &mut [u8]
) {
    let wanted = width * height * 3;

    assert_eq!(
        target.len(),
        wanted as usize,
        "Target RGB8 array does not match image dimensions. Wanted: {} * {} * 3 = {}, got {}",
        width,
        height,
        wanted,
        target.len()
    );

    for y in 0..height {
        for x in 0..width {
            let base_tgt = (y * width + x) * 3;
            let base_y = y * y_stride + x;
            let base_u = (y / 2 * u_stride) + (x / 2);
            let base_v = (y / 2 * v_stride) + (x / 2);

            let rgb_pixel = &mut target[base_tgt..base_tgt + 3];

            let y = y_data[base_y] as f32;
            let u = u_data[base_u] as f32;
            let v = v_data[base_v] as f32;

            rgb_pixel[0] = (y + 1.402 * (v - 128.0)) as u8;
            rgb_pixel[1] = (y - 0.344 * (u - 128.0) - 0.714 * (v - 128.0)) as u8;
            rgb_pixel[2] = (y + 1.772 * (u - 128.0)) as u8;
        }
    }
} 

pub fn yuv420_to_rgba8(
    width: usize,
    height: usize,
    y_data: &[u8], 
    u_data: &[u8], 
    v_data: &[u8],
    y_stride: usize,
    u_stride: usize,
    v_stride: usize,
    target: &mut [u8]
) {

    let wanted = width * height * 4;

    // This needs some love, and better architecture.
    // assert_eq!(self.info.iFormat, videoFormatI420 as i32);
    assert_eq!(
        target.len(),
        wanted as usize,
        "Target RGBA8 array does not match image dimensions. Wanted: {} * {} * 4 = {}, got {}",
        width,
        height,
        wanted,
        target.len()
    );

    for y in 0..height {
        for x in 0..width {
            let base_tgt = (y * width + x) * 4;
            let base_y = y * y_stride + x;
            let base_u = (y / 2 * u_stride) + (x / 2);
            let base_v = (y / 2 * v_stride) + (x / 2);

            let rgb_pixel = &mut target[base_tgt..base_tgt + 4];

            let y = y_data[base_y] as f32;
            let u = u_data[base_u] as f32;
            let v = v_data[base_v] as f32;

            rgb_pixel[0] = (y + 1.402 * (v - 128.0)) as u8;
            rgb_pixel[1] = (y - 0.344 * (u - 128.0) - 0.714 * (v - 128.0)) as u8;
            rgb_pixel[2] = (y + 1.772 * (u - 128.0)) as u8;
            rgb_pixel[3] = 255;
        }
    }
} 
