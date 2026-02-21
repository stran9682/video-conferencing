use bytes::{BufMut, BytesMut};

pub struct ReceptionReport {
    pub ssrc: u32,
    pub fraction_lost: u8,
    pub total_lost: u32,
    pub last_sequence_number: u32,
    pub jitter: u32,
    pub last_sender_report: u32,
    pub delay: u32,
}

impl ReceptionReport {
    /// marshal_to encodes the ReceptionReport in binary
    pub fn serialize(&self) -> BytesMut {
        /*
         *  0                   1                   2                   3
         *  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
         * +=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+
         * |                              SSRC                             |
         * +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
         * | fraction lost |       cumulative number of packets lost       |
         * +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
         * |           extended highest sequence number received           |
         * +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
         * |                      interarrival jitter                      |
         * +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
         * |                         last SR (LSR)                         |
         * +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
         * |                   delay since last SR (DLSR)                  |
         * +=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+
         */

        let mut buf = BytesMut::with_capacity(24);

        buf.put_u32(self.ssrc);

        buf.put_u8(self.fraction_lost);

        buf.put_u8(((self.total_lost >> 16) & 0xFF) as u8);
        buf.put_u8(((self.total_lost >> 8) & 0xFF) as u8);
        buf.put_u8((self.total_lost & 0xFF) as u8);

        buf.put_u32(self.last_sequence_number);
        buf.put_u32(self.jitter);
        buf.put_u32(self.last_sender_report);
        buf.put_u32(self.delay);

        buf
    }
}



