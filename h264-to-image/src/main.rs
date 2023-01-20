///
/// FFMPEG_DIR=/Users/simon/simon/src/poc/ffmpeg/stage cargo run --release
/// 


use anyhow::Result;
mod util;
mod poc_ffmpeg;
pub mod h264_annexb;
pub mod yuv;


fn main() -> Result<()> { 
    let rtype = 3;

    match rtype {
        1 => poc_decode::run(),
        2 => poc_snapshot::run(),
        3 => poc_ffmpeg::run(),
        _ => Ok(())
    }

    // let h264_packets = &include_bytes!("/tmp/sample-data/test.h264")[..];

    // let width = 640;
    // let height = 360;

    // let mut rgb = vec![0; width * height * 3];

    // let mut decoder = Decoder::new()?;

    // let image = decoder.decode(h264_packets)?.ok_or_else(|| Error::msg("Must have image"))?;
    
    // image.write_rgb8(&mut rgb);




    // let mut decoder = Decoder::new()?;

    // let mut decode_output = None;

    // let iter = annexb::AnnexB::new(&h264_packets[..]);
    // for d in iter {

    //     decode_output = decode_nal(&mut decoder, d.annexb(), d.pos())?;
    //     if decode_output.is_some() {
    //         break;
    //     }
    // }

    // match decode_output {
    //     Some(yuv) => { 
    //         println!(
    //             "decoded dimension [{:?}], stride [{:?}]", 
    //             (yuv.dimension_y(), yuv.dimension_u(), yuv.dimension_v(), yuv.dimension_rgb()),
    //             yuv.strides_yuv(),
    //         );

    //         let (width, height) = yuv.dimension_rgb();
    //         let mut rgb = vec![0; width * height * 3];
    //         yuv.write_rgb8(&mut rgb);
    //         let image = RgbImage::from_vec(width as u32, height as u32, rgb)
    //         .with_context(||"fail to convert to RgbImage")?;

    //         let output_file = "/tmp/snapshot.png";
            
    //         image.save(output_file)
    //         .with_context(||format!("fail to save file [{}]", output_file))?;

    //         println!("saved to [{}]", output_file);

    //     },
    //     None => println!("all decoding fail"),
    // }


    // Ok(())
}

mod poc_decode {
    use anyhow::{Result, Context};
    use image::RgbImage;
    use openh264::decoder::Decoder;

    use super::h264_annexb::AnnexB;

    
    pub fn run() -> Result<()> {
        // let h264_packets = &include_bytes!("/tmp/sample-data/test.h264")[..]; 

        let file_bytes = crate::util::read_to_vec("/tmp/sample-data/test.h264")?;
        let h264_packets = &file_bytes;

        let mut decoder = Decoder::new()?;
    
        let iter = AnnexB::new(&h264_packets[..]);
        for data in iter { 
            const START_CODE: [u8; 4] = [0, 0, 0, 1];
            decoder.decode(&START_CODE).with_context(||"decode startcode fail")?;
            let r = decoder.decode(data.unit()).with_context(||"decode unit fail")?;

            // let r = decoder.decode(data.annexb()).with_context(||"h264 decode fail")?;
            if let Some(yuv) = r { 
                
                let (width, height) = yuv.dimension_rgb();
                let mut rgb = vec![0; width * height * 3];
                yuv.write_rgb8(&mut rgb);
                let image = RgbImage::from_vec(width as u32, height as u32, rgb)
                .with_context(||"fail to convert to RgbImage")?;
    
                let output_file = "/tmp/snapshot.png";
                
                image.save(output_file)
                .with_context(||format!("fail to save file [{}]", output_file))?;
    
                println!("saved to [{}]", output_file);

                break;
            }
        }
    
        
        Ok(())
    }
}

mod poc_snapshot {
    use super::h264_annexb;
    use super::h264_snapshot::H264SnapshotIdr;
    use anyhow::{Result, Context};

    pub fn run() -> Result<()> {
        // let h264_packets = &include_bytes!("/tmp/sample-data/test.h264")[..];
        // let h264_packets = &include_bytes!("/tmp/output.h264")[..];
        let file_bytes = crate::util::read_to_vec("/tmp/output.h264")?;
        let h264_packets = &file_bytes;
        let fps = 25;

        let mut snapshot = H264SnapshotIdr::try_new(5000)?;
        let mut output_num = 0_usize;
        let iter = h264_annexb::AnnexB::new(&h264_packets[..]);
        for (n, data) in iter.enumerate() {
            
            {
                let first = data.unit()[0];
                let header = h264_reader::nal::NalHeader::new(first)
                .map_err(|e| anyhow::anyhow!("{:?}", e))
                .with_context(||"invalid nalu header")?;
                println!("process nalu type [{:?}]-[{}]", header.nal_unit_type(), first & 0x1F);
            }

            let ts = 1000 * n as i64 / fps;

            let r = snapshot.try_to_rbg(ts, data)?;
            if let Some(image) = r {

                output_num += 1;
                let output_file = format!("/tmp/snapshot-{}.png", output_num);
                
                image.save(&output_file)
                .with_context(||format!("fail to save file [{}]", output_file))?;

                println!("ts {}, saved to [{}]", ts, output_file);
            }
        }
        if !snapshot.is_ready() {
            println!("no snapshot")
        }

        Ok(())
    }
}

mod h264_snapshot {
    use std::{ops::Range, time::Instant};
    use anyhow::{Result, anyhow, Context};
    use h264_reader::nal::{Nal, UnitType};
    use image::RgbImage;
    use openh264::decoder::Decoder;

    use crate::h264_annexb::NaluData;
    
    /// snaphot for IDR frame only
    pub struct H264SnapshotIdr { 
        snapshot: H264Snapshot,
        interval: VideSegmentInterval,
    }
    
    impl H264SnapshotIdr {
        pub fn try_new(interval: i64) -> Result<Self> {
            Ok(Self {
                snapshot: H264Snapshot::try_new()?,
                interval: VideSegmentInterval::new(interval),
            })
        }
    
        pub fn is_ready(&self) -> bool {
            self.snapshot.is_ready()
        }
    
        pub fn try_to_rbg(&mut self, ts: i64, nalu: NaluData<'_>) -> Result<Option<RgbImage>> { 
            let header = nalu.nalu().header().map_err(|e|anyhow!("{:?}", e))?;
            let is_keyframe = header.nal_unit_type() == UnitType::SliceLayerWithoutPartitioningIdr;
            
            if header.nal_unit_type() == UnitType::AccessUnitDelimiter {
                return Ok(None)
            }

            // if is_keyframe {
            //     println!("ts {}, keyframe", ts);
            // }
            
            if !self.snapshot.is_ready() {
                // println!("feed nalu type [{:?}]", header.nal_unit_type());
                let r = self.snapshot.feed_to_rbg(nalu.annexb())?;
                if r.is_some() {
                    self.interval.input(ts, true);
                }
                return Ok(r)
            } else {
                
                if self.interval.input(ts, is_keyframe) {
                    // println!("feed nalu type [{:?}]", header.nal_unit_type());
                    return self.snapshot.feed_to_rbg(nalu.annexb());
                } else {
                    return Ok(None)
                }
            }
    
        }
    }
    
    pub struct H264Snapshot { 
        decoder: Decoder,
        is_ready: bool,
    }
    
    impl H264Snapshot {
        pub fn try_new() -> Result<Self> {
            Ok(Self {
                decoder: Decoder::new().with_context(||"init h264 decoder fail")?,
                is_ready: false,
            })
        }
    
        pub fn is_ready(&self) -> bool {
            self.is_ready 
        }
    
        pub fn feed_to_rbg(&mut self, annexb: &[u8]) -> Result<Option<RgbImage>> {
            let r = self.decoder.decode(annexb).with_context(||"h264 decode fail")?;
            if let Some(yuv) = r {
                let (width, height) = yuv.dimension_rgb();
                let mut rgb = vec![0; width * height * 3];
                yuv.write_rgb8(&mut rgb);
                let image = RgbImage::from_vec(width as u32, height as u32, rgb)
                .with_context(||"fail to convert to RgbImage")?;
                self.is_ready = true;
                Ok(Some(image))
            } else {
                Ok(None)
            }
        }
    
        // pub fn try_key_frame_to_rbg(&mut self, nalu: NaluData<'_>) -> Result<Option<RgbImage>> { 
            
        //     if !self.is_ready {
        //         return self.feed_to_rbg(nalu.annexb())
        //     } else {
        //         let nal_type = nalu.nalu().header()
        //         .map_err(|e|anyhow!("{:?}", e))?
        //         .nal_unit_type();
        //         if nal_type == UnitType::SliceLayerWithoutPartitioningIdr {
        //             return self.feed_to_rbg(nalu.annexb())
        //         }
        //     }
        //     Ok(None)
        // }
    
        // pub fn prepare<'a, I>(&mut self, iter: &mut I) -> Result<bool> 
        // where
        //     I: Iterator<Item = NaluData<'a>>,
        // {
        //     if self.is_ready() {
        //         return Ok(true)
        //     }
    
        //     while let Some(nalu) = iter.next() { 
        //         let annexb = nalu.annexb();
        //         let nalu = nalu.nalu();
        //         let header = nalu.header().map_err(|e| anyhow!("{:?}", e))?;
        //         self.decoder.decode(annexb).with_context(||"prepare but decode fail")?;
        //     }
        //     Ok(false)
        // }
    }

    #[derive(Default)]
    struct DetectRange<T> {
        range: Option<Range<T>>,
    }
    
    impl<T> DetectRange<T> 
    where
        T: Clone
    {
        pub fn input(&mut self, v: T) {
            match &mut self.range {
                Some(exist) => {
                    exist.start = exist.end.clone();
                    exist.end = v;
                },
                None => self.range = Some(Range{start: v.clone(), end: v}),
            }
        }
    
        pub fn range(&self) -> &Option<Range<T>> {
            &self.range
        }
    }
    
    
    struct VideSegmentInterval {
        gop: DetectRange<i64>, // gop timestamp
        milli: MilliTs,
        interval: i64,
    }
    
    impl VideSegmentInterval {
        pub fn new(interval: i64) -> Self {
            Self {
                gop: DetectRange::default(), 
                milli: MilliTs::default(),
                interval,
            }
        }
    
        pub fn input(&mut self, ts: i64, is_keyframe: bool) -> bool {
            
            if !is_keyframe {
                return false;
            }
    
            self.gop.input(ts);
    
            let gop = self.gop.range().as_ref()
            .map(|x|x.end - x.start)
            .unwrap_or(0)
            .max(0);
            
            if (self.milli.elapsed(ts) + gop) >= self.interval {
                self.milli.set_ts(ts);
                true
            } else {
                false
            }
        }
    }
    
    
    #[derive(Default)]
    struct MilliTs {
        last: Option<TsType>,
    }
    
    impl MilliTs {
        pub fn set_ts(&mut self, ts: i64) {
            self.last = Some(TsType::I64(ts));
        }
    
        pub fn elapsed(&mut self, ts: i64) -> i64 {
            match self.last {
                Some(last) => {
                    match last {
                        TsType::I64(v) => {
                            let elapsed = ts - v;
                            if elapsed >= 0 {
                                elapsed
                            } else {
                                self.last = Some(TsType::Moment(Instant::now()));
                                0
                            }
                        },
                        TsType::Moment(last) => {
                            last.elapsed().as_millis() as i64
                        },
                    }
                },
                None => {
                    self.last = Some(TsType::I64(ts));
                    0
                },
            }
        }
    }
    
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum TsType {
        I64(i64),
        Moment(Instant),
    }

}




