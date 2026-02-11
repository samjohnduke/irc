# FILEHOST: File Upload Extension

Server-side file upload for sharing images and files via IRC.

## Overview

FILEHOST provides an HTTP endpoint for clients to upload files. The server stores the file and returns a URL that can be shared in IRC messages.

```
┌────────┐                           ┌────────────────────┐
│ Client │                           │    irc-server      │
│        │                           │                    │
│        │──── POST /upload ────────►│  ┌──────────────┐  │
│        │     [file data]           │  │ HTTP Handler │  │
│        │                           │  └──────┬───────┘  │
│        │◄─── 201 Created ──────────│         │          │
│        │     Location: https://... │  ┌──────▼───────┐  │
│        │                           │  │   Storage    │  │
│        │                           │  └──────────────┘  │
└────────┘                           └────────────────────┘
     │
     │ PRIVMSG #channel :Check this out: https://irc.example.com/f/abc123.jpg
     ▼
┌────────┐
│ Other  │  Sees URL, client can preview
│Clients │
└────────┘
```

## Protocol

### Capability Advertisement

The server advertises FILEHOST support via ISUPPORT:

```
:server 005 nick FILEHOST=https://irc.example.com/upload :are supported
```

The value is the upload endpoint URL.

### Upload Request

```http
POST /upload HTTP/1.1
Host: irc.example.com
Authorization: Bearer <session-token>
Content-Type: image/jpeg
Content-Length: 102400
Content-Disposition: attachment; filename="photo.jpg"

<binary file data>
```

#### Required Headers

| Header | Description |
|--------|-------------|
| `Authorization` | Bearer token from SASL or session |
| `Content-Type` | MIME type of the file |
| `Content-Length` | File size in bytes |

#### Optional Headers

| Header | Description |
|--------|-------------|
| `Content-Disposition` | Original filename |
| `X-Expire-After` | Requested retention (seconds) |

### Upload Response

#### Success (201 Created)

```http
HTTP/1.1 201 Created
Location: https://irc.example.com/f/abc123.jpg
Content-Type: application/json

{
  "url": "https://irc.example.com/f/abc123.jpg",
  "id": "abc123",
  "size": 102400,
  "type": "image/jpeg",
  "expires": "2024-02-15T10:30:00Z"
}
```

#### Errors

| Status | Meaning |
|--------|---------|
| 400 | Bad request (missing headers, invalid file) |
| 401 | Not authenticated |
| 403 | Upload not permitted for this user |
| 413 | File too large |
| 415 | Unsupported media type |
| 507 | Storage quota exceeded |

### File Retrieval

```http
GET /f/abc123.jpg HTTP/1.1
Host: irc.example.com
```

Response serves the file with appropriate `Content-Type`.

## Server Implementation

### Configuration

```toml
[filehost]
enabled = true

# Public URL base for generated links
public_url = "https://irc.example.com"

# Upload endpoint path
upload_path = "/upload"

# File serving path
serve_path = "/f"

# Storage backend
[filehost.storage]
driver = "filesystem"  # or "s3"
path = "/var/lib/irc/uploads"

# Limits
[filehost.limits]
max_file_size = "10MB"
max_total_storage = "1GB"  # Per user
allowed_types = [
    "image/jpeg",
    "image/png",
    "image/gif",
    "image/webp",
    "video/mp4",
    "video/webm",
    "audio/mpeg",
    "audio/ogg",
    "text/plain",
    "application/pdf",
]

# Retention
[filehost.retention]
default_days = 30       # Files expire after 30 days
max_days = 365          # Maximum retention
cleanup_interval = "1h" # Check for expired files
```

### Storage Backends

#### Filesystem

```rust
pub struct FilesystemStorage {
    base_path: PathBuf,
}

impl Storage for FilesystemStorage {
    async fn store(&self, id: &str, data: Bytes, meta: &FileMeta) -> Result<()> {
        let path = self.base_path.join(id);
        tokio::fs::write(&path, data).await?;

        // Store metadata alongside
        let meta_path = self.base_path.join(format!("{}.meta", id));
        let meta_json = serde_json::to_vec(meta)?;
        tokio::fs::write(&meta_path, meta_json).await?;

        Ok(())
    }

    async fn retrieve(&self, id: &str) -> Result<(Bytes, FileMeta)> {
        let path = self.base_path.join(id);
        let data = tokio::fs::read(&path).await?;

        let meta_path = self.base_path.join(format!("{}.meta", id));
        let meta_json = tokio::fs::read(&meta_path).await?;
        let meta: FileMeta = serde_json::from_slice(&meta_json)?;

        Ok((Bytes::from(data), meta))
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let path = self.base_path.join(id);
        tokio::fs::remove_file(&path).await?;

        let meta_path = self.base_path.join(format!("{}.meta", id));
        let _ = tokio::fs::remove_file(&meta_path).await;

        Ok(())
    }
}
```

#### S3-Compatible

```rust
pub struct S3Storage {
    client: aws_sdk_s3::Client,
    bucket: String,
    prefix: String,
}

impl Storage for S3Storage {
    async fn store(&self, id: &str, data: Bytes, meta: &FileMeta) -> Result<()> {
        let key = format!("{}/{}", self.prefix, id);

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(data.into())
            .content_type(&meta.content_type)
            .metadata("filename", &meta.filename)
            .metadata("uploader", &meta.uploader)
            .metadata("expires", &meta.expires.to_rfc3339())
            .send()
            .await?;

        Ok(())
    }

    // ... retrieve, delete
}
```

### HTTP Handler

```rust
use axum::{
    extract::{State, TypedHeader},
    headers::{Authorization, ContentType, ContentLength},
    response::Json,
    http::StatusCode,
};

pub async fn handle_upload(
    State(state): State<AppState>,
    auth: TypedHeader<Authorization<Bearer>>,
    content_type: TypedHeader<ContentType>,
    content_length: TypedHeader<ContentLength>,
    body: Bytes,
) -> Result<(StatusCode, Json<UploadResponse>), UploadError> {
    // Authenticate
    let account = state.auth.verify_token(auth.token())
        .await
        .map_err(|_| UploadError::Unauthorized)?;

    // Validate content type
    let mime = content_type.0.to_string();
    if !state.config.filehost.is_allowed_type(&mime) {
        return Err(UploadError::UnsupportedType);
    }

    // Validate size
    let size = content_length.0 as usize;
    if size > state.config.filehost.max_file_size {
        return Err(UploadError::TooLarge);
    }

    // Check quota
    let used = state.storage.get_usage(&account).await?;
    if used + size > state.config.filehost.max_total_storage {
        return Err(UploadError::QuotaExceeded);
    }

    // Generate ID
    let id = generate_file_id();

    // Determine expiration
    let expires = Utc::now() + Duration::days(
        state.config.filehost.retention.default_days as i64
    );

    // Store file
    let meta = FileMeta {
        filename: extract_filename(&content_disposition),
        content_type: mime.clone(),
        size,
        uploader: account.clone(),
        uploaded_at: Utc::now(),
        expires,
    };

    state.storage.store(&id, body, &meta).await?;

    // Record in database
    state.db.record_upload(&id, &meta).await?;

    // Build response
    let url = format!(
        "{}{}/{}",
        state.config.filehost.public_url,
        state.config.filehost.serve_path,
        id
    );

    Ok((
        StatusCode::CREATED,
        Json(UploadResponse {
            url: url.clone(),
            id,
            size,
            content_type: mime,
            expires,
        }),
    ))
}

pub async fn handle_serve(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let (data, meta) = state.storage.retrieve(&id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // Check expiration
    if Utc::now() > meta.expires {
        // Expired, delete and return 404
        let _ = state.storage.delete(&id).await;
        return Err(StatusCode::NOT_FOUND);
    }

    Ok((
        [
            (header::CONTENT_TYPE, meta.content_type),
            (header::CACHE_CONTROL, "public, max-age=86400".to_string()),
        ],
        data,
    ))
}
```

### Authentication

Uploads require authentication. Options:

#### 1. Session Token (Cookie)

Web-based clients can use session cookies.

```rust
// On successful SASL, issue session token
pub async fn issue_upload_token(account: &str) -> String {
    let claims = UploadClaims {
        sub: account.to_string(),
        exp: Utc::now() + Duration::hours(24),
        iat: Utc::now(),
    };

    jsonwebtoken::encode(&Header::default(), &claims, &ENCODING_KEY)
}
```

#### 2. Bearer Token (API)

Clients can request a token via IRC:

```
Client: FILEHOST TOKEN
Server: :server NOTICE client :Your upload token: eyJ...
```

#### 3. Basic Auth with Account Credentials

Simple but requires sending password:

```http
Authorization: Basic base64(username:password)
```

### Database Schema

Track uploads for quota and cleanup:

```sql
CREATE TABLE uploads (
    id TEXT PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts(id),
    filename TEXT,
    content_type TEXT NOT NULL,
    size INTEGER NOT NULL,
    uploaded_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    deleted INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_uploads_account ON uploads(account_id);
CREATE INDEX idx_uploads_expires ON uploads(expires_at) WHERE deleted = 0;
```

### Cleanup Task

```rust
pub async fn cleanup_expired_files(state: &AppState) {
    loop {
        tokio::time::sleep(state.config.filehost.retention.cleanup_interval).await;

        let expired = state.db.get_expired_uploads().await?;

        for upload in expired {
            if let Err(e) = state.storage.delete(&upload.id).await {
                tracing::warn!("Failed to delete {}: {}", upload.id, e);
                continue;
            }

            state.db.mark_deleted(&upload.id).await?;
            tracing::info!("Deleted expired file: {}", upload.id);
        }
    }
}
```

## Client Implementation

### Upload Flow

```rust
impl Client {
    /// Upload a file and return the URL
    pub async fn upload_file(&self, path: &Path) -> Result<String, UploadError> {
        // Get upload endpoint from ISUPPORT
        let endpoint = self.session.isupport.filehost
            .as_ref()
            .ok_or(UploadError::NotSupported)?;

        // Read file
        let data = tokio::fs::read(path).await?;
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file");

        // Detect content type
        let content_type = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();

        // Get auth token
        let token = self.get_upload_token().await?;

        // Upload
        let response = reqwest::Client::new()
            .post(endpoint)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", &content_type)
            .header("Content-Disposition", format!("attachment; filename=\"{}\"", filename))
            .body(data)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(UploadError::ServerError(response.status()));
        }

        let result: UploadResponse = response.json().await?;
        Ok(result.url)
    }
}
```

### GUI Integration

```rust
// Drag-and-drop handler
pub fn handle_drop(&mut self, files: Vec<PathBuf>) -> Command<Message> {
    if files.is_empty() {
        return Command::none();
    }

    // Upload each file
    let commands: Vec<_> = files.into_iter().map(|path| {
        Command::perform(
            self.client.upload_file(path.clone()),
            move |result| match result {
                Ok(url) => Message::FileUploaded { path, url },
                Err(e) => Message::UploadFailed { path, error: e.to_string() },
            }
        )
    }).collect();

    Command::batch(commands)
}

// On successful upload, insert URL into input
fn handle_file_uploaded(&mut self, url: String) {
    // Append URL to current input
    if !self.input.is_empty() {
        self.input.push(' ');
    }
    self.input.push_str(&url);
}
```

### CLI Integration

```rust
// /upload command
pub fn handle_upload_command(&mut self, args: &str) -> Result<(), Error> {
    let path = PathBuf::from(args.trim());

    if !path.exists() {
        self.show_error("File not found");
        return Ok(());
    }

    // Start upload in background
    let client = self.client.clone();
    let buffer = self.active_buffer.clone();

    tokio::spawn(async move {
        match client.upload_file(&path).await {
            Ok(url) => {
                // Send URL to current channel/query
                if let Some(target) = buffer.target() {
                    client.privmsg(&target, &url).await?;
                }
            }
            Err(e) => {
                // Show error to user
            }
        }
    });

    self.show_status("Uploading...");
    Ok(())
}
```

## Security Considerations

1. **Authentication required** - Only logged-in users can upload
2. **Content-Type validation** - Check magic bytes, not just header
3. **Filename sanitization** - Never use user filename directly in paths
4. **Size limits** - Prevent resource exhaustion
5. **Quota enforcement** - Per-user storage limits
6. **Virus scanning** - Optional integration with ClamAV
7. **Rate limiting** - Prevent upload spam
8. **HTTPS only** - Encrypt uploads in transit

### Content-Type Verification

Don't trust the `Content-Type` header alone:

```rust
use infer;

fn verify_content_type(data: &[u8], claimed: &str) -> Result<String, Error> {
    let detected = infer::get(data)
        .map(|t| t.mime_type())
        .unwrap_or("application/octet-stream");

    // Allow if detected matches claimed (or is more specific)
    if detected == claimed || claimed == "application/octet-stream" {
        Ok(detected.to_string())
    } else {
        Err(Error::ContentTypeMismatch {
            claimed: claimed.to_string(),
            detected: detected.to_string(),
        })
    }
}
```

## Integration with IRC

### Sharing Uploaded Files

After upload, user shares the URL:

```
PRIVMSG #channel :Check out this photo: https://irc.example.com/f/abc123.jpg
```

### URL Preview in Clients

Clients can detect FILEHOST URLs and show inline previews:

```rust
fn is_our_filehost_url(&self, url: &str) -> bool {
    url.starts_with(&self.session.isupport.filehost_base)
}

fn render_message(&self, msg: &Message) -> Element {
    // Extract URLs
    let urls = extract_urls(&msg.text);

    for url in urls {
        if self.is_our_filehost_url(&url) {
            // Show inline preview
            return self.render_with_preview(msg, &url);
        }
    }

    self.render_plain(msg)
}
```

## References

- [soju FILEHOST implementation](https://soju.im/doc/soju.1.html)
- [convoyeur - FILEHOST adapter](https://github.com/classabbyamp/convoyeur)
- [IRCv3 file upload discussion](https://github.com/ircv3/ircv3-ideas/issues/100)
