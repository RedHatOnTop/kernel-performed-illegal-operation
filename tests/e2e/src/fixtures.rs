//! Test fixtures and data factories
//!
//! Provides reusable test data and setup utilities.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

/// HTML test fixtures
pub struct HtmlFixtures;

impl HtmlFixtures {
    /// Basic HTML page
    pub fn basic_page() -> String {
        String::from(r#"<!DOCTYPE html>
<html>
<head>
    <title>Test Page</title>
</head>
<body>
    <h1>Hello World</h1>
    <p>This is a test page.</p>
</body>
</html>"#)
    }

    /// Page with forms
    pub fn form_page() -> String {
        String::from(r#"<!DOCTYPE html>
<html>
<head>
    <title>Form Test</title>
</head>
<body>
    <form id="test-form" action="/submit" method="post">
        <label for="username">Username:</label>
        <input type="text" id="username" name="username" required>
        
        <label for="password">Password:</label>
        <input type="password" id="password" name="password" required>
        
        <label for="email">Email:</label>
        <input type="email" id="email" name="email">
        
        <label>
            <input type="checkbox" id="remember" name="remember">
            Remember me
        </label>
        
        <select id="country" name="country">
            <option value="">Select country</option>
            <option value="us">United States</option>
            <option value="uk">United Kingdom</option>
            <option value="jp">Japan</option>
        </select>
        
        <button type="submit">Submit</button>
    </form>
</body>
</html>"#)
    }

    /// Page with complex layout
    pub fn layout_page() -> String {
        String::from(r#"<!DOCTYPE html>
<html>
<head>
    <title>Layout Test</title>
    <style>
        .container { display: flex; }
        .sidebar { width: 200px; background: #f0f0f0; }
        .main { flex: 1; padding: 20px; }
        .header { background: #333; color: white; padding: 10px; }
        .footer { background: #333; color: white; padding: 10px; text-align: center; }
    </style>
</head>
<body>
    <header class="header">
        <nav>
            <a href="/">Home</a>
            <a href="/about">About</a>
            <a href="/contact">Contact</a>
        </nav>
    </header>
    <div class="container">
        <aside class="sidebar">
            <ul>
                <li><a href="\u{0023}section1">Section 1</a></li>
                <li><a href="\u{0023}section2">Section 2</a></li>
                <li><a href="\u{0023}section3">Section 3</a></li>
            </ul>
        </aside>
        <main class="main">
            <h1>Main Content</h1>
            <p>Lorem ipsum dolor sit amet.</p>
        </main>
    </div>
    <footer class="footer">
        &copy; 2024 Test Site
    </footer>
</body>
</html>"#)
    }

    /// Page with interactive elements
    pub fn interactive_page() -> String {
        String::from(r#"<!DOCTYPE html>
<html>
<head>
    <title>Interactive Test</title>
    <style>
        .modal { display: none; position: fixed; top: 50%; left: 50%; transform: translate(-50%, -50%); 
                 background: white; padding: 20px; border: 1px solid #ccc; z-index: 100; }
        .modal.visible { display: block; }
        .overlay { display: none; position: fixed; top: 0; left: 0; right: 0; bottom: 0; 
                   background: rgba(0,0,0,0.5); z-index: 99; }
        .overlay.visible { display: block; }
        .dropdown { position: relative; }
        .dropdown-menu { display: none; position: absolute; background: white; border: 1px solid #ccc; }
        .dropdown.open .dropdown-menu { display: block; }
        .tab-content { display: none; }
        .tab-content.active { display: block; }
    </style>
</head>
<body>
    <button id="open-modal">Open Modal</button>
    <div class="overlay" id="overlay"></div>
    <div class="modal" id="modal">
        <h2>Modal Title</h2>
        <p>Modal content here.</p>
        <button id="close-modal">Close</button>
    </div>
    
    <div class="dropdown" id="dropdown">
        <button id="dropdown-toggle">Dropdown</button>
        <div class="dropdown-menu">
            <a href="\u{0023}">Option 1</a>
            <a href="\u{0023}">Option 2</a>
            <a href="\u{0023}">Option 3</a>
        </div>
    </div>
    
    <div class="tabs">
        <button class="tab" data-tab="tab1">Tab 1</button>
        <button class="tab" data-tab="tab2">Tab 2</button>
        <button class="tab" data-tab="tab3">Tab 3</button>
    </div>
    <div id="tab1" class="tab-content active">Tab 1 content</div>
    <div id="tab2" class="tab-content">Tab 2 content</div>
    <div id="tab3" class="tab-content">Tab 3 content</div>
    
    <script>
        document.getElementById('open-modal').onclick = function() {
            document.getElementById('modal').classList.add('visible');
            document.getElementById('overlay').classList.add('visible');
        };
        document.getElementById('close-modal').onclick = function() {
            document.getElementById('modal').classList.remove('visible');
            document.getElementById('overlay').classList.remove('visible');
        };
        document.getElementById('dropdown-toggle').onclick = function() {
            document.getElementById('dropdown').classList.toggle('open');
        };
        document.querySelectorAll('.tab').forEach(function(tab) {
            tab.onclick = function() {
                document.querySelectorAll('.tab-content').forEach(function(c) { 
                    c.classList.remove('active'); 
                });
                document.getElementById(tab.dataset.tab).classList.add('active');
            };
        });
    </script>
</body>
</html>"#)
    }

    /// Page with images
    pub fn image_page() -> String {
        String::from(r#"<!DOCTYPE html>
<html>
<head>
    <title>Image Test</title>
</head>
<body>
    <h1>Image Gallery</h1>
    <div class="gallery">
        <img src="/images/test1.jpg" alt="Test Image 1" width="200" height="150">
        <img src="/images/test2.jpg" alt="Test Image 2" width="200" height="150">
        <img src="/images/test3.jpg" alt="Test Image 3" loading="lazy" width="200" height="150">
    </div>
    <picture>
        <source media="(min-width: 800px)" srcset="/images/large.jpg">
        <source media="(min-width: 400px)" srcset="/images/medium.jpg">
        <img src="/images/small.jpg" alt="Responsive Image">
    </picture>
</body>
</html>"#)
    }

    /// Page with tables
    pub fn table_page() -> String {
        String::from(r#"<!DOCTYPE html>
<html>
<head>
    <title>Table Test</title>
    <style>
        table { border-collapse: collapse; width: 100%; }
        th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }
        th { background-color: #4CAF50; color: white; }
        tr:nth-child(even) { background-color: #f2f2f2; }
    </style>
</head>
<body>
    <table id="data-table">
        <thead>
            <tr>
                <th>Name</th>
                <th>Age</th>
                <th>City</th>
            </tr>
        </thead>
        <tbody>
            <tr><td>Alice</td><td>25</td><td>New York</td></tr>
            <tr><td>Bob</td><td>30</td><td>London</td></tr>
            <tr><td>Charlie</td><td>35</td><td>Tokyo</td></tr>
        </tbody>
    </table>
</body>
</html>"#)
    }
}

/// URL test fixtures
pub struct UrlFixtures;

impl UrlFixtures {
    /// Local test server base URL
    pub fn local_base() -> String {
        String::from("http://localhost:8080")
    }

    /// Generate a local test URL
    pub fn local(path: &str) -> String {
        format!("http://localhost:8080{}", path)
    }

    /// Example URLs for testing
    pub fn example_urls() -> Vec<String> {
        alloc::vec![
            String::from("https://example.com"),
            String::from("https://example.org"),
            String::from("https://www.example.net/path?query=value"),
        ]
    }

    /// Various URL formats for parsing tests
    pub fn url_formats() -> Vec<(String, UrlComponents)> {
        alloc::vec![
            (
                String::from("https://user:pass@example.com:8080/path?query=value#fragment"),
                UrlComponents {
                    scheme: String::from("https"),
                    username: Some(String::from("user")),
                    password: Some(String::from("pass")),
                    host: String::from("example.com"),
                    port: Some(8080),
                    path: String::from("/path"),
                    query: Some(String::from("query=value")),
                    fragment: Some(String::from("fragment")),
                }
            ),
            (
                String::from("http://localhost/"),
                UrlComponents {
                    scheme: String::from("http"),
                    username: None,
                    password: None,
                    host: String::from("localhost"),
                    port: None,
                    path: String::from("/"),
                    query: None,
                    fragment: None,
                }
            ),
        ]
    }
}

/// URL components for testing
#[derive(Debug, Clone)]
pub struct UrlComponents {
    pub scheme: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub host: String,
    pub port: Option<u16>,
    pub path: String,
    pub query: Option<String>,
    pub fragment: Option<String>,
}

/// Cookie test fixtures
pub struct CookieFixtures;

impl CookieFixtures {
    /// Simple session cookie
    pub fn session_cookie() -> Cookie {
        Cookie {
            name: String::from("session"),
            value: String::from("abc123"),
            domain: String::from("example.com"),
            path: String::from("/"),
            secure: true,
            http_only: true,
            same_site: SameSite::Strict,
            expires: None,
        }
    }

    /// Persistent cookie
    pub fn persistent_cookie() -> Cookie {
        Cookie {
            name: String::from("remember"),
            value: String::from("user123"),
            domain: String::from("example.com"),
            path: String::from("/"),
            secure: true,
            http_only: false,
            same_site: SameSite::Lax,
            expires: Some(1735689600), // Some future timestamp
        }
    }
}

/// Cookie for testing
#[derive(Debug, Clone)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: SameSite,
    pub expires: Option<u64>,
}

/// SameSite attribute
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSite {
    Strict,
    Lax,
    None,
}

/// User data fixtures
pub struct UserFixtures;

impl UserFixtures {
    /// Generate test users
    pub fn test_users() -> Vec<TestUser> {
        alloc::vec![
            TestUser {
                username: String::from("alice"),
                email: String::from("alice@example.com"),
                password: String::from("password123"),
            },
            TestUser {
                username: String::from("bob"),
                email: String::from("bob@example.com"),
                password: String::from("secret456"),
            },
            TestUser {
                username: String::from("charlie"),
                email: String::from("charlie@example.com"),
                password: String::from("p@ssw0rd!"),
            },
        ]
    }

    /// Invalid user data for testing validation
    pub fn invalid_users() -> Vec<(TestUser, &'static str)> {
        alloc::vec![
            (
                TestUser {
                    username: String::new(),
                    email: String::from("test@example.com"),
                    password: String::from("password"),
                },
                "empty username"
            ),
            (
                TestUser {
                    username: String::from("test"),
                    email: String::from("not-an-email"),
                    password: String::from("password"),
                },
                "invalid email"
            ),
            (
                TestUser {
                    username: String::from("test"),
                    email: String::from("test@example.com"),
                    password: String::from("short"),
                },
                "password too short"
            ),
        ]
    }
}

/// Test user data
#[derive(Debug, Clone)]
pub struct TestUser {
    pub username: String,
    pub email: String,
    pub password: String,
}

/// Network fixtures
pub struct NetworkFixtures;

impl NetworkFixtures {
    /// HTTP response headers
    pub fn typical_headers() -> Vec<(String, String)> {
        alloc::vec![
            (String::from("content-type"), String::from("text/html; charset=utf-8")),
            (String::from("cache-control"), String::from("max-age=3600")),
            (String::from("x-content-type-options"), String::from("nosniff")),
            (String::from("x-frame-options"), String::from("DENY")),
        ]
    }

    /// Security headers
    pub fn security_headers() -> Vec<(String, String)> {
        alloc::vec![
            (String::from("strict-transport-security"), String::from("max-age=31536000; includeSubDomains")),
            (String::from("content-security-policy"), String::from("default-src 'self'")),
            (String::from("x-content-type-options"), String::from("nosniff")),
            (String::from("x-frame-options"), String::from("DENY")),
            (String::from("x-xss-protection"), String::from("1; mode=block")),
        ]
    }

    /// Mock responses
    pub fn json_response() -> MockResponse {
        MockResponse {
            status: 200,
            headers: alloc::vec![
                (String::from("content-type"), String::from("application/json")),
            ],
            body: String::from(r#"{"success": true, "data": {"id": 1, "name": "Test"}}"#),
        }
    }

    /// Error response
    pub fn error_response(status: u16, message: &str) -> MockResponse {
        MockResponse {
            status,
            headers: alloc::vec![
                (String::from("content-type"), String::from("application/json")),
            ],
            body: format!(r#"{{"error": true, "message": "{}"}}"#, message),
        }
    }
}

/// Mock HTTP response
#[derive(Debug, Clone)]
pub struct MockResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

/// Bookmark fixtures
pub struct BookmarkFixtures;

impl BookmarkFixtures {
    /// Sample bookmarks
    pub fn sample_bookmarks() -> Vec<TestBookmark> {
        alloc::vec![
            TestBookmark {
                title: String::from("Example Site"),
                url: String::from("https://example.com"),
                folder: None,
            },
            TestBookmark {
                title: String::from("Rust Documentation"),
                url: String::from("https://doc.rust-lang.org"),
                folder: Some(String::from("Programming")),
            },
            TestBookmark {
                title: String::from("GitHub"),
                url: String::from("https://github.com"),
                folder: Some(String::from("Programming")),
            },
        ]
    }
}

/// Test bookmark
#[derive(Debug, Clone)]
pub struct TestBookmark {
    pub title: String,
    pub url: String,
    pub folder: Option<String>,
}

/// Test data generator
pub struct DataGenerator {
    seed: u64,
}

impl DataGenerator {
    /// Create a new generator with seed
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Generate a random-ish string (deterministic based on seed)
    pub fn string(&mut self, len: usize) -> String {
        let mut result = String::new();
        for _ in 0..len {
            self.seed = self.seed.wrapping_mul(1103515245).wrapping_add(12345);
            let c = b'a' + ((self.seed >> 16) % 26) as u8;
            result.push(c as char);
        }
        result
    }

    /// Generate a random-ish number
    pub fn number(&mut self, min: u64, max: u64) -> u64 {
        self.seed = self.seed.wrapping_mul(1103515245).wrapping_add(12345);
        min + ((self.seed >> 16) % (max - min + 1))
    }

    /// Generate an email
    pub fn email(&mut self) -> String {
        let user = self.string(8);
        format!("{}@example.com", user)
    }

    /// Generate a URL
    pub fn url(&mut self) -> String {
        let path = self.string(10);
        format!("https://example.com/{}", path)
    }
}
