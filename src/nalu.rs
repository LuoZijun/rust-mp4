
use std::fmt;

// AVC level 6.2 allows up to 139264 macro blocks per frame. 
// If we use 10 bit color 4:4:4 it’s 30 bits per pixel. 
// So (30*139264*16*16)/8 gives about 133.7mbytes for an uncompressed image. 
// H.264 has a PCM_I encoding that allows for uncompressed images. 
// There is a little ovehead for the NAL header, so let’s call it 134Mbyte. 
// But in the real world the frame probablly will not be this large, and will likely be compressed.

#[repr(u8)]
#[derive(Debug)]
pub enum NaluKind {
    Unspecified = 0,
    // Coded slice of a non-IDR picture
    SLICE    = 1,
    // Coded slice data partition A
    DPA      = 2,
    // Coded slice data partition B
    DPB      = 3,
    // Coded slice data partition C
    DPC      = 4,
    // Coded slice of an IDR picture
    IDR      = 5,
    // Supplemental enhancement information (SEI)
    SEI      = 6,
    // Sequence parameter set
    SPS      = 7,
    // Picture parameter set
    PPS      = 8,
    // Access unit delimiter
    AUD      = 9,
    // End of sequence
    EOSEQ    = 10,
    // End of stream
    EOSTREAM = 11,
    // Filler data
    FILL     = 12,
    // Sequence parameter set extension
    SPSE     = 13,
}


#[repr(u8)]
#[derive(Debug)]
pub enum NaluRefIdc {
    DISPOSABLE = 0,
    LOW        = 1,
    HIGH       = 2,
    HIGHEST    = 3,
}

// 7.3   Syntax in tabular form
// 7.3.1 NAL unit syntax
#[derive(Debug)]
pub struct Nalu {
    leading_zeros: usize,
    bytes: Vec<u8>,
}


impl Nalu {
    pub fn new(mut bytes: Vec<u8>) -> Nalu {
        assert_eq!(bytes.len() > 0, true);
        assert_eq!(bytes[0] >> 7, 0);

        let len = bytes.len();

        let mut leading_zeros = 2usize;

        if &bytes[len-2..] == b"00" {
            bytes.insert(0, 1u8);
            bytes.insert(0, 0u8);
            bytes.insert(0, 0u8);

            bytes.push(3);
        } else {
            bytes.insert(0, 1u8);
            bytes.insert(0, 0u8);
            bytes.insert(0, 0u8);
            bytes.insert(0, 0u8);

            leading_zeros = 3;
        }

        Nalu {
            leading_zeros: leading_zeros,
            bytes: bytes
        }
    }

    pub fn forbidden_zero_bit(&self) -> u8 {
        // forbidden_zero_bit  1 bits
        let n = self.bytes[0] >> 7;
        assert_eq!(n, 0);

        n
    }

    pub fn ref_idc(&self) -> NaluRefIdc {
        // nal_ref_idc         2 bits
        let n = (self.bytes[self.leading_zeros+1] >> 5) & 0b011;

        match n {
            0 => NaluRefIdc::DISPOSABLE,
            1 => NaluRefIdc::LOW,
            2 => NaluRefIdc::HIGH,
            3 => NaluRefIdc::HIGHEST,
            _ => unreachable!(),
        }
    }

    pub fn kind(&self) -> NaluKind {
        // nal_unit_type       5 bits
        let n = self.bytes[self.leading_zeros+1] & 0b00011111;

        match n {
            0 => NaluKind::Unspecified,
            1 => NaluKind::SLICE,
            2 => NaluKind::DPA,
            3 => NaluKind::DPB,
            4 => NaluKind::DPC,
            5 => NaluKind::IDR,
            6 => NaluKind::SEI,
            7 => NaluKind::SPS,
            8 => NaluKind::PPS,
            9 => NaluKind::AUD,
            10 => NaluKind::EOSEQ,
            11 => NaluKind::EOSTREAM,
            12 => NaluKind::FILL,
            13 => NaluKind::SPSE,
            _ => unreachable!(),
        }
    }

    pub fn payload(&self) -> &[u8] {
        unimplemented!()
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn as_annex_b_bytes(&self) -> &[u8] {
        unimplemented!()
    }

    pub fn as_avc_bytes(&self) -> &[u8] {
        unimplemented!()
    }
}

impl fmt::Display for Nalu {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "NALu: Reference IDC: {:15} Type: {:10}",
            format!("{:?}", self.ref_idc()),
            format!("{:?}", self.kind()),
        )
    }
}
