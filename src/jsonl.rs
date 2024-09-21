use std::io::Read;

use orfail::OrFail;
use serde::Deserialize;

#[derive(Debug)]
pub struct JsonlReader<R> {
    inner: R,
    buf: Vec<u8>,
    buf_offset: usize,
    buf_end: usize,
}

impl<R: Read> JsonlReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            inner: reader,
            buf: vec![0; 4096],
            buf_offset: 0,
            buf_end: 0,
        }
    }

    pub fn read_item<T>(&mut self) -> orfail::Result<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        if self.buf_offset != 0 {
            if let Some(i) = self.buf[self.buf_offset..self.buf_end]
                .iter()
                .position(|&b| b == b'\n')
                .map(|i| self.buf_offset + i)
            {
                let item = serde_json::from_slice(&self.buf[self.buf_offset..i]).or_fail()?;
                self.buf_offset = i + 1;
                return Ok(item);
            }

            self.buf.copy_within(self.buf_offset..self.buf_end, 0);
            self.buf_end -= self.buf_offset;
            self.buf_offset = 0;
        }

        loop {
            if self.buf_end == self.buf.len() {
                self.buf.resize(self.buf.len() * 2, 0);
            }

            let read_size = self.inner.read(&mut self.buf[self.buf_end..]).or_fail()?;
            if read_size == 0 {
                return Ok(None);
            }

            let old_end = self.buf_end;
            self.buf_end += read_size;

            if let Some(i) = self.buf[old_end..self.buf_end]
                .iter()
                .position(|&b| b == b'\n')
                .map(|i| old_end + i)
            {
                let item = serde_json::from_slice(&self.buf[..i]).or_fail()?;
                self.buf_offset = i + 1;
                return Ok(Some(item));
            }
        }
    }
}
