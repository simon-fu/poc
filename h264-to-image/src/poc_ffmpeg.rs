
use std::{fs::File, io::Write};

use anyhow::{Result, Context, bail};
use ffmpeg_next as ffmpeg;
use ffmpeg::{
    format::Pixel, frame, Packet, 
    ffi::*, 
};
use h264_reader::nal::UnitType;
use image::RgbImage;

use crate::{h264_annexb::{self, NaluData}, yuv};

pub fn run() -> Result<()> {
    ffmpeg::init()
    .with_context(||"init ffmpeg fail")?;

    let rtype = 1;
    match rtype {
        1 => decode_to_yuv_file(),
        2 => decode_to_yuv_file2(),
        3 => decode_to_image_file(),
        _ => Ok(())
    }    
}

fn decode_to_yuv_file() -> Result<()> {
    // let h264_packets = &include_bytes!("/tmp/h264_1920x1080.h264")[..];
    let codec_id = AVCodecID::AV_CODEC_ID_HEVC;
    let file_bytes = crate::util::read_to_vec("/tmp/sample-data/sampleh265.h265")?;

    // let codec_id = AVCodecID::AV_CODEC_ID_H264;
    // let file_bytes = crate::util::read_to_vec("/tmp/h264_1920x1080.h264")?;

    let h264_packets = &file_bytes;
    // let fps = 25;
    let output_filename = "/tmp/output.yuv";
    

    let decoder = unsafe {
        
        let codec = avcodec_find_decoder(codec_id);
        if codec.is_null() {
            bail!("could NOT found codec [{:?}]", codec_id)
        }

        let codec_ctx = avcodec_alloc_context3(codec);
        if codec_ctx.is_null() {
            bail!("could NOT allocate context for codec [{:?}]", codec_id)
        }

        ffmpeg::codec::context::Context::wrap(codec_ctx, None)
        .decoder()
    };

    let mut decoder = decoder.video()
    .with_context(||format!("fail to open as video codec for [{:?}]", codec_id))?;

    println!("opened video decoder [{:?}]", codec_id);

    println!("decoder resolution [{}x{}]", decoder.width(), decoder.height());

    let mut ofile = File::create(output_filename)
    .with_context(||format!("fail to create file [{}]", output_filename))?;

    let iter = h264_annexb::AnnexB::new(&h264_packets[..]);
    for (n, data) in iter.take(20).enumerate() { 
        
        {
            let first = data.unit()[0];
            let header = h264_reader::nal::NalHeader::new(first)
            .map_err(|e| anyhow::anyhow!("{:?}", e))
            .with_context(||"invalid nalu header")?;
            // println!("process nalu type [{:?}]-[{}]", header.nal_unit_type(), first & 0x1F);
            println!("- No.{} nalu, type = [{}]-[{:?}], len [{}]", n, first & 0x1F, header.nal_unit_type(), data.unit().len());
        }

        // let packet = Packet::copy(data.annexb());
        let packet = Packet::borrow(data.annexb());

        let r = decoder.send_packet(&packet);
        if r.is_err() {
            continue;
        }

        // decoder.send_packet(&packet)
        // .with_context(||"send packet to decoder fail")?;
        println!("sent packet to decoder");

        let mut frame = frame::Video::empty();
        while decoder.receive_frame(&mut frame).is_ok() {
            println!(
                "got decoded frame [{}x{}], ts [{:?}], planes [{}], format [{:?}]", 
                frame.width(), frame.height(), frame.timestamp(), frame.planes(), frame.format()
            );
            
            // write yuv to file
            for i in 0..frame.planes() {
                println!("  plane[{}]: rect [{}x{}], len [{}]", i, frame.plane_width(i), frame.plane_height(i), frame.data(i).len());
                ofile.write_all(frame.data(i))
                .with_context(||"write file fail")?;
            }
        }
    }

    Ok(())
}

/// decode with delimiter
fn decode_to_yuv_file2() -> Result<()> {
    // let h264_packets = &include_bytes!("/tmp/h264_1920x1080.h264")[..];
    let file_bytes = crate::util::read_to_vec("/tmp/h264_1920x1080.h264")?;
    let h264_packets = &file_bytes;
    // let fps = 25;
    let output_filename = "/tmp/output.yuv";
    let codec_id = AVCodecID::AV_CODEC_ID_H264;

    let decoder = unsafe {
        
        let codec = avcodec_find_decoder(codec_id);
        if codec.is_null() {
            bail!("could NOT found codec [{:?}]", codec_id)
        }

        let codec_ctx = avcodec_alloc_context3(codec);
        if codec_ctx.is_null() {
            bail!("could NOT allocate context for codec [{:?}]", codec_id)
        }

        ffmpeg::codec::context::Context::wrap(codec_ctx, None)
        .decoder()
    };

    let mut decoder = decoder.video()
    .with_context(||format!("fail to open as video codec for [{:?}]", codec_id))?;

    println!("opened video decoder [{:?}]", codec_id);

    println!("decoder resolution [{}x{}]", decoder.width(), decoder.height());

    let mut ofile = File::create(output_filename)
    .with_context(||format!("fail to create file [{}]", output_filename))?;

    let mut access_unit: Vec<u8> = Vec::new();

    let iter = h264_annexb::AnnexB::new(&h264_packets[..]);
    for (n, data) in iter.take(20).enumerate() { 
        
        let nalu_type = {
            let first = data.unit()[0];
            let header = h264_reader::nal::NalHeader::new(first)
            .map_err(|e| anyhow::anyhow!("{:?}", e))
            .with_context(||"invalid nalu header")?;
            // println!("process nalu type [{:?}]-[{}]", header.nal_unit_type(), first & 0x1F);
            println!("- No.{} nalu, type = [{}]-[{:?}], len [{}]", n, first & 0x1F, header.nal_unit_type(), data.unit().len());
            header.nal_unit_type()
        };

        if nalu_type == UnitType::AccessUnitDelimiter {
            if access_unit.is_empty() {
                access_unit.extend_from_slice(data.annexb());
                continue;
            }
        } else {
            access_unit.extend_from_slice(data.annexb());
            continue;
        }

        let r = {
            let packet = Packet::borrow(&access_unit[..]);
            decoder.send_packet(&packet)
        };
        
        access_unit.clear();
        access_unit.extend_from_slice(data.annexb());

        if let Err(e) = r {
            println!("send packet to decoder error [{:?}]", e);
            continue;
        }

        println!("sent packet to decoder");

        let mut frame = frame::Video::empty();
        while decoder.receive_frame(&mut frame).is_ok() {
            println!(
                "got decoded frame [{}x{}], ts [{:?}], planes [{}], format [{:?}]", 
                frame.width(), frame.height(), frame.timestamp(), frame.planes(), frame.format()
            );
            
            // write yuv to file
            for i in 0..frame.planes() {
                println!("  plane[{}]: rect [{}x{}], len [{}]", i, frame.plane_width(i), frame.plane_height(i), frame.data(i).len());
                ofile.write_all(frame.data(i))
                .with_context(||"write file fail")?;
            }
        }
    }

    Ok(())
}

// fn read_a_file() -> std::io::Result<Vec<u8>> {
//     let mut file = try!(File::open("example.data"));

//     let mut data = Vec::new();
//     try!(file.read_to_end(&mut data));

//     return Ok(data);
// }


fn decode_to_image_file() -> Result<()> { 

    // let h264_packets = &include_bytes!("/tmp/h264_1920x1080.h264")[..];
    // let fps = 25;

    // let codec_id = AVCodecID::AV_CODEC_ID_H264;
    // // let filename = "/tmp/h264_1920x1080.h264";
    // let filename = "/tmp/hebei.h264";

    let codec_id = AVCodecID::AV_CODEC_ID_HEVC;
    let filename = "/tmp/sample-data/sampleh265.h265";

    let file_bytes = crate::util::read_to_vec(filename)?;
    println!("loaded file [{}]", filename);
    let h264_packets = &file_bytes;
 
    

    let output_file_base= "/tmp/snapshot-";
    let output_file_ext = "jpeg";


    let stype = 1;
    let mut snapshot : Box<dyn FeedToRgb> = match stype {
        1 => Box::new(FFSnapshot1::try_new(codec_id)?),
        2 => Box::new(FFSnapshot2::try_new(codec_id)?),
        _ => bail!("never reach here")
    };

    let mut output_num = 0_usize;

    let iter = h264_annexb::AnnexB::new(&h264_packets[..]);
    for (n, data) in iter.take(120).enumerate() { 
        
        {
            let first = data.unit()[0];
            let header = h264_reader::nal::NalHeader::new(first)
            .map_err(|e| anyhow::anyhow!("{:?}", e))
            .with_context(||"invalid nalu header")?;

            println!("- No.{} nalu, type = [{}]-[{:?}], len [{}]", n, first & 0x1F, header.nal_unit_type(), data.unit().len());
        }

        let r = snapshot.feed_to_rgb(data)?;

        if let Some(image) = r {

            output_num += 1;
            let output_file = format!("{}{}.{}", output_file_base, output_num, output_file_ext);
            
            image.save(&output_file)
            .with_context(||format!("fail to save file [{}]", output_file))?;

            println!("  saved to [{}]", output_file);
        }
    }

    Ok(())
}

pub struct FFSnapshot1 {
    decoder: ffmpeg::decoder::Video,
    data: Vec<u8>,
    sps_pps: SpsPps,
    next_keyframe: bool,
}

impl FFSnapshot1 {
    pub fn try_new(codec_id: AVCodecID) -> Result<Self> {
        Ok(Self {
            decoder: make_video_decoder(codec_id)?,
            data: Vec::new(),
            sps_pps: SpsPps::default(),
            next_keyframe: false,
        })
    }

    fn has_sps_pps(&self) -> bool {
        self.data.len() > 0
    }

    fn try_merge_sps_pps(&mut self) {
        if self.data.is_empty() && self.sps_pps.is_complete() {
            println!("merged sps/pps");
            self.sps_pps.merge_to(&mut self.data);
        }
    }
}

impl FeedToRgb for FFSnapshot1 {
    fn feed_to_rgb(&mut self, data: NaluData) -> Result<Option<RgbImage>>  {
        if data.unit().is_empty() {
            return Ok(None)
        }

        let header = {
            let first = data.unit()[0];
            h264_reader::nal::NalHeader::new(first)
            .map_err(|e| anyhow::anyhow!("{:?}", e))
            .with_context(||"invalid nalu header")?
        };

        match header.nal_unit_type() {
            UnitType::SliceLayerWithoutPartitioningNonIdr 
            | UnitType::SliceDataPartitionALayer 
            | UnitType::SliceDataPartitionBLayer 
            | UnitType::SliceDataPartitionCLayer => {
                println!("  P frame 111");
                if self.has_sps_pps() && !self.next_keyframe {
                    println!("  P frame 222");
                    let packet = Packet::borrow(data.annexb());
                    let _r = self.decoder.send_packet(&packet);
                }
            },
            UnitType::SliceLayerWithoutPartitioningIdr => {
                println!("  Idr frame 111");
                if self.has_sps_pps() { 
                    println!("  Idr frame 222");
                    let idr = IdrGuard::new(&mut self.data, data.annexb());
                    let packet = Packet::borrow(idr.data());
                    let r = self.decoder.send_packet(&packet);
                    if r.is_ok() {
                        println!("  Idr frame 333");
                        // let _r = self.decoder.send_packet(&Packet::empty());
                        self.next_keyframe = false;
                    }
                }
            },
            
            UnitType::SeqParameterSet => {
                if self.sps_pps.input_sps(data.annexb()) {
                    if !self.sps_pps.is_complete() {
                        self.data.clear();
                        self.next_keyframe = true;
                    }
                    self.try_merge_sps_pps();
                }
            },
            UnitType::PicParameterSet => {
                if self.sps_pps.input_pps(data.annexb()) {
                    if !self.sps_pps.is_complete() {
                        self.data.clear();
                        self.next_keyframe = true;
                    }
                    self.try_merge_sps_pps();
                }
            },
            _ => {}
        }
        
        return try_recv_frame(&mut self.decoder);
    }
}


struct IdrGuard<'a> {
    data: &'a mut Vec<u8>,
    basic_len: usize,
}

impl<'a> IdrGuard<'a> { 
    pub fn new(data: &'a mut Vec<u8>, idr: &[u8]) -> Self {
        let basic_len = data.len();
        data.extend_from_slice(idr);
        IdrGuard { data, basic_len }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl Drop for IdrGuard<'_> {
    fn drop(&mut self) {
        self.data.truncate(self.basic_len);
    }
}

pub struct FFSnapshot2 {
    sps_pps: SpsPps,
    codec_id: AVCodecID,
}

impl FFSnapshot2 {
    pub fn try_new(codec_id: AVCodecID) -> Result<Self> {

        Ok(Self {
            sps_pps: SpsPps::default(),
            codec_id,
        })
    }
    
}

impl FeedToRgb for FFSnapshot2 {
    fn feed_to_rgb(&mut self, data: NaluData) -> Result<Option<RgbImage>>  {
        if data.unit().is_empty() {
            return Ok(None)
        }

        let header = {
            let first = data.unit()[0];
            h264_reader::nal::NalHeader::new(first)
            .map_err(|e| anyhow::anyhow!("{:?}", e))
            .with_context(||"invalid nalu header")?
        };

        match header.nal_unit_type() {
            
            UnitType::SliceLayerWithoutPartitioningIdr => {
                if self.sps_pps.is_complete() { 
                    let mut input = Vec::with_capacity(self.sps_pps.len() + data.annexb().len());

                    self.sps_pps.merge_to(&mut input);
                    input.extend_from_slice(data.annexb());

                    let mut decoder = make_video_decoder(self.codec_id)?;
                    let packet = Packet::borrow(&input);
                    decoder.send_packet(&packet)?;
                    let _r = decoder.send_packet(&Packet::empty());
                    return try_recv_frame(&mut decoder);
                }
            },

            
            UnitType::SeqParameterSet => { 
                self.sps_pps.input_sps(data.annexb());
            },
            UnitType::PicParameterSet => {
                self.sps_pps.input_pps(data.annexb());
            },
            _ => {}
        }
        
        return Ok(None)
    }
}

pub trait  FeedToRgb {
    fn feed_to_rgb(&mut self, data: NaluData) -> Result<Option<RgbImage>> ;
}

fn try_recv_frame(decoder: &mut ffmpeg::decoder::Video) -> Result<Option<RgbImage>> { 

    let mut last_image = None;
    {
        let mut frame = frame::Video::empty();
        while decoder.receive_frame(&mut frame).is_ok() { 
            // println!("  sreceived frame [{:?}]", frame.format());
            if frame.format() == Pixel::YUV420P {
                let width = frame.width() as usize;
                let height = frame.height() as usize;
                let mut rgb = vec![0; width * height * 3];
    
                yuv::yuv420_to_rgb8(
                    width, height, 
                    frame.data(0),
                    frame.data(1),
                    frame.data(2),
                    frame.stride(0), 
                    frame.stride(1), 
                    frame.stride(2), 
                    &mut rgb
                );
    
                let image = RgbImage::from_vec(width as u32, height as u32, rgb)
                .with_context(||"fail to convert to RgbImage")?;
    
                last_image = Some(image);
            }
        }
    }
    
    return Ok(last_image);
    
}

fn make_video_decoder(codec_id: AVCodecID) -> Result<ffmpeg::decoder::Video> {
    let decoder = unsafe {
        
        let codec = avcodec_find_decoder(codec_id);
        if codec.is_null() {
            bail!("could NOT found codec [{:?}]", codec_id)
        }

        let codec_ctx = avcodec_alloc_context3(codec);
        if codec_ctx.is_null() {
            bail!("could NOT allocate context for codec [{:?}]", codec_id)
        }

        ffmpeg::codec::context::Context::wrap(codec_ctx, None)
        .decoder()
    };

    decoder.video()
    .with_context(||format!("fail to open as video codec for [{:?}]", codec_id))
}


#[derive(Default)]
pub struct SpsPps {
    sps: Vec<u8>,
    pps: Vec<u8>,
}

impl SpsPps {
    pub fn is_complete(&self) -> bool {
        self.sps.len() > 0 && self.pps.len() > 0
    }

    pub fn len(&self) -> usize {
        self.sps.len() + self.pps.len()
    }

    pub fn merge_to(&self, data: &mut Vec<u8>) {
        data.extend_from_slice(&self.sps);
        data.extend_from_slice(&self.pps);
    }

    pub fn input_sps(&mut self, annexb: &[u8]) -> bool {
        if annexb != self.sps {
            if !self.sps.is_empty() {
                self.pps.clear();
            } 
            self.sps = annexb.to_vec();
            true
        } else {
            false
        }
    }

    pub fn input_pps(&mut self, annexb: &[u8]) -> bool {
        if annexb != self.pps {
            if !self.pps.is_empty() {
                self.sps.clear();
            } 
            self.pps = annexb.to_vec();
            true
        } else {
            false
        }
    }
}
