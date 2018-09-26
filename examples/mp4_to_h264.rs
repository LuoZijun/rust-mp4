extern crate mp4;

use mp4::track::H264VideoTrack;
use mp4::track::VideoCodec;
use mp4::nalu::Nalu;

use std::fs::{ self, OpenOptions, };
use std::io::{ Read, Write, Seek, SeekFrom };


fn main() {
    let infile = "a.mp4";
    let outfile = "b.h264";

    let mut mp4_input_file = fs::File::open(infile).unwrap();    
    let mut h264_output_file = {
        let _ = fs::remove_file(outfile);
        OpenOptions::new().create_new(true).write(true).open(outfile).unwrap()
    };

    let mp4 = mp4::parse::parse(&mut mp4_input_file).unwrap();

    for video_track in mp4.video_tracks {
        if video_track.codec() == VideoCodec::H264 {
            println!("{}", video_track);
            
            let avc_config = video_track.avc_config_record().unwrap();

            for sps in avc_config.sps.iter() {
                h264_output_file.write_all(Nalu::new(sps.clone()).as_bytes()).unwrap();
            }
            for pps in avc_config.pps.iter() {
                h264_output_file.write_all(Nalu::new(pps.clone()).as_bytes()).unwrap();
            }

            for sample in video_track.samples() {
                for nalu in sample.nalus(&mut mp4_input_file) {
                    h264_output_file.write_all(nalu.as_bytes()).unwrap();
                }
            }
        }
    }

    println!("$ ffplay {}", outfile);
}