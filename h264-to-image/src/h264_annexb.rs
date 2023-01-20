use std::ops::Range;
// use anyhow::Result;
use h264_reader::nal::RefNal;

pub struct AnnexB<'a>{
    data: &'a [u8],
    last: Option<Range<usize>>,
}

impl<'a> AnnexB<'a> {
    pub fn new(d: &'a [u8]) -> Self {
        Self { data: d, last: None }
    }
}

impl<'a> Iterator for AnnexB<'a> {
    type Item = NaluData<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match &self.last {
            Some(last) => {
                let r = search_startcode2(self.data, &last);
                if let Some(v) = r {
                    let item = NaluData{ pos: last.len(), data: &self.data[last.start..v.start]};
                    self.last = Some(v);
                    return Some(item);
                }
            },
            None => {
                let r = search_startcode(self.data);
                if let Some(v) = r {
                    self.last = Some(v);
                        return self.next()
                }
            },
        }
        self.last = Some(Range { start: self.data.len(), end: self.data.len() });
        None
    }
}

#[derive(Clone, Copy)]
pub struct NaluData<'a> {
    pos: usize,
    data: &'a [u8],
}

impl <'a> NaluData<'a> {
    /// nalu data with startcode
    pub fn annexb(&'a self) -> &'a [u8] {
        self.data
    }

    /// nalu data offset of annexb
    pub fn pos(&'a self) -> usize {
        self.pos
    }

    /// nalu data without startcode
    pub fn unit(&'a self) -> &'a [u8] {
        &self.data[self.pos..]
        
    }

    pub fn nalu(&'a self) -> RefNal<'a> { 
        RefNal::new(self.unit(), &[], true)
    }
}

pub fn search_startcode2(data: &[u8], last: &Range<usize>) -> Option<Range<usize>> {
    search_startcode(&data[last.end..])
    .map(|v|Range{ start: v.start + last.end , end: v.end + last.end })
}

pub fn search_startcode(data: &[u8]) -> Option<Range<usize>> {
    let mut i = 2_usize;
    while i < data.len() {
        if 0x01 == data[i] && 0x00 == data[i - 1] && 0x00 == data[i - 2] {
            let mut range = Range{ start: i-2, end: i+1 };
            while range.start > 0 {
                if data[range.start] != 0x00 {
                    break;
                }
                range.start -= 1;
            }

            if data[range.start] != 0x00 {
                range.start += 1;
            }

            return Some(range);
        }
        i += 1;
    } 
    None
}
