//! History
//!
//! Browsing history management and search.

#![allow(dead_code)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::RwLock;

/// History entry ID.
pub type HistoryEntryId = u64;

/// History entry.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// ID.
    pub id: HistoryEntryId,
    /// URL.
    pub url: String,
    /// Title.
    pub title: String,
    /// Visit time (timestamp).
    pub last_visit_time: u64,
    /// Visit count.
    pub visit_count: u32,
    /// Typed count (from address bar).
    pub typed_count: u32,
    /// Favicon URL.
    pub favicon: Option<String>,
}

impl HistoryEntry {
    /// Create a new history entry.
    pub fn new(id: HistoryEntryId, url: &str, title: &str, visit_time: u64) -> Self {
        Self {
            id,
            url: url.to_string(),
            title: title.to_string(),
            last_visit_time: visit_time,
            visit_count: 1,
            typed_count: 0,
            favicon: None,
        }
    }
}

/// Visit entry (each visit to a URL).
#[derive(Debug, Clone)]
pub struct Visit {
    /// History entry ID.
    pub history_id: HistoryEntryId,
    /// Visit time.
    pub visit_time: u64,
    /// Transition type.
    pub transition: TransitionType,
    /// Referring visit ID.
    pub referring_visit_id: Option<u64>,
}

/// Page transition type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionType {
    /// User typed the URL.
    Typed,
    /// User clicked a link.
    Link,
    /// Auto bookmark.
    AutoBookmark,
    /// Subframe navigation.
    AutoSubframe,
    /// Manual subframe navigation.
    ManualSubframe,
    /// Generated (e.g., from search).
    Generated,
    /// User started typing.
    AutoToplevel,
    /// Form submission.
    FormSubmit,
    /// Reload.
    Reload,
    /// Keyword (search engine).
    Keyword,
    /// Keyword generated.
    KeywordGenerated,
}

impl Default for TransitionType {
    fn default() -> Self {
        Self::Link
    }
}

/// Time range.
#[derive(Debug, Clone, Copy)]
pub struct TimeRange {
    /// Start time (inclusive).
    pub start: u64,
    /// End time (exclusive).
    pub end: u64,
}

impl TimeRange {
    /// Last hour.
    pub fn last_hour(now: u64) -> Self {
        Self {
            start: now.saturating_sub(3600),
            end: now,
        }
    }

    /// Last 24 hours.
    pub fn last_day(now: u64) -> Self {
        Self {
            start: now.saturating_sub(86400),
            end: now,
        }
    }

    /// Last 7 days.
    pub fn last_week(now: u64) -> Self {
        Self {
            start: now.saturating_sub(604800),
            end: now,
        }
    }

    /// Last 4 weeks.
    pub fn last_month(now: u64) -> Self {
        Self {
            start: now.saturating_sub(2419200),
            end: now,
        }
    }

    /// All time.
    pub fn all_time() -> Self {
        Self {
            start: 0,
            end: u64::MAX,
        }
    }
}

/// Clear browsing data options.
#[derive(Debug, Clone, Default)]
pub struct ClearDataOptions {
    /// Clear browsing history.
    pub browsing_history: bool,
    /// Clear download history.
    pub download_history: bool,
    /// Clear cookies.
    pub cookies: bool,
    /// Clear cached images and files.
    pub cache: bool,
    /// Clear passwords.
    pub passwords: bool,
    /// Clear autofill form data.
    pub autofill: bool,
    /// Clear hosted app data.
    pub hosted_app_data: bool,
    /// Clear site settings.
    pub site_settings: bool,
    /// Time range.
    pub time_range: Option<TimeRange>,
}

/// History query.
#[derive(Debug, Clone, Default)]
pub struct HistoryQuery {
    /// Text to search for.
    pub text: Option<String>,
    /// Start time.
    pub start_time: Option<u64>,
    /// End time.
    pub end_time: Option<u64>,
    /// Max results.
    pub max_results: Option<usize>,
}

/// History manager.
pub struct HistoryManager {
    /// History entries by ID.
    entries: RwLock<Vec<HistoryEntry>>,
    /// Visits.
    visits: RwLock<Vec<Visit>>,
    /// Next entry ID.
    next_entry_id: RwLock<HistoryEntryId>,
    /// Next visit ID.
    next_visit_id: RwLock<u64>,
    /// Max entries to keep.
    max_entries: usize,
}

impl HistoryManager {
    /// Create a new history manager.
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(Vec::new()),
            visits: RwLock::new(Vec::new()),
            next_entry_id: RwLock::new(1),
            next_visit_id: RwLock::new(1),
            max_entries: 100_000,
        }
    }

    /// Add a visit to history.
    pub fn add_visit(
        &self,
        url: &str,
        title: &str,
        transition: TransitionType,
        visit_time: u64,
    ) -> HistoryEntryId {
        let mut entries = self.entries.write();

        // Check if URL already exists
        if let Some(entry) = entries.iter_mut().find(|e| e.url == url) {
            entry.last_visit_time = visit_time;
            entry.visit_count += 1;
            if title != entry.title && !title.is_empty() {
                entry.title = title.to_string();
            }
            if transition == TransitionType::Typed {
                entry.typed_count += 1;
            }

            let entry_id = entry.id;
            drop(entries);

            // Add visit record
            self.add_visit_record(entry_id, visit_time, transition);

            return entry_id;
        }

        // Create new entry
        let mut next_id = self.next_entry_id.write();
        let id = *next_id;
        *next_id += 1;
        drop(next_id);

        let mut entry = HistoryEntry::new(id, url, title, visit_time);
        if transition == TransitionType::Typed {
            entry.typed_count = 1;
        }

        entries.push(entry);

        // Enforce max entries
        if entries.len() > self.max_entries {
            entries.remove(0);
        }

        drop(entries);

        // Add visit record
        self.add_visit_record(id, visit_time, transition);

        id
    }

    /// Add visit record.
    fn add_visit_record(
        &self,
        history_id: HistoryEntryId,
        visit_time: u64,
        transition: TransitionType,
    ) {
        let mut next_id = self.next_visit_id.write();
        let _visit_id = *next_id;
        *next_id += 1;
        drop(next_id);

        let visit = Visit {
            history_id,
            visit_time,
            transition,
            referring_visit_id: None,
        };

        self.visits.write().push(visit);
    }

    /// Update entry title.
    pub fn update_title(&self, url: &str, title: &str) {
        if let Some(entry) = self.entries.write().iter_mut().find(|e| e.url == url) {
            entry.title = title.to_string();
        }
    }

    /// Update favicon.
    pub fn update_favicon(&self, url: &str, favicon: &str) {
        if let Some(entry) = self.entries.write().iter_mut().find(|e| e.url == url) {
            entry.favicon = Some(favicon.to_string());
        }
    }

    /// Get entry by URL.
    pub fn get_by_url(&self, url: &str) -> Option<HistoryEntry> {
        self.entries.read().iter().find(|e| e.url == url).cloned()
    }

    /// Get entry by ID.
    pub fn get_by_id(&self, id: HistoryEntryId) -> Option<HistoryEntry> {
        self.entries.read().iter().find(|e| e.id == id).cloned()
    }

    /// Search history.
    pub fn search(&self, query: &HistoryQuery) -> Vec<HistoryEntry> {
        let entries = self.entries.read();
        let mut results: Vec<HistoryEntry> = entries
            .iter()
            .filter(|entry| {
                // Text filter
                if let Some(ref text) = query.text {
                    let text = text.to_lowercase();
                    if !entry.title.to_lowercase().contains(&text)
                        && !entry.url.to_lowercase().contains(&text)
                    {
                        return false;
                    }
                }

                // Time filter
                if let Some(start) = query.start_time {
                    if entry.last_visit_time < start {
                        return false;
                    }
                }
                if let Some(end) = query.end_time {
                    if entry.last_visit_time >= end {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect();

        // Sort by visit time (most recent first)
        results.sort_by(|a, b| b.last_visit_time.cmp(&a.last_visit_time));

        // Limit results
        if let Some(max) = query.max_results {
            results.truncate(max);
        }

        results
    }

    /// Get recent history.
    pub fn get_recent(&self, max_results: usize) -> Vec<HistoryEntry> {
        self.search(&HistoryQuery {
            max_results: Some(max_results),
            ..Default::default()
        })
    }

    /// Get most visited.
    pub fn get_most_visited(&self, max_results: usize) -> Vec<HistoryEntry> {
        let entries = self.entries.read();
        let mut results: Vec<HistoryEntry> = entries.iter().cloned().collect();

        results.sort_by(|a, b| b.visit_count.cmp(&a.visit_count));
        results.truncate(max_results);

        results
    }

    /// Get frequently typed (for autocomplete).
    pub fn get_frequently_typed(&self, prefix: &str, max_results: usize) -> Vec<HistoryEntry> {
        let prefix = prefix.to_lowercase();
        let entries = self.entries.read();

        let mut results: Vec<HistoryEntry> = entries
            .iter()
            .filter(|e| {
                e.typed_count > 0
                    && (e.url.to_lowercase().contains(&prefix)
                        || e.title.to_lowercase().contains(&prefix))
            })
            .cloned()
            .collect();

        // Sort by typed count * visit count (frecency-like)
        results.sort_by(|a, b| {
            let score_a = (a.typed_count as u64) * (a.visit_count as u64);
            let score_b = (b.typed_count as u64) * (b.visit_count as u64);
            score_b.cmp(&score_a)
        });

        results.truncate(max_results);
        results
    }

    /// Delete history entry.
    pub fn delete_entry(&self, id: HistoryEntryId) {
        self.entries.write().retain(|e| e.id != id);
        self.visits.write().retain(|v| v.history_id != id);
    }

    /// Delete history by URL.
    pub fn delete_by_url(&self, url: &str) {
        let entries = self.entries.read();
        let ids: Vec<HistoryEntryId> = entries
            .iter()
            .filter(|e| e.url == url)
            .map(|e| e.id)
            .collect();
        drop(entries);

        for id in ids {
            self.delete_entry(id);
        }
    }

    /// Clear history in time range.
    pub fn clear_range(&self, range: TimeRange) {
        let mut entries = self.entries.write();
        let mut visits = self.visits.write();

        // Find entries to remove
        let ids_to_remove: Vec<HistoryEntryId> = entries
            .iter()
            .filter(|e| e.last_visit_time >= range.start && e.last_visit_time < range.end)
            .map(|e| e.id)
            .collect();

        entries.retain(|e| !ids_to_remove.contains(&e.id));
        visits.retain(|v| !ids_to_remove.contains(&v.history_id));
    }

    /// Clear all history.
    pub fn clear_all(&self) {
        self.entries.write().clear();
        self.visits.write().clear();
    }

    /// Get total entry count.
    pub fn count(&self) -> usize {
        self.entries.read().len()
    }

    /// Get visits for entry.
    pub fn get_visits(&self, history_id: HistoryEntryId) -> Vec<Visit> {
        self.visits
            .read()
            .iter()
            .filter(|v| v.history_id == history_id)
            .cloned()
            .collect()
    }
}

impl Default for HistoryManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_visit() {
        let manager = HistoryManager::new();

        let id = manager.add_visit(
            "https://example.com",
            "Example",
            TransitionType::Typed,
            1000,
        );

        let entry = manager.get_by_id(id).unwrap();
        assert_eq!(entry.url, "https://example.com");
        assert_eq!(entry.title, "Example");
        assert_eq!(entry.visit_count, 1);
        assert_eq!(entry.typed_count, 1);
    }

    #[test]
    fn test_revisit() {
        let manager = HistoryManager::new();

        manager.add_visit(
            "https://example.com",
            "Example",
            TransitionType::Typed,
            1000,
        );
        manager.add_visit(
            "https://example.com",
            "Example Updated",
            TransitionType::Link,
            2000,
        );

        let entry = manager.get_by_url("https://example.com").unwrap();
        assert_eq!(entry.visit_count, 2);
        assert_eq!(entry.typed_count, 1);
        assert_eq!(entry.title, "Example Updated");
    }

    #[test]
    fn test_search() {
        let manager = HistoryManager::new();

        manager.add_visit("https://google.com", "Google", TransitionType::Typed, 1000);
        manager.add_visit(
            "https://rust-lang.org",
            "Rust Language",
            TransitionType::Link,
            2000,
        );

        let results = manager.search(&HistoryQuery {
            text: Some("rust".to_string()),
            ..Default::default()
        });

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Language");
    }

    #[test]
    fn test_clear_range() {
        let manager = HistoryManager::new();

        manager.add_visit("https://old.com", "Old", TransitionType::Link, 1000);
        manager.add_visit("https://new.com", "New", TransitionType::Link, 2000);

        manager.clear_range(TimeRange {
            start: 0,
            end: 1500,
        });

        assert!(manager.get_by_url("https://old.com").is_none());
        assert!(manager.get_by_url("https://new.com").is_some());
    }
}
