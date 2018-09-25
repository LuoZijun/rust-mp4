#![feature(int_to_from_bytes)]

extern crate mp4parse;

use std::fmt;
use std::fs::{ self, OpenOptions, };
use std::io::{ Read, Write, Seek, SeekFrom };

// A "Frame" called a "Access Unit" (or AU) in h.264 contains 1 more more NALU.
// The trun encodes each AUs size, this includes all NALUs for that AU.
// NALUs do not have timestamps, AUs do.
// One sample is one frame, which is one or more nalu.
// One MP4 Sample = One AU(Access Unit) = one or more nalu
// 
// 简单来讲，H.264 流由多个 `NAL-units` 组成一个 `Access Unit(AU)`。
// 多个 `Access Unit` 再组成一个 `Coded video sequence`，也就是我们所说的 `H.264 Stream`。
// 
// H.264 Stream Format
// 
// *    Annex-B Byte Stream
// *    AVCC
// 
// H264_AnnexB: 0x00_00_01 OR 0x00_00_00_01 | NALU_LENGTH( 4 Bytes) | NALU_HEADER( 1 Bytes) | NALU_PAYLOAD( .. Bytes)
// H264_AVCC:   NALU_PACKET_SIZE( 4 Bytes)  | NALU_LENGTH( 4 Bytes) | NALU_HEADER( 1 Bytes) | NALU_PAYLOAD( .. Bytes)
// 
// 总的来说H264的码流的打包方式有两种,一种为 annex-b byte stream format 的格式，
// 这个是绝大部分编码器的默认输出格式，就是每个帧的开头的3~4个字节是H264的start_code,0x00000001或者0x000001。
// 另一种是原始的NAL打包格式，就是开始的若干字节（1，2，4字节）是NAL的长度，而不是start_code,此时必须借助某个全局的数据来获得编 码器的profile,level,PPS,SPS等信息才可以解码。

struct Chunks<'a> {
    track: &'a mp4parse::Track,
    stsc_sample_index: usize,
    index: usize,
    sample_index: usize,
}

impl<'a> Iterator for Chunks<'a> {
    type Item = Samples<'a>;

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
                            println!("[WARN] 部分样本遭到遗弃 ...", );
                            return None;
                        }
                    }
                }
                
                Some(Samples {
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

#[derive(Debug)]
pub struct Sample {
    pub chunk_index: usize,
    pub chunk_offset: u64,
    pub index: usize,
    pub offset: u64,
    pub size: u32,
    pub delta: u32, // 样本显示时间
}

struct Samples<'a> {
    track: &'a mp4parse::Track,
    
    chunk_index: usize,
    chunk_offset: u64,

    sample_offset: u64,
    sample_count: usize,
    sample_index: usize,
}


impl<'a> Iterator for Samples<'a> {
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




pub struct Mp4Track {
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
    pub tracks: Vec<Mp4Track>,
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
        
        for track in &self.tracks {
            writeln!(f, "{:?}", track);
        }

        Ok(())
    }
}



fn parse<F: Read>(mut input_file: F) -> Result<Mp4File, mp4parse::Error> {
    let mut mp4_media_ctx = mp4parse::MediaContext::new();
    mp4parse::read_mp4(&mut input_file, &mut mp4_media_ctx).unwrap();

    let mut tracks = vec![];
    for track in mp4_media_ctx.tracks {
        // let chunks_count = match track.stco {
        //     Some(ref stco) => stco.offsets.len(),
        //     None => 0,
        // };
        // let sample_count = match track.stsz {
        //     Some(ref stsz) => stsz.sample_sizes.len(),
        //     None => 0,
        // };
        let mut samples = vec![];
        {
            let chunks = Chunks { track: &track, index: 0, stsc_sample_index: 0, sample_index: 0 };
            for chunk_samples in chunks {
                for sample in chunk_samples {
                    samples.push(sample);
                }
            }
        }

        // println!("stss: {:?}", track.stss);
        // println!("ctts: {:?}", track.ctts);
        
        tracks.push(Mp4Track {
            id: track.id,
            track_id: track.track_id.unwrap(),
            kind: track.track_type,
            empty_duration: track.empty_duration,
            media_time: track.media_time,
            timescale: track.timescale,
            duration: track.duration,
            codec_type: track.codec_type,
            data: track.data.unwrap(),
            samples: samples,
        });
    }

    Ok(Mp4File {
        timescale: mp4_media_ctx.timescale,
        mvex: mp4_media_ctx.mvex,
        psshs: mp4_media_ctx.psshs,
        tracks: tracks,
    })
}



fn mp4_samples_to_h264<F: Read + Write + Seek>(mut input_file: F, mut output_file: F, samples: &[Sample]) {
    let mut buffer = Vec::new();

    for video_sample in samples.iter() {
        buffer.resize(video_sample.size as usize, 0u8);

        input_file.seek(SeekFrom::Start(video_sample.offset)).unwrap();
        input_file.read_exact(&mut buffer).unwrap();

        let mut start: usize = 0usize;
        loop {
            if start >= buffer.len() {
                break;
            }

            let size = u32::from_be_bytes([
                buffer[start + 0], buffer[start + 1],
                buffer[start + 2], buffer[start + 3]
            ]) as usize;

            start += 4;
            let end = start + size;

            let nalu = &buffer[start..end];
            
            if nalu[size-2] == 0 && nalu[size-1] == 0 {
                output_file.write(&[0u8, 0, 1]).unwrap();
                output_file.write_all(&nalu);
                output_file.write(&[3]).unwrap();
            } else {
                output_file.write(&[0u8, 0, 0, 1]).unwrap();
                output_file.write_all(&nalu);
            }

            start = end;
        }
    }
}

// AVCC format extradata
#[derive(Debug)]
struct AVCVideoConfigurationRecord {
    version: u8,
    profile: u8,
    compatibility: u8,
    level: u8,
    // indicates the length in bytes of the length field in an AVC video access unit used indicate the length of each NAL unit. 
    length_size_minus_one: u8,
    sps: Vec<Vec<u8>>,
    pps: Vec<Vec<u8>>,
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


fn main() {
    let input_filepath = "a.mp4";
    let output_filepath = "a.h264";

    let mut mp4_input_file = fs::File::open(input_filepath).unwrap();
    let mut h264_output_file = {
        let _ = fs::remove_file(output_filepath);
        OpenOptions::new().create_new(true).write(true).open(output_filepath).unwrap()
    };

    let mp4 = parse(&mut mp4_input_file).unwrap();

    print!("{:?}", mp4);

    for track in &mp4.tracks {
        if track.kind == mp4parse::TrackType::Video {
            // extradata (sequence header)
            // https://stackoverflow.com/questions/24884827/possible-locations-for-sequence-picture-parameter-sets-for-h-264-stream/24890903#24890903
            let (width, height, extradata) = match track.data {
                mp4parse::SampleEntry::Video(ref video_sample_entry) => {
                    let width = video_sample_entry.width as usize;
                    let height = video_sample_entry.height as usize;
                    let extradata = match video_sample_entry.codec_specific {
                        mp4parse::VideoCodecSpecific::AVCConfig(ref data) => data,
                        _ => unreachable!(),
                    };
                    (width, height, extradata)
                },
                _ => unreachable!()
            };
            println!("Video: {} x {}", width, height);

            let avc_config_record = parse_avc_config(&extradata);
            println!("extradata: {:?}", avc_config_record);

            for sps_nalu in avc_config_record.sps {
                let size = sps_nalu.len();
                if sps_nalu[size-2] == 0 && sps_nalu[size-1] == 0 {
                    h264_output_file.write(&[0u8, 0, 1]).unwrap();
                    h264_output_file.write_all(&sps_nalu).unwrap();
                    h264_output_file.write(&[3]).unwrap();
                } else {
                    h264_output_file.write(&[0u8, 0, 0, 1]).unwrap();
                    h264_output_file.write_all(&sps_nalu).unwrap();
                }
            }
            for pps_nalu in avc_config_record.pps {
                let size = pps_nalu.len();
                if pps_nalu[size-2] == 0 && pps_nalu[size-1] == 0 {
                    h264_output_file.write(&[0u8, 0, 1]).unwrap();
                    h264_output_file.write_all(&pps_nalu).unwrap();
                    h264_output_file.write(&[3]).unwrap();
                } else {
                    h264_output_file.write(&[0u8, 0, 0, 1]).unwrap();
                    h264_output_file.write_all(&pps_nalu).unwrap();
                }
            }

            // NOTE: 将 MP4 Video Track Samples 转换为 H264 Byte Stream.
            mp4_samples_to_h264(&mut mp4_input_file, &mut h264_output_file, &track.samples);
        }
    }
}