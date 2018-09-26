
use super::nalu::Nalu;
use super::parse::Sample;
use super::parse::AVCVideoConfigurationRecord;
use super::parse::VPxConfigBox;

use std::fmt;


#[derive(Debug)]
pub enum AudioCodec {
    AAC,
    Opus,
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Copy, Clone)]
pub enum VideoCodec {
    H264,
    VP8,
    VP9,
    VP10,
    // AV1,
}

pub trait VideoTrack: fmt::Display + fmt::Debug {
    fn codec(&self) -> VideoCodec;
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn samples(&self) -> &[Sample];
    // extradata
    fn avc_config_record(&self) -> Option<&AVCVideoConfigurationRecord>;
    fn vpx_config_box(&self) -> Option<&VPxConfigBox>;
}

#[derive(Debug)]
pub struct H264VideoTrack {
    pub(crate) id: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) samples: Vec<Sample>,
    pub(crate) avc_config_record: AVCVideoConfigurationRecord,
}

impl VideoTrack for H264VideoTrack {
    fn codec(&self) -> VideoCodec {
        VideoCodec::H264
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn samples(&self) -> &[Sample] {
        &self.samples
    }

    fn avc_config_record(&self) -> Option<&AVCVideoConfigurationRecord> {
        Some(&self.avc_config_record)
    }

    fn vpx_config_box(&self) -> Option<&VPxConfigBox> {
        None
    }
}

impl fmt::Display for H264VideoTrack {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "VideoTrack: Codec: {:10} Width: {:8} Height: {:8} Samples Count: {}",
            format!("{:?}", self.codec()),
            format!("{:?}", self.width()),
            format!("{:?}", self.height()),
            self.samples.len(),
        )
    }
}