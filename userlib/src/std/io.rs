//! std::io compatibility layer for KPIO
//!
//! Provides Read, Write, and BufRead traits via KPIO syscalls.

use alloc::string::String;
use alloc::vec::Vec;

use super::net::IoError;

/// Result type for IO operations
pub type Result<T> = core::result::Result<T, IoError>;

/// Read trait
pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
    
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        let mut pos = 0;
        while pos < buf.len() {
            match self.read(&mut buf[pos..]) {
                Ok(0) => return Err(IoError::UnexpectedEof),
                Ok(n) => pos += n,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
    
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        let mut tmp = [0u8; 4096];
        let mut total = 0;
        loop {
            match self.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    total += n;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(total)
    }
    
    fn read_to_string(&mut self, buf: &mut String) -> Result<usize> {
        let mut bytes = Vec::new();
        let n = self.read_to_end(&mut bytes)?;
        match core::str::from_utf8(&bytes) {
            Ok(s) => {
                buf.push_str(s);
                Ok(n)
            }
            Err(_) => Err(IoError::InvalidData),
        }
    }
}

/// Write trait
pub trait Write {
    fn write(&mut self, buf: &[u8]) -> Result<usize>;
    fn flush(&mut self) -> Result<()>;
    
    fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        let mut pos = 0;
        while pos < buf.len() {
            match self.write(&buf[pos..]) {
                Ok(0) => return Err(IoError::WriteZero),
                Ok(n) => pos += n,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
    
    fn write_fmt(&mut self, fmt: core::fmt::Arguments<'_>) -> Result<()> {
        struct Adapter<'a, T: ?Sized + 'a> {
            inner: &'a mut T,
            error: Result<()>,
        }
        
        impl<T: Write + ?Sized> core::fmt::Write for Adapter<'_, T> {
            fn write_str(&mut self, s: &str) -> core::fmt::Result {
                match self.inner.write_all(s.as_bytes()) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        self.error = Err(e);
                        Err(core::fmt::Error)
                    }
                }
            }
        }
        
        let mut adapter = Adapter {
            inner: self,
            error: Ok(()),
        };
        
        match core::fmt::write(&mut adapter, fmt) {
            Ok(()) => Ok(()),
            Err(_) => adapter.error,
        }
    }
}

/// BufRead trait
pub trait BufRead: Read {
    fn fill_buf(&mut self) -> Result<&[u8]>;
    fn consume(&mut self, amt: usize);
    
    fn read_line(&mut self, buf: &mut String) -> Result<usize> {
        let mut total = 0;
        loop {
            let available = self.fill_buf()?;
            if available.is_empty() {
                break;
            }
            
            if let Some(pos) = available.iter().position(|&b| b == b'\n') {
                let line = &available[..=pos];
                if let Ok(s) = core::str::from_utf8(line) {
                    buf.push_str(s);
                }
                total += pos + 1;
                self.consume(pos + 1);
                break;
            } else {
                if let Ok(s) = core::str::from_utf8(available) {
                    buf.push_str(s);
                }
                let len = available.len();
                total += len;
                self.consume(len);
            }
        }
        Ok(total)
    }
}

/// Seek trait
pub trait Seek {
    fn seek(&mut self, pos: super::fs::SeekFrom) -> Result<u64>;
    
    fn rewind(&mut self) -> Result<()> {
        self.seek(super::fs::SeekFrom::Start(0))?;
        Ok(())
    }
    
    fn stream_position(&mut self) -> Result<u64> {
        self.seek(super::fs::SeekFrom::Current(0))
    }
}

/// Buffered reader
pub struct BufReader<R> {
    inner: R,
    buf: Vec<u8>,
    pos: usize,
    cap: usize,
}

impl<R: Read> BufReader<R> {
    pub fn new(inner: R) -> Self {
        Self::with_capacity(8192, inner)
    }
    
    pub fn with_capacity(capacity: usize, inner: R) -> Self {
        let mut buf = Vec::with_capacity(capacity);
        buf.resize(capacity, 0);
        BufReader {
            inner,
            buf,
            pos: 0,
            cap: 0,
        }
    }
    
    pub fn get_ref(&self) -> &R {
        &self.inner
    }
    
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }
    
    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: Read> Read for BufReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        // If we have buffered data, return it
        if self.pos < self.cap {
            let available = &self.buf[self.pos..self.cap];
            let len = available.len().min(buf.len());
            buf[..len].copy_from_slice(&available[..len]);
            self.pos += len;
            return Ok(len);
        }
        
        // Buffer is empty, read directly for large requests
        if buf.len() >= self.buf.len() {
            return self.inner.read(buf);
        }
        
        // Refill buffer
        self.pos = 0;
        self.cap = self.inner.read(&mut self.buf)?;
        if self.cap == 0 {
            return Ok(0);
        }
        
        let len = self.cap.min(buf.len());
        buf[..len].copy_from_slice(&self.buf[..len]);
        self.pos = len;
        Ok(len)
    }
}

impl<R: Read> BufRead for BufReader<R> {
    fn fill_buf(&mut self) -> Result<&[u8]> {
        if self.pos >= self.cap {
            self.pos = 0;
            self.cap = self.inner.read(&mut self.buf)?;
        }
        Ok(&self.buf[self.pos..self.cap])
    }
    
    fn consume(&mut self, amt: usize) {
        self.pos = (self.pos + amt).min(self.cap);
    }
}

/// Buffered writer
pub struct BufWriter<W: Write> {
    inner: W,
    buf: Vec<u8>,
}

impl<W: Write> BufWriter<W> {
    pub fn new(inner: W) -> Self {
        Self::with_capacity(8192, inner)
    }
    
    pub fn with_capacity(capacity: usize, inner: W) -> Self {
        BufWriter {
            inner,
            buf: Vec::with_capacity(capacity),
        }
    }
    
    fn flush_buf(&mut self) -> Result<()> {
        if !self.buf.is_empty() {
            self.inner.write_all(&self.buf)?;
            self.buf.clear();
        }
        Ok(())
    }
    
    pub fn get_ref(&self) -> &W {
        &self.inner
    }
    
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }
    
    pub fn into_inner(mut self) -> core::result::Result<W, (IoError, Self)> {
        match self.flush_buf() {
            Ok(()) => {
                // Use ManuallyDrop to avoid double-free
                let mut me = core::mem::ManuallyDrop::new(self);
                // SAFETY: We're consuming self and won't drop it
                Ok(unsafe { core::ptr::read(&me.inner) })
            },
            Err(e) => Err((e, self)),
        }
    }
}

impl<W: Write> Write for BufWriter<W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if self.buf.len() + buf.len() > self.buf.capacity() {
            self.flush_buf()?;
        }
        
        if buf.len() >= self.buf.capacity() {
            self.inner.write(buf)
        } else {
            self.buf.extend_from_slice(buf);
            Ok(buf.len())
        }
    }
    
    fn flush(&mut self) -> Result<()> {
        self.flush_buf()?;
        self.inner.flush()
    }
}

impl<W: Write> Drop for BufWriter<W> {
    fn drop(&mut self) {
        let _ = self.flush_buf();
    }
}

/// Cursor for in-memory I/O
pub struct Cursor<T> {
    inner: T,
    pos: u64,
}

impl<T> Cursor<T> {
    pub fn new(inner: T) -> Self {
        Cursor { inner, pos: 0 }
    }
    
    pub fn into_inner(self) -> T {
        self.inner
    }
    
    pub fn get_ref(&self) -> &T {
        &self.inner
    }
    
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
    
    pub fn position(&self) -> u64 {
        self.pos
    }
    
    pub fn set_position(&mut self, pos: u64) {
        self.pos = pos;
    }
}

impl Read for Cursor<&[u8]> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let pos = self.pos as usize;
        if pos >= self.inner.len() {
            return Ok(0);
        }
        
        let remaining = &self.inner[pos..];
        let len = remaining.len().min(buf.len());
        buf[..len].copy_from_slice(&remaining[..len]);
        self.pos += len as u64;
        Ok(len)
    }
}

impl Read for Cursor<Vec<u8>> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let pos = self.pos as usize;
        if pos >= self.inner.len() {
            return Ok(0);
        }
        
        let remaining = &self.inner[pos..];
        let len = remaining.len().min(buf.len());
        buf[..len].copy_from_slice(&remaining[..len]);
        self.pos += len as u64;
        Ok(len)
    }
}

impl Write for Cursor<Vec<u8>> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let pos = self.pos as usize;
        
        // Extend if necessary
        if pos + buf.len() > self.inner.len() {
            self.inner.resize(pos + buf.len(), 0);
        }
        
        self.inner[pos..pos + buf.len()].copy_from_slice(buf);
        self.pos += buf.len() as u64;
        Ok(buf.len())
    }
    
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl<T: AsRef<[u8]>> Seek for Cursor<T> {
    fn seek(&mut self, pos: super::fs::SeekFrom) -> Result<u64> {
        let new_pos = match pos {
            super::fs::SeekFrom::Start(n) => n as i64,
            super::fs::SeekFrom::End(n) => self.inner.as_ref().len() as i64 + n,
            super::fs::SeekFrom::Current(n) => self.pos as i64 + n,
        };
        
        if new_pos < 0 {
            return Err(IoError::InvalidInput);
        }
        
        self.pos = new_pos as u64;
        Ok(self.pos)
    }
}

// ============================================
// Stdin/Stdout/Stderr
// ============================================

/// Standard input
pub struct Stdin;

/// Standard output
pub struct Stdout;

/// Standard error
pub struct Stderr;

impl Read for Stdin {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        crate::syscall::stdin_read(buf)
            .map(|n| n as usize)
            .map_err(|_| IoError::Other)
    }
}

impl Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        crate::syscall::stdout_write(buf)
            .map(|n| n as usize)
            .map_err(|_| IoError::Other)
    }
    
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Write for Stderr {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        crate::syscall::stderr_write(buf)
            .map(|n| n as usize)
            .map_err(|_| IoError::Other)
    }
    
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

pub fn stdin() -> Stdin { Stdin }
pub fn stdout() -> Stdout { Stdout }
pub fn stderr() -> Stderr { Stderr }

/// Empty reader (always returns EOF)
pub struct Empty;

impl Read for Empty {
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize> {
        Ok(0)
    }
}

pub fn empty() -> Empty { Empty }

/// Sink writer (discards all data)
pub struct Sink;

impl Write for Sink {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        Ok(buf.len())
    }
    
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

pub fn sink() -> Sink { Sink }

/// Repeat reader (infinite stream of a byte)
pub struct Repeat {
    byte: u8,
}

impl Read for Repeat {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        for b in buf.iter_mut() {
            *b = self.byte;
        }
        Ok(buf.len())
    }
}

pub fn repeat(byte: u8) -> Repeat {
    Repeat { byte }
}

/// Copy from reader to writer
pub fn copy<R: Read, W: Write>(reader: &mut R, writer: &mut W) -> Result<u64> {
    let mut buf = [0u8; 8192];
    let mut total = 0u64;
    
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                writer.write_all(&buf[..n])?;
                total += n as u64;
            }
            Err(e) => return Err(e),
        }
    }
    
    Ok(total)
}
