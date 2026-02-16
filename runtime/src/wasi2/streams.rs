// WASI Preview 2 — Streams (wasi:io/streams)
//
// This module implements the `wasi:io/streams` interface, providing
// `InputStream` and `OutputStream` abstractions used by all other
// WASI P2 interfaces (filesystem, sockets, HTTP, etc.).

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use super::StreamError;

// ---------------------------------------------------------------------------
// InputStream
// ---------------------------------------------------------------------------

/// Concrete data storage for input streams.
pub enum InputStreamData {
    /// Reads from an in-memory buffer.
    Memory(MemoryInputStream),
    /// Reads from standard input (kernel console).
    Stdin(StdinStream),
}

impl InputStreamData {
    /// Read up to `len` bytes.
    pub fn read(&mut self, len: usize) -> Result<Vec<u8>, StreamError> {
        match self {
            InputStreamData::Memory(s) => s.read(len),
            InputStreamData::Stdin(s) => s.read(len),
        }
    }

    /// Read exactly `len` bytes, blocking if necessary.
    pub fn blocking_read(&mut self, len: usize) -> Result<Vec<u8>, StreamError> {
        // In our single-threaded kernel, blocking == non-blocking.
        self.read(len)
    }

    /// Skip up to `len` bytes without copying.
    pub fn skip(&mut self, len: usize) -> Result<u64, StreamError> {
        match self {
            InputStreamData::Memory(s) => s.skip(len),
            InputStreamData::Stdin(s) => s.skip(len),
        }
    }

    /// Check if any data is available to read.
    pub fn is_ready(&self) -> bool {
        match self {
            InputStreamData::Memory(s) => s.is_ready(),
            InputStreamData::Stdin(s) => s.is_ready(),
        }
    }

    /// Check if the stream has been fully consumed / closed.
    pub fn is_closed(&self) -> bool {
        match self {
            InputStreamData::Memory(s) => s.is_closed(),
            InputStreamData::Stdin(_) => false,
        }
    }
}

// ---------------------------------------------------------------------------
// OutputStream
// ---------------------------------------------------------------------------

/// Concrete data storage for output streams.
pub enum OutputStreamData {
    /// Writes to an in-memory buffer.
    Memory(MemoryOutputStream),
    /// Writes to standard output.
    Stdout(StdoutStream),
    /// Writes to standard error.
    Stderr(StderrStream),
}

impl OutputStreamData {
    /// Check how many bytes can be written without blocking.
    pub fn check_write(&self) -> Result<u64, StreamError> {
        match self {
            OutputStreamData::Memory(s) => s.check_write(),
            OutputStreamData::Stdout(s) => s.check_write(),
            OutputStreamData::Stderr(s) => s.check_write(),
        }
    }

    /// Write bytes to the stream (non-blocking where possible).
    pub fn write(&mut self, bytes: &[u8]) -> Result<(), StreamError> {
        match self {
            OutputStreamData::Memory(s) => s.write(bytes),
            OutputStreamData::Stdout(s) => s.write(bytes),
            OutputStreamData::Stderr(s) => s.write(bytes),
        }
    }

    /// Write bytes and flush (blocking).
    pub fn blocking_write_and_flush(&mut self, bytes: &[u8]) -> Result<(), StreamError> {
        self.write(bytes)?;
        self.flush()
    }

    /// Flush buffered data.
    pub fn flush(&mut self) -> Result<(), StreamError> {
        match self {
            OutputStreamData::Memory(s) => s.flush(),
            OutputStreamData::Stdout(s) => s.flush(),
            OutputStreamData::Stderr(s) => s.flush(),
        }
    }

    /// Check if the stream is ready for writing.
    pub fn is_ready(&self) -> bool {
        // All our streams are always ready (no real I/O blocking).
        true
    }

    /// Read back the written data (only for memory streams).
    pub fn written_data(&self) -> Option<&[u8]> {
        match self {
            OutputStreamData::Memory(s) => Some(s.data()),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// MemoryInputStream — reads from a byte buffer
// ---------------------------------------------------------------------------

/// An input stream backed by an in-memory byte buffer.
pub struct MemoryInputStream {
    buffer: Vec<u8>,
    position: usize,
}

impl MemoryInputStream {
    /// Create a new memory input stream from the given data.
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            buffer: data,
            position: 0,
        }
    }

    /// Read up to `len` bytes.
    pub fn read(&mut self, len: usize) -> Result<Vec<u8>, StreamError> {
        if self.position >= self.buffer.len() {
            return Err(StreamError::Closed);
        }
        let available = self.buffer.len() - self.position;
        let to_read = len.min(available);
        let data = self.buffer[self.position..self.position + to_read].to_vec();
        self.position += to_read;
        Ok(data)
    }

    /// Skip up to `len` bytes.
    pub fn skip(&mut self, len: usize) -> Result<u64, StreamError> {
        if self.position >= self.buffer.len() {
            return Err(StreamError::Closed);
        }
        let available = self.buffer.len() - self.position;
        let to_skip = len.min(available);
        self.position += to_skip;
        Ok(to_skip as u64)
    }

    /// Returns true if there is data available.
    pub fn is_ready(&self) -> bool {
        self.position < self.buffer.len()
    }

    /// Returns true if all data has been consumed.
    pub fn is_closed(&self) -> bool {
        self.position >= self.buffer.len()
    }

    /// Remaining bytes.
    pub fn remaining(&self) -> usize {
        self.buffer.len().saturating_sub(self.position)
    }
}

// ---------------------------------------------------------------------------
// MemoryOutputStream — writes to a growable buffer
// ---------------------------------------------------------------------------

/// An output stream backed by a growable in-memory buffer.
pub struct MemoryOutputStream {
    buffer: Vec<u8>,
    /// Maximum capacity (0 = unlimited).
    max_capacity: usize,
}

impl MemoryOutputStream {
    /// Create a new memory output stream with unlimited capacity.
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            max_capacity: 0,
        }
    }

    /// Create a new memory output stream with a maximum capacity.
    pub fn with_capacity(max: usize) -> Self {
        Self {
            buffer: Vec::new(),
            max_capacity: max,
        }
    }

    /// How many bytes can be written.
    pub fn check_write(&self) -> Result<u64, StreamError> {
        if self.max_capacity == 0 {
            // Unlimited — report 64KB available
            Ok(65536)
        } else {
            let remaining = self.max_capacity.saturating_sub(self.buffer.len());
            Ok(remaining as u64)
        }
    }

    /// Write bytes to the buffer.
    pub fn write(&mut self, bytes: &[u8]) -> Result<(), StreamError> {
        if self.max_capacity > 0 && self.buffer.len() + bytes.len() > self.max_capacity {
            return Err(StreamError::LastOperationFailed(String::from(
                "write exceeds capacity",
            )));
        }
        self.buffer.extend_from_slice(bytes);
        Ok(())
    }

    /// Flush (no-op for memory stream).
    pub fn flush(&self) -> Result<(), StreamError> {
        Ok(())
    }

    /// Get the written data.
    pub fn data(&self) -> &[u8] {
        &self.buffer
    }

    /// Take the buffer, consuming it.
    pub fn take_data(&mut self) -> Vec<u8> {
        core::mem::take(&mut self.buffer)
    }
}

// ---------------------------------------------------------------------------
// StdinStream — reads from kernel console input
// ---------------------------------------------------------------------------

/// Standard input stream (reads from kernel console).
///
/// In the kernel environment, stdin is typically empty or provides
/// pre-buffered input that was configured before execution.
pub struct StdinStream {
    buffer: Vec<u8>,
    position: usize,
}

impl StdinStream {
    /// Create a new stdin stream (empty by default).
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            position: 0,
        }
    }

    /// Provide input data (e.g., from kernel console).
    pub fn provide_input(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    pub fn read(&mut self, len: usize) -> Result<Vec<u8>, StreamError> {
        if self.position >= self.buffer.len() {
            // Stdin returns empty read (not closed) when no data
            return Ok(Vec::new());
        }
        let available = self.buffer.len() - self.position;
        let to_read = len.min(available);
        let data = self.buffer[self.position..self.position + to_read].to_vec();
        self.position += to_read;
        Ok(data)
    }

    pub fn skip(&mut self, len: usize) -> Result<u64, StreamError> {
        let available = self.buffer.len().saturating_sub(self.position);
        let to_skip = len.min(available);
        self.position += to_skip;
        Ok(to_skip as u64)
    }

    pub fn is_ready(&self) -> bool {
        self.position < self.buffer.len()
    }
}

// ---------------------------------------------------------------------------
// StdoutStream — writes to stdout buffer
// ---------------------------------------------------------------------------

/// Standard output stream (accumulates output for retrieval).
pub struct StdoutStream {
    buffer: Vec<u8>,
}

impl StdoutStream {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn check_write(&self) -> Result<u64, StreamError> {
        Ok(65536)
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<(), StreamError> {
        self.buffer.extend_from_slice(bytes);
        Ok(())
    }

    pub fn flush(&self) -> Result<(), StreamError> {
        Ok(())
    }

    /// Read the accumulated stdout data.
    pub fn data(&self) -> &[u8] {
        &self.buffer
    }

    /// Take the buffer.
    pub fn take_data(&mut self) -> Vec<u8> {
        core::mem::take(&mut self.buffer)
    }
}

// ---------------------------------------------------------------------------
// StderrStream — writes to stderr buffer
// ---------------------------------------------------------------------------

/// Standard error stream (accumulates error output for retrieval).
pub struct StderrStream {
    buffer: Vec<u8>,
}

impl StderrStream {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn check_write(&self) -> Result<u64, StreamError> {
        Ok(65536)
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<(), StreamError> {
        self.buffer.extend_from_slice(bytes);
        Ok(())
    }

    pub fn flush(&self) -> Result<(), StreamError> {
        Ok(())
    }

    /// Read the accumulated stderr data.
    pub fn data(&self) -> &[u8] {
        &self.buffer
    }

    /// Take the buffer.
    pub fn take_data(&mut self) -> Vec<u8> {
        core::mem::take(&mut self.buffer)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_input_stream_read() {
        let mut stream = MemoryInputStream::new(alloc::vec![1, 2, 3, 4, 5]);
        assert!(stream.is_ready());
        assert!(!stream.is_closed());
        assert_eq!(stream.remaining(), 5);

        // Read 3 bytes
        let data = stream.read(3).unwrap();
        assert_eq!(data, alloc::vec![1, 2, 3]);
        assert_eq!(stream.remaining(), 2);

        // Read remaining
        let data = stream.read(10).unwrap();
        assert_eq!(data, alloc::vec![4, 5]);
        assert!(stream.is_closed());

        // Read after closed
        assert!(stream.read(1).is_err());
    }

    #[test]
    fn memory_input_stream_skip() {
        let mut stream = MemoryInputStream::new(alloc::vec![1, 2, 3, 4, 5]);
        let skipped = stream.skip(3).unwrap();
        assert_eq!(skipped, 3);
        let data = stream.read(10).unwrap();
        assert_eq!(data, alloc::vec![4, 5]);
    }

    #[test]
    fn memory_output_stream_write() {
        let mut stream = MemoryOutputStream::new();
        stream.write(b"Hello").unwrap();
        stream.write(b", World!").unwrap();
        assert_eq!(stream.data(), b"Hello, World!");
    }

    #[test]
    fn memory_output_stream_capacity() {
        let mut stream = MemoryOutputStream::with_capacity(5);
        stream.write(b"Hi").unwrap();
        assert_eq!(stream.check_write().unwrap(), 3);
        // Exceeds capacity
        assert!(stream.write(b"Hello!").is_err());
        // Exactly fill
        stream.write(b"!!X").unwrap();
        assert_eq!(stream.data(), b"Hi!!X");
    }

    #[test]
    fn stdin_stream_empty_read() {
        let mut stdin = StdinStream::new();
        let data = stdin.read(10).unwrap();
        assert!(data.is_empty());
    }

    #[test]
    fn stdin_stream_with_input() {
        let mut stdin = StdinStream::new();
        stdin.provide_input(b"test input");
        assert!(stdin.is_ready());
        let data = stdin.read(4).unwrap();
        assert_eq!(data, b"test");
        let data = stdin.read(100).unwrap();
        assert_eq!(data, b" input");
    }

    #[test]
    fn stdout_stream_write_and_read() {
        let mut stdout = StdoutStream::new();
        stdout.write(b"line 1\n").unwrap();
        stdout.write(b"line 2\n").unwrap();
        assert_eq!(stdout.data(), b"line 1\nline 2\n");
    }

    #[test]
    fn stderr_stream_write_and_read() {
        let mut stderr = StderrStream::new();
        stderr.write(b"error!").unwrap();
        assert_eq!(stderr.data(), b"error!");
    }

    #[test]
    fn input_stream_data_memory_roundtrip() {
        let mut isd = InputStreamData::Memory(MemoryInputStream::new(alloc::vec![10, 20, 30]));
        assert!(isd.is_ready());
        let data = isd.read(2).unwrap();
        assert_eq!(data, alloc::vec![10, 20]);
        let data = isd.blocking_read(5).unwrap();
        assert_eq!(data, alloc::vec![30]);
    }

    #[test]
    fn output_stream_data_memory_roundtrip() {
        let mut osd = OutputStreamData::Memory(MemoryOutputStream::new());
        assert!(osd.is_ready());
        assert!(osd.check_write().unwrap() > 0);
        osd.write(b"test").unwrap();
        assert_eq!(osd.written_data().unwrap(), b"test");
    }

    #[test]
    fn output_stream_data_stdout() {
        let mut osd = OutputStreamData::Stdout(StdoutStream::new());
        osd.blocking_write_and_flush(b"hello").unwrap();
        assert!(osd.written_data().is_none()); // written_data only for memory
        assert!(osd.is_ready());
    }

    #[test]
    fn memory_output_take_data() {
        let mut stream = MemoryOutputStream::new();
        stream.write(b"data").unwrap();
        let taken = stream.take_data();
        assert_eq!(taken, b"data");
        assert!(stream.data().is_empty());
    }
}
