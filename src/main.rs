#![feature(int_to_from_bytes)]

extern crate mp4parse;

use std::fmt;
use std::fs::{ self, OpenOptions, File, };
use std::io::{ Read, Write, Seek, SeekFrom };


#![feature(int_to_from_bytes)]

extern crate mp4parse;

use std::fmt;
use std::fs::{ self, OpenOptions, File, };
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

        // TODO: 需要检查 `buffer` 中 `Access Unit` 的数据的的排列规则
        //      *   Annex-B byte stream format
        //      *   AVCC format
        output_file.write_all(&mut buffer).unwrap();
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
            // NOTE: 将 MP4 Video Track Samples 转换为 H264 Byte Stream.
            mp4_samples_to_h264(&mut mp4_input_file, &mut h264_output_file, &track.samples);
        }
    }
}