//! Media Viewer
//!
//! Image and video viewer application.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Media viewer instance
#[derive(Debug, Clone)]
pub struct MediaViewer {
    /// Viewer ID
    pub id: u64,
    /// Current media
    pub current: Option<Media>,
    /// Playlist/gallery
    pub playlist: Vec<MediaItem>,
    /// Current index in playlist
    pub current_index: usize,
    /// Viewer mode
    pub mode: ViewerMode,
    /// Zoom level (1.0 = 100%)
    pub zoom: f32,
    /// Pan offset
    pub pan_offset: (f32, f32),
    /// Rotation (degrees)
    pub rotation: i32,
    /// Flip horizontal
    pub flip_h: bool,
    /// Flip vertical
    pub flip_v: bool,
    /// Settings
    pub settings: ViewerSettings,
    /// Slideshow state
    pub slideshow: Option<SlideshowState>,
}

impl MediaViewer {
    /// Create new media viewer
    pub fn new(id: u64) -> Self {
        Self {
            id,
            current: None,
            playlist: Vec::new(),
            current_index: 0,
            mode: ViewerMode::Image,
            zoom: 1.0,
            pan_offset: (0.0, 0.0),
            rotation: 0,
            flip_h: false,
            flip_v: false,
            settings: ViewerSettings::default(),
            slideshow: None,
        }
    }

    /// Open media file
    pub fn open(&mut self, path: &str) {
        let media_type = MediaType::from_path(path);

        self.current = Some(Media {
            path: path.to_string(),
            media_type,
            name: path.split('/').last().unwrap_or("media").to_string(),
            metadata: MediaMetadata::default(),
        });

        self.mode = match media_type {
            MediaType::Image => ViewerMode::Image,
            MediaType::Video => ViewerMode::Video,
            MediaType::Audio => ViewerMode::Audio,
        };

        self.reset_view();
    }

    /// Open multiple files
    pub fn open_multiple(&mut self, paths: &[String]) {
        self.playlist.clear();

        for path in paths {
            let media_type = MediaType::from_path(path);
            self.playlist.push(MediaItem {
                path: path.clone(),
                name: path.split('/').last().unwrap_or("media").to_string(),
                media_type,
                thumbnail: None,
            });
        }

        if !self.playlist.is_empty() {
            self.current_index = 0;
            self.open(&self.playlist[0].path.clone());
        }
    }

    /// Reset view
    pub fn reset_view(&mut self) {
        self.zoom = 1.0;
        self.pan_offset = (0.0, 0.0);
        self.rotation = 0;
        self.flip_h = false;
        self.flip_v = false;
    }

    /// Next item
    pub fn next(&mut self) {
        if self.playlist.is_empty() {
            return;
        }
        self.current_index = (self.current_index + 1) % self.playlist.len();
        self.open(&self.playlist[self.current_index].path.clone());
    }

    /// Previous item
    pub fn prev(&mut self) {
        if self.playlist.is_empty() {
            return;
        }
        if self.current_index == 0 {
            self.current_index = self.playlist.len() - 1;
        } else {
            self.current_index -= 1;
        }
        self.open(&self.playlist[self.current_index].path.clone());
    }

    /// Go to index
    pub fn go_to(&mut self, index: usize) {
        if index < self.playlist.len() {
            self.current_index = index;
            self.open(&self.playlist[index].path.clone());
        }
    }

    /// Zoom in
    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 1.25).min(10.0);
    }

    /// Zoom out
    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / 1.25).max(0.1);
    }

    /// Fit to window
    pub fn fit_to_window(&mut self) {
        self.zoom = 1.0;
        self.pan_offset = (0.0, 0.0);
    }

    /// Actual size (100%)
    pub fn actual_size(&mut self) {
        self.zoom = 1.0;
    }

    /// Rotate left
    pub fn rotate_left(&mut self) {
        self.rotation = (self.rotation - 90 + 360) % 360;
    }

    /// Rotate right
    pub fn rotate_right(&mut self) {
        self.rotation = (self.rotation + 90) % 360;
    }

    /// Flip horizontal
    pub fn flip_horizontal(&mut self) {
        self.flip_h = !self.flip_h;
    }

    /// Flip vertical
    pub fn flip_vertical(&mut self) {
        self.flip_v = !self.flip_v;
    }

    /// Start slideshow
    pub fn start_slideshow(&mut self) {
        self.slideshow = Some(SlideshowState {
            playing: true,
            interval_secs: self.settings.slideshow_interval,
            elapsed: 0.0,
        });
    }

    /// Stop slideshow
    pub fn stop_slideshow(&mut self) {
        self.slideshow = None;
    }

    /// Toggle slideshow
    pub fn toggle_slideshow(&mut self) {
        if self.slideshow.is_some() {
            self.stop_slideshow();
        } else {
            self.start_slideshow();
        }
    }

    /// Update slideshow (call each frame)
    pub fn update_slideshow(&mut self, delta_secs: f32) {
        if let Some(state) = &mut self.slideshow {
            if state.playing {
                state.elapsed += delta_secs;
                if state.elapsed >= state.interval_secs as f32 {
                    state.elapsed = 0.0;
                    self.next();
                }
            }
        }
    }

    /// Toggle fullscreen mode
    pub fn toggle_fullscreen(&mut self) {
        self.mode = match self.mode {
            ViewerMode::Image => ViewerMode::ImageFullscreen,
            ViewerMode::ImageFullscreen => ViewerMode::Image,
            ViewerMode::Video => ViewerMode::VideoFullscreen,
            ViewerMode::VideoFullscreen => ViewerMode::Video,
            other => other,
        };
    }
}

impl Default for MediaViewer {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Media
#[derive(Debug, Clone)]
pub struct Media {
    /// File path
    pub path: String,
    /// Media type
    pub media_type: MediaType,
    /// File name
    pub name: String,
    /// Metadata
    pub metadata: MediaMetadata,
}

/// Media type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Image,
    Video,
    Audio,
}

impl MediaType {
    /// Detect from path
    pub fn from_path(path: &str) -> Self {
        let ext = path.rsplit('.').next().map(|s| s.to_lowercase());

        match ext.as_deref() {
            Some("jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "svg" | "ico" | "tiff") => {
                Self::Image
            }
            Some("mp4" | "mkv" | "avi" | "mov" | "webm" | "wmv" | "flv" | "m4v") => Self::Video,
            Some("mp3" | "wav" | "ogg" | "flac" | "aac" | "m4a" | "wma") => Self::Audio,
            _ => Self::Image, // Default
        }
    }
}

/// Media item in playlist
#[derive(Debug, Clone)]
pub struct MediaItem {
    /// File path
    pub path: String,
    /// File name
    pub name: String,
    /// Media type
    pub media_type: MediaType,
    /// Thumbnail URL
    pub thumbnail: Option<String>,
}

/// Media metadata
#[derive(Debug, Clone, Default)]
pub struct MediaMetadata {
    /// Width (images/video)
    pub width: Option<u32>,
    /// Height (images/video)
    pub height: Option<u32>,
    /// Duration (video/audio) in seconds
    pub duration: Option<f64>,
    /// File size
    pub file_size: Option<u64>,
    /// Created date
    pub created: Option<u64>,
    /// Modified date
    pub modified: Option<u64>,
    /// Camera/device
    pub camera: Option<String>,
    /// Location
    pub location: Option<GpsLocation>,
    /// Additional tags
    pub tags: Vec<String>,
}

/// GPS location
#[derive(Debug, Clone)]
pub struct GpsLocation {
    pub latitude: f64,
    pub longitude: f64,
}

/// Viewer mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewerMode {
    Image,
    ImageFullscreen,
    Video,
    VideoFullscreen,
    Audio,
    Gallery,
}

/// Slideshow state
#[derive(Debug, Clone)]
pub struct SlideshowState {
    /// Is playing
    pub playing: bool,
    /// Interval in seconds
    pub interval_secs: u32,
    /// Elapsed time
    pub elapsed: f32,
}

/// Viewer settings
#[derive(Debug, Clone)]
pub struct ViewerSettings {
    /// Slideshow interval (seconds)
    pub slideshow_interval: u32,
    /// Loop playlist
    pub loop_playlist: bool,
    /// Shuffle playlist
    pub shuffle: bool,
    /// Background color
    pub background: String,
    /// Show info overlay
    pub show_info: bool,
    /// Auto-rotate based on EXIF
    pub auto_rotate: bool,
}

impl Default for ViewerSettings {
    fn default() -> Self {
        Self {
            slideshow_interval: 5,
            loop_playlist: true,
            shuffle: false,
            background: String::from("#000000"),
            show_info: false,
            auto_rotate: true,
        }
    }
}

// =============================================================================
// Video Player
// =============================================================================

/// Video player controls
#[derive(Debug, Clone)]
pub struct VideoPlayer {
    /// Is playing
    pub playing: bool,
    /// Current time (seconds)
    pub current_time: f64,
    /// Duration (seconds)
    pub duration: f64,
    /// Volume (0-1)
    pub volume: f32,
    /// Is muted
    pub muted: bool,
    /// Playback rate
    pub playback_rate: f32,
    /// Is buffering
    pub buffering: bool,
    /// Buffered ranges
    pub buffered: Vec<(f64, f64)>,
    /// Subtitles
    pub subtitles: Vec<SubtitleTrack>,
    /// Active subtitle track
    pub active_subtitle: Option<usize>,
    /// Audio tracks
    pub audio_tracks: Vec<AudioTrack>,
    /// Active audio track
    pub active_audio: usize,
    /// Show controls
    pub show_controls: bool,
}

impl Default for VideoPlayer {
    fn default() -> Self {
        Self {
            playing: false,
            current_time: 0.0,
            duration: 0.0,
            volume: 1.0,
            muted: false,
            playback_rate: 1.0,
            buffering: false,
            buffered: Vec::new(),
            subtitles: Vec::new(),
            active_subtitle: None,
            audio_tracks: Vec::new(),
            active_audio: 0,
            show_controls: true,
        }
    }
}

impl VideoPlayer {
    /// Play
    pub fn play(&mut self) {
        self.playing = true;
    }

    /// Pause
    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// Toggle play/pause
    pub fn toggle_play(&mut self) {
        self.playing = !self.playing;
    }

    /// Seek to time
    pub fn seek(&mut self, time: f64) {
        self.current_time = time.max(0.0).min(self.duration);
    }

    /// Seek forward
    pub fn seek_forward(&mut self, seconds: f64) {
        self.seek(self.current_time + seconds);
    }

    /// Seek backward
    pub fn seek_backward(&mut self, seconds: f64) {
        self.seek(self.current_time - seconds);
    }

    /// Set volume
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.max(0.0).min(1.0);
        if self.volume > 0.0 {
            self.muted = false;
        }
    }

    /// Toggle mute
    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
    }

    /// Set playback rate
    pub fn set_playback_rate(&mut self, rate: f32) {
        self.playback_rate = rate.max(0.25).min(4.0);
    }

    /// Format time
    pub fn format_time(seconds: f64) -> String {
        let total_secs = seconds as u64;
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let secs = total_secs % 60;

        if hours > 0 {
            alloc::format!("{}:{:02}:{:02}", hours, minutes, secs)
        } else {
            alloc::format!("{}:{:02}", minutes, secs)
        }
    }

    /// Progress percentage
    pub fn progress(&self) -> f64 {
        if self.duration > 0.0 {
            self.current_time / self.duration
        } else {
            0.0
        }
    }
}

/// Subtitle track
#[derive(Debug, Clone)]
pub struct SubtitleTrack {
    /// Track ID
    pub id: usize,
    /// Language
    pub language: String,
    /// Label
    pub label: String,
    /// Track type
    pub kind: SubtitleKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubtitleKind {
    #[default]
    Subtitles,
    Captions,
    Descriptions,
}

/// Audio track
#[derive(Debug, Clone)]
pub struct AudioTrack {
    /// Track ID
    pub id: usize,
    /// Language
    pub language: String,
    /// Label
    pub label: String,
}

// =============================================================================
// Image Editor (basic)
// =============================================================================

/// Basic image adjustments
#[derive(Debug, Clone)]
pub struct ImageAdjustments {
    /// Brightness (-100 to 100)
    pub brightness: i32,
    /// Contrast (-100 to 100)
    pub contrast: i32,
    /// Saturation (-100 to 100)
    pub saturation: i32,
    /// Hue rotation (degrees)
    pub hue: i32,
    /// Sharpness (0 to 100)
    pub sharpness: i32,
    /// Blur (0 to 100)
    pub blur: i32,
}

impl Default for ImageAdjustments {
    fn default() -> Self {
        Self {
            brightness: 0,
            contrast: 0,
            saturation: 0,
            hue: 0,
            sharpness: 0,
            blur: 0,
        }
    }
}

impl ImageAdjustments {
    /// Reset all adjustments
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Has any adjustments
    pub fn has_adjustments(&self) -> bool {
        self.brightness != 0
            || self.contrast != 0
            || self.saturation != 0
            || self.hue != 0
            || self.sharpness != 0
            || self.blur != 0
    }
}

/// Image crop
#[derive(Debug, Clone)]
pub struct Crop {
    /// X offset
    pub x: u32,
    /// Y offset
    pub y: u32,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
}
