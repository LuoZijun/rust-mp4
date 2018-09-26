
use mp4parse;
pub use mp4parse::VPxConfigBox;

use super::nalu::{Nalu, NaluKind};
use super::track::{ VideoTrack, H264VideoTrack, VideoCodec, };

use std::ptr;
use std::fmt;
use std::fs::{ self, OpenOptions, };
use std::io::{ Read, Write, Seek, SeekFrom };


pub struct Chunks<'a> {
    track: &'a mp4parse::Track,
    stsc_sample_index: usize,
    index: usize,
    sample_index: usize,
}

pub struct ChunkSamples<'a> {
    track: &'a mp4parse::Track,
    
    chunk_index: usize,
    chunk_offset: u64,

    sample_offset: u64,
    sample_count: usize,
    sample_index: usize,
}

#[derive(Debug)]
pub struct Sample {
    pub chunk_index: usize,
    pub chunk_offset: u64,
    pub index: usize,
    pub offset: u64,
    pub size: u32,
    pub delta: u32, // 样本显示时间
}

impl<'a> Iterator for Chunks<'a> {
    type Item = ChunkSamples<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.track.stco {
            Some(ref chunk_offset_box) => {
                let index = self.index;
                
                if index >= chunk_offset_box.offsets.len() {
                    return None;
                }

                let offset = chunk_offset_box.offsets[index];
                
                let samples_per_chunk = match self.track.stsc {
                    Some(ref sample_to_chunk) => {
                        let chunk_idx = (self.index + 1) as u32;

                        for (i, ref stc) in sample_to_chunk.samples[self.stsc_sample_index..]
                                                .iter().enumerate() {
                            if stc.first_chunk == chunk_idx {
                                self.stsc_sample_index = i;
                            }
                        }

                        sample_to_chunk.samples[self.stsc_sample_index].samples_per_chunk as usize
                    },
                    None => panic!("Sample to Chunk 至少需要包含一条指示信息。"),
                };

                assert_eq!(samples_per_chunk > 0, true);

                let sample_index = self.sample_index;

                self.index += 1;
                self.sample_index += samples_per_chunk;

                if let Some(ref stsz) = self.track.stsz {
                    if stsz.sample_size == 0 {
                        if self.sample_index >= stsz.sample_sizes.len() {
                            warn!("部分样本遭到遗弃 ...", );
                            return None;
                        }
                    }
                }
                
                Some(ChunkSamples {
                    track: &self.track,
                    chunk_index: index,
                    chunk_offset: offset,
                    sample_offset: offset,
                    sample_count: samples_per_chunk,
                    sample_index: sample_index,
                })
            },
            None => None,
        }
    }

    fn count(self) -> usize {
        match self.track.stco {
            Some(ref chunk_offset_box) => chunk_offset_box.offsets.len(),
            None => 0,
        }
    }
}

impl<'a> Iterator for ChunkSamples<'a> {
    type Item = Sample;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_count == 0 {
            return None;
        }

        // stsz
        let sample_size = if let Some(ref stsz) = self.track.stsz {
            if stsz.sample_size > 0 {
                stsz.sample_size
            } else { 
                stsz.sample_sizes[self.sample_index]
            }
        } else {
            0
        };

        if sample_size == 0 {
            panic!("sample size 必须大于零!");
        }

        // stts 同步表
        let delta = if let Some(ref stts) = self.track.stts {
            let mut i = 0u32;
            let mut delta = 0u32;
            for item in stts.samples.iter() {
                i += item.sample_count;

                if i >= self.sample_index as u32 {
                    delta = item.sample_delta;
                }
            }

            delta
        } else {
            panic!("时间同步表缺失！");
        };

        // TODO: stss, ctts

        let ret = Some(Sample {
                chunk_index: self.chunk_index,
                chunk_offset: self.chunk_offset,
                index: self.sample_index,
                offset: self.sample_offset,
                size: sample_size,
                delta: delta,
        });

        self.sample_offset += sample_size as u64;
        self.sample_count -= 1;
        self.sample_index += 1;

        ret
    }
}


struct Mp4Track {
    pub id: usize,
    pub kind: mp4parse::TrackType,
    pub track_id: u32,
    pub empty_duration: Option<mp4parse::MediaScaledTime>,
    pub media_time: Option<mp4parse::TrackScaledTime<u64>>,
    pub timescale: Option<mp4parse::TrackTimeScale<u64>>,
    pub duration: Option<mp4parse::TrackScaledTime<u64>>,
    pub codec_type: mp4parse::CodecType,
    pub data: mp4parse::SampleEntry,
    pub samples: Vec<Sample>,
}

pub struct Mp4File {
    pub timescale: Option<mp4parse::MediaTimeScale>,
    pub mvex: Option<mp4parse::MovieExtendsBox>,
    pub psshs: Vec<mp4parse::ProtectionSystemSpecificHeaderBox>,
    // pub tracks: Vec<Mp4Track>,
    pub video_tracks: Vec<Box<VideoTrack>>,
    // pub audio_tracks: Vec<Box<AudioTrack>>,
}

impl fmt::Debug for Mp4Track {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "\tTrack: #{}", self.track_id);
        writeln!(f, "\t\tKind: {:?} ", self.kind);
        writeln!(f, "\t\tEmpty duration: {:?} ", self.empty_duration);
        writeln!(f, "\t\tMedia Time: {:?} ", self.media_time);
        writeln!(f, "\t\tTimescale: {:?} ", self.timescale);
        writeln!(f, "\t\tDuration: {:?} ", self.duration);
        writeln!(f, "\t\tCodec: {:?} ", self.codec_type);
        writeln!(f, "\t\tData: {:?} ", self.data);
        writeln!(f, "\t\tSamples: {:?} ", self.samples.len());
        Ok(())
    }
}

impl fmt::Debug for Mp4File {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Timescale: {:?} ", self.timescale);
        writeln!(f, "Mvex: {:?} ", self.mvex);
        writeln!(f, "Psshs: {:?} ", self.psshs);
        writeln!(f, "Tracks:");
        
        for track in &self.video_tracks {
            writeln!(f, "{}", track);
        }

        Ok(())
    }
}

// AVCC format extradata
#[derive(Debug)]
pub struct AVCVideoConfigurationRecord {
    pub version: u8,
    pub profile: u8,
    pub compatibility: u8,
    pub level: u8,
    // indicates the length in bytes of the length field in an AVC video access unit used indicate the length of each NAL unit. 
    pub length_size_minus_one: u8,
    pub sps: Vec<Vec<u8>>,
    pub pps: Vec<Vec<u8>>,
}


fn parse_avc_config(data: &[u8]) -> AVCVideoConfigurationRecord {
    let version = data[0];
    let avc_profile = data[1];
    let avc_compatibility = data[2];
    let avc_level = data[3];
    let NALULengthSizeMinusOne = data[4] & 0b00000011;
    let number_of_SPS_NALUs = data[5] & 0b00011111;
    let mut i: usize = 6;

    let sps_elems = (0..number_of_SPS_NALUs)
        .map(|_|{
            let sps_size = u16::from_be_bytes([data[i], data[i+1]]) as usize;
            i += 2;
            let sps: Vec<u8> = data[i..i+sps_size].to_vec();
            i += sps_size;
            sps
        })
        .collect::<Vec<Vec<u8>>>();

    let number_of_PPS_NALUs = data[i];
    i += 1;

    let pps_elems = (0..number_of_PPS_NALUs)
        .map(|_|{
            let pps_size = u16::from_be_bytes([data[i], data[i+1]]) as usize;
            i += 2;
            let sps: Vec<u8> = data[i..i+pps_size].to_vec();
            i += pps_size;
            sps
        })
        .collect::<Vec<Vec<u8>>>();

    assert_eq!(version, 1);

    AVCVideoConfigurationRecord {
        version,
        profile: avc_profile,
        compatibility: avc_compatibility,
        level: avc_level,
        length_size_minus_one: NALULengthSizeMinusOne,
        sps: sps_elems,
        pps: pps_elems,
    }
}

pub fn parse<F: Read>(mut input_file: F) -> Result<Mp4File, mp4parse::Error> {
    let mut mp4_media_ctx = mp4parse::MediaContext::new();
    mp4parse::read_mp4(&mut input_file, &mut mp4_media_ctx).unwrap();

    let mut video_tracks: Vec<Box<VideoTrack>> = vec![];

    for track in mp4_media_ctx.tracks {
        // println!("stss: {:?}", track.stss);
        // println!("ctts: {:?}", track.ctts);

        if track.codec_type != mp4parse::CodecType::H264 {
            warn!("暂不支持该类型的媒体资源: {:?}", track.codec_type);
            continue;
        }

        let mut samples = vec![];
        {
            let chunks = Chunks { track: &track, index: 0, stsc_sample_index: 0, sample_index: 0 };
            for chunk_samples in chunks {
                for sample in chunk_samples {
                    samples.push(sample);
                }
            }
        }

        // extradata (sequence header)
        // https://stackoverflow.com/questions/24884827/possible-locations-for-sequence-picture-parameter-sets-for-h-264-stream/24890903#24890903
        let (width, height, extradata) = match track.data {
            Some(mp4parse::SampleEntry::Video(ref video_sample_entry)) => {
                let width = video_sample_entry.width as u32;
                let height = video_sample_entry.height as u32;
                let extradata = match video_sample_entry.codec_specific {
                    mp4parse::VideoCodecSpecific::AVCConfig(ref data) => data,
                    _ => unreachable!(),
                };
                (width, height, extradata)
            },
            _ => unreachable!()
        };

        let avc_config_record = parse_avc_config(&extradata);
        
        video_tracks.push(Box::new(H264VideoTrack {
            id: track.track_id.unwrap(),
            width: width,
            height: height,
            samples: samples,
            avc_config_record: avc_config_record,
        }));

        // tracks.push(Mp4Track {
        //     id: track.id,
        //     track_id: track.track_id.unwrap(),
        //     kind: track.track_type,
        //     empty_duration: track.empty_duration,
        //     media_time: track.media_time,
        //     timescale: track.timescale,
        //     duration: track.duration,
        //     codec_type: track.codec_type,
        //     data: track.data.unwrap(),
        //     samples: samples,
        // });
    }

    Ok(Mp4File {
        timescale: mp4_media_ctx.timescale,
        mvex: mp4_media_ctx.mvex,
        psshs: mp4_media_ctx.psshs,
        video_tracks: video_tracks,
    })
}


pub struct Nalus< 'b, F: Read + Write + Seek> {
    reader: F,
    sample: &'b Sample,
    readed: usize,
}

impl< 'b, F: Read + Write + Seek> Iterator for Nalus< 'b, F> {
    type Item = Nalu;

    fn next(&mut self) -> Option<Self::Item> {
        if self.readed >= self.sample.size as usize {
            return None;
        }

        let mut size_buffer = [0u8; 4];
        self.reader.read_exact(&mut size_buffer).unwrap();
        let size = u32::from_be_bytes(size_buffer) as usize;

        self.readed += 4;

        let mut buffer: Vec<u8> = vec![0u8; size];
        self.reader.read_exact(&mut buffer).unwrap();

        self.readed += size;

        Some(Nalu::new(buffer))
    }
}

impl Sample {
    pub fn nalus< 'b, F: Read + Write + Seek>(&'b self, mut reader: F) -> Nalus< 'b, F> {
        reader.seek(SeekFrom::Start(self.offset)).unwrap();

        Nalus {
            reader: reader,
            sample: &self,
            readed: 0usize,
        }
    }
}
