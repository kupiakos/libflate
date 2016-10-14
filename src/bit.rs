use std::io;
use byteorder::LittleEndian;
use byteorder::ReadBytesExt;
use byteorder::WriteBytesExt;

#[derive(Debug)]
pub struct BitWriter<W> {
    inner: W,
    buf: u32,
    end: u8,
}
impl<W> BitWriter<W>
    where W: io::Write
{
    pub fn new(inner: W) -> Self {
        BitWriter {
            inner: inner,
            buf: 0,
            end: 0,
        }
    }
    #[inline(always)]
    pub fn write_bit(&mut self, bit: bool) -> io::Result<()> {
        self.write_bits(1, bit as u16)
    }
    #[inline(always)]
    pub fn write_bits(&mut self, bitwidth: u8, bits: u16) -> io::Result<()> {
        debug_assert!(bitwidth < 16);
        debug_assert!(self.end + bitwidth <= 32);
        self.buf |= (bits as u32) << self.end;
        self.end += bitwidth;
        self.flush_if_needed()
    }
    pub fn flush(&mut self) -> io::Result<()> {
        while self.end > 0 {
            try!(self.inner.write_u8(self.buf as u8));
            self.buf >>= 8;
            self.end = self.end.saturating_sub(8);
        }
        try!(self.inner.flush());
        Ok(())
    }
    #[inline(always)]
    fn flush_if_needed(&mut self) -> io::Result<()> {
        if self.end >= 16 {
            try!(self.inner.write_u16::<LittleEndian>(self.buf as u16));
            self.end -= 16;
            self.buf >>= 16;
        }
        Ok(())
    }
}
impl<W> BitWriter<W> {
    pub fn as_inner_ref(&self) -> &W {
        &self.inner
    }
    pub fn as_inner_mut(&mut self) -> &mut W {
        &mut self.inner
    }
    pub fn into_inner(self) -> W {
        self.inner
    }
}

#[derive(Debug)]
pub struct BitReader<R> {
    inner: R,
    last_read: u32,
    offset: u8,
}
impl<R> BitReader<R>
    where R: io::Read
{
    pub fn new(inner: R) -> Self {
        BitReader {
            inner: inner,
            last_read: 0,
            offset: 32,
        }
    }
    #[inline(always)]
    pub fn read_bit(&mut self) -> io::Result<bool> {
        self.read_bits(1).map(|b| b != 0)
    }
    #[inline(always)]
    pub fn read_bits(&mut self, bitwidth: u8) -> io::Result<u16> {
        self.peek_bits(bitwidth).map(|bits| {
            self.skip_bits(bitwidth);
            bits
        })
    }
    #[inline(always)]
    pub fn peek_bits(&mut self, bitwidth: u8) -> io::Result<u16> {
        debug_assert!(bitwidth <= 16);
        while (32 - self.offset) < bitwidth {
            try!(self.fill_next_u8());
        }
        let bits = (self.last_read >> self.offset) as u16;
        Ok(bits & ((1 << bitwidth) - 1))
    }
    #[inline(always)]
    pub fn skip_bits(&mut self, bitwidth: u8) {
        debug_assert!(32 - self.offset >= bitwidth);
        self.offset += bitwidth;
    }
    #[inline(always)]
    fn fill_next_u8(&mut self) -> io::Result<()> {
        self.offset -= 8;
        self.last_read >>= 8;

        let next = try!(self.inner.read_u8()) as u32;
        self.last_read |= next << (32 - 8);
        Ok(())
    }
}
impl<R> BitReader<R> {
    pub fn reset(&mut self) {
        self.offset = 32;
    }
    pub fn as_inner_ref(&self) -> &R {
        &self.inner
    }
    pub fn as_inner_mut(&mut self) -> &mut R {
        &mut self.inner
    }
    pub fn into_inner(self) -> R {
        self.inner
    }
}

#[cfg(test)]
mod test {
    use std::io;
    use super::*;

    #[test]
    fn writer_works() {
        let mut writer = BitWriter::new(Vec::new());
        writer.write_bit(true).unwrap();
        writer.write_bits(3, 0b010).unwrap();
        writer.write_bits(11, 0b10101011010).unwrap();
        writer.flush().unwrap();
        writer.write_bit(true).unwrap();
        writer.flush().unwrap();

        let buf = writer.into_inner();
        assert_eq!(buf, [0b10100101, 0b01010101, 0b00000001]);
    }

    #[test]
    fn reader_works() {
        let buf = vec![0b10100101, 0b11010101];
        let mut reader = BitReader::new(io::Cursor::new(buf));
        assert_eq!(reader.read_bit().unwrap(), true);
        assert_eq!(reader.read_bit().unwrap(), false);
        assert_eq!(reader.read_bits(8).unwrap(), 0b01101001);
        assert_eq!(reader.peek_bits(3).unwrap(), 0b101);
        assert_eq!(reader.peek_bits(3).unwrap(), 0b101);
        reader.skip_bits(1);
        assert_eq!(reader.peek_bits(3).unwrap(), 0b010);
        assert_eq!(reader.read_bits(8).map_err(|e| e.kind()),
                   Err(io::ErrorKind::UnexpectedEof));
    }
}