//! fs: the file server — read, write, edit, list.
//!
//! Paths work like OpenCode's file tools: absolute paths go straight through,
//! relative paths resolve against the base directory (argv[1],
//! `BIONICHAT_FS_ROOT`, or the working directory). Reads are read-only; write
//! and edit are mutating, but the hub does not gate them: calls run at once.

use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
        ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
        ToolAnnotations,
    },
    service::RequestContext,
    transport::stdio,
};
use serde_json::{Map, Value, json};

#[derive(Clone)]
struct FsServer {
    base: Arc<PathBuf>,
}

impl FsServer {
    /// Absolute paths pass through; anything else joins the base. `..` that
    /// would climb above the anchor is rejected.
    fn resolve(&self, requested: &str) -> Result<PathBuf, String> {
        let requested = requested.trim();
        if requested.is_empty() {
            return Err("empty path".to_string());
        }
        let raw = Path::new(requested);
        if raw.is_absolute() {
            return Ok(raw.to_path_buf());
        }
        let mut candidate = self.base.as_path().to_path_buf();
        for part in raw.components() {
            match part {
                Component::Normal(piece) => candidate.push(piece),
                Component::CurDir => {}
                _ => return Err(format!("`{requested}` climbs above the base directory")),
            }
        }
        Ok(candidate)
    }
}

fn schema(properties: Value, required: &[&str]) -> Arc<Map<String, Value>> {
    let mut obj = json!({
        "type": "object",
        "properties": properties,
        "additionalProperties": false,
    });
    if !required.is_empty() {
        obj["required"] = json!(required);
    }
    Arc::new(obj.as_object().expect("object schema").clone())
}

/// Biggest image the tool will return; a chat client has to carry it inline.
const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;

/// Biggest PDF the tool will open. Text is extracted from the bytes, so this
/// is a memory guard, not an upload limit.
const MAX_PDF_BYTES: usize = 32 * 1024 * 1024;

/// Extracted PDF text is clamped: a 500 page report would otherwise eat the
/// whole context window in one call.
const MAX_PDF_CHARS: usize = 60_000;

fn is_pdf(path: &std::path::Path, bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF-")
        || path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
}

/// MIME type for a file the model can look at, by extension.
fn image_mime(path: &std::path::Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        _ => return None,
    })
}

/// Text out of a Word document: paragraphs, tabs and table cells, in order.
/// .docx is a zip with word/document.xml inside; nothing else is needed.
fn docx_text(bytes: &[u8]) -> Result<String, String> {
    // A .docx that starts with the OLE header is an encrypted file, or an old
    // .doc someone renamed; a real one is a zip and starts with PK.
    if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) {
        return Err(
            "it is password protected or an old .doc renamed to .docx: encrypted Office files cannot be read"
                .to_string(),
        );
    }
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|error| error.to_string())?;
    let mut document = archive
        .by_name("word/document.xml")
        .map_err(|_| "not a Word document: no word/document.xml inside".to_string())?;
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut document, &mut xml).map_err(|error| error.to_string())?;

    let mut reader = quick_xml::Reader::from_str(&xml);
    let mut out = String::new();
    let mut in_text = false;
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(event)) => match event.local_name().as_ref() {
                b"t" => in_text = true,
                b"tab" => out.push('\t'),
                b"br" | b"cr" => out.push('\n'),
                _ => {}
            },
            Ok(quick_xml::events::Event::End(event)) => match event.local_name().as_ref() {
                b"t" => in_text = false,
                b"p" => out.push('\n'),
                b"tc" => out.push('\t'),
                b"tr" => out.push('\n'),
                _ => {}
            },
            Ok(quick_xml::events::Event::Text(text)) if in_text => {
                if let Ok(value) = text.decode() {
                    out.push_str(&value);
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(error) => return Err(error.to_string()),
            _ => {}
        }
    }
    Ok(out)
}

/// Every sheet of an Excel workbook as rows of cells, tab separated.
fn xlsx_text(path: &std::path::Path) -> Result<String, String> {
    use calamine::Reader;
    let mut workbook = calamine::open_workbook_auto(path).map_err(|error| error.to_string())?;
    let names = workbook.sheet_names().to_owned();
    let mut out = String::new();
    for name in names {
        let Ok(range) = workbook.worksheet_range(&name) else {
            continue;
        };
        out.push_str(&format!("\n## {name}\n"));
        for row in range.rows() {
            let cells: Vec<String> = row.iter().map(|cell| cell.to_string()).collect();
            if cells.iter().all(|cell| cell.is_empty()) {
                continue;
            }
            out.push_str(&cells.join("\t"));
            out.push('\n');
        }
    }
    Ok(out)
}

fn numbered(text: &str, offset: usize) -> String {
    text.lines()
        .enumerate()
        .map(|(index, line)| format!("{}: {line}", offset + index + 1))
        .collect::<Vec<_>>()
        .join("\n")
}

impl ServerHandler for FsServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("fs", env!("CARGO_PKG_VERSION")))
            .with_instructions("File tools: read, write, edit.")
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(vec![
            Tool::new(
                "read",
                "Read the contents of a file. Supports text files, images, PDFs and Office documents (.docx, .xlsx, .xls): an image (PNG, JPEG, GIF, WebP, BMP) is returned as an image block the model can look at, up to 8 MB. A PDF, Word document or Excel workbook is returned as its extracted text, so any client can read it. Each text line is prefixed by its 1-based line number as <line>: <content>; the prefix is for reference and is not part of the file content. Use offset and limit to read large files in sections. Prefer one larger read over many small slices, and use bash with grep to find specific content in large files.",
                schema(
                    json!({
                        "path": { "type": "string", "description": "File to read" },
                        "offset": { "type": "integer", "description": "The line to start reading from (1-based). Defaults to 1." },
                        "limit": { "type": "integer", "description": "The maximum number of lines to read (defaults to and capped at 10000)" }
                    }),
                    &["path"],
                ),
            )
            .with_annotations(ToolAnnotations::new().read_only(true)),
            Tool::new(
                "write",
                "Writes a file to the local filesystem, overwriting if one exists. Missing parent directories are created automatically. Use this tool to create new files or overwrite existing files. For partial changes, use the edit tool instead.",
                schema(
                    json!({
                        "path": { "type": "string", "description": "Path to the file to write to" },
                        "content": { "type": "string", "description": "Content to write to the file" }
                    }),
                    &["path", "content"],
                ),
            ),
            Tool::new(
                "edit",
                "Performs exact string replacements in files. oldString must match the file exactly, including whitespace and indentation. When replaceAll is false, oldString must appear exactly once; otherwise the call fails and nothing is written.",
                schema(
                    json!({
                        "path": { "type": "string", "description": "File to edit" },
                        "oldString": { "type": "string", "description": "Exact text to find and replace" },
                        "newString": { "type": "string", "description": "Text to replace oldString with (must differ from oldString)" },
                        "replaceAll": { "type": "boolean", "description": "Whether to replace every occurrence of oldString. When false, oldString must match exactly once. Defaults to false." }
                    }),
                    &["path", "oldString", "newString"],
                ),
            ),
        ]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let arguments = request.arguments.unwrap_or_default();
        let result = match request.name.as_ref() {
            "read" => self.read(&arguments),
            "write" => self.write(&arguments),
            "edit" => self.edit(&arguments),
            other => {
                return Err(McpError::invalid_params(
                    format!("unknown tool `{other}`"),
                    None,
                ));
            }
        };
        Ok(result.into())
    }
}

impl FsServer {
    fn param(arguments: &Map<String, Value>, name: &str, tool: &str) -> Result<String, McpError> {
        arguments
            .get(name)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| McpError::invalid_params(format!("{tool} requires a string `{name}`"), None))
    }

    fn read(&self, arguments: &Map<String, Value>) -> CallToolResult {
        let requested = match Self::param(arguments, "path", "read") {
            Ok(path) => path,
            Err(error) => return CallToolResult::error(vec![ContentBlock::text(error.to_string())]),
        };
        let offset = arguments
            .get("offset")
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .max(1) as usize;
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(2000)
            .clamp(1, 10_000) as usize;
        let path = match self.resolve(&requested) {
            Ok(path) => path,
            Err(message) => return CallToolResult::error(vec![ContentBlock::text(message)]),
        };
        // PDFs are read as text: the text layer comes out of the file itself,
        // so any client can use it, including ones that never forward media.
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
        {
            return match std::fs::read(&path) {
                Ok(bytes) if !is_pdf(&path, &bytes) => CallToolResult::error(vec![
                    ContentBlock::text(format!("`{requested}` does not look like a PDF")),
                ]),
                Ok(bytes) if bytes.len() > MAX_PDF_BYTES => CallToolResult::error(vec![
                    ContentBlock::text(format!(
                        "`{requested}` is {} bytes, larger than the {} byte PDF limit",
                        bytes.len(),
                        MAX_PDF_BYTES
                    )),
                ]),
                Ok(bytes) => match pdf_extract::extract_text_from_mem(&bytes) {
                    Ok(text) if text.trim().is_empty() => CallToolResult::success(vec![
                        ContentBlock::text(format!(
                            "`{requested}` is a PDF ({} bytes) with no text layer: every page is a scanned image. Nothing can be read from it as text.",
                            bytes.len()
                        )),
                    ]),
                    Ok(text) => {
                        let total = text.chars().count();
                        let body: String = if total > MAX_PDF_CHARS {
                            format!(
                                "{}\n\n... (truncated: {} of {} characters shown)",
                                text.chars().take(MAX_PDF_CHARS).collect::<String>(),
                                MAX_PDF_CHARS,
                                total
                            )
                        } else {
                            text.trim().to_string()
                        };
                        CallToolResult::success(vec![ContentBlock::text(format!(
                            "`{requested}` is a PDF ({} bytes, {} characters of text)\n\n{body}",
                            bytes.len(),
                            total
                        ))])
                    }
                    Err(error) => CallToolResult::error(vec![ContentBlock::text(format!(
                        "`{requested}` could not be read as a PDF: {error}"
                    ))]),
                },
                Err(error) => CallToolResult::error(vec![ContentBlock::text(format!("{error}"))]),
            };
        }
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .unwrap_or_default();

        if extension == "docx" || extension == "xlsx" || extension == "xls" {
            let extracted = match std::fs::read(&path) {
                Ok(bytes) if bytes.len() > MAX_PDF_BYTES => Err(format!(
                    "`{requested}` is {} bytes, larger than the {} byte document limit",
                    bytes.len(),
                    MAX_PDF_BYTES
                )),
                Ok(bytes) if extension == "docx" => docx_text(&bytes),
                Ok(_) => xlsx_text(&path),
                Err(error) => Err(error.to_string()),
            };
            return match extracted {
                Ok(text) if text.trim().is_empty() => CallToolResult::success(vec![
                    ContentBlock::text(format!("`{requested}` contains no text")),
                ]),
                Ok(text) => {
                    let total = text.chars().count();
                    let body: String = if total > MAX_PDF_CHARS {
                        format!(
                            "{}\n\n... (truncated: {} of {} characters shown)",
                            text.chars().take(MAX_PDF_CHARS).collect::<String>(),
                            MAX_PDF_CHARS,
                            total
                        )
                    } else {
                        text.trim().to_string()
                    };
                    CallToolResult::success(vec![ContentBlock::text(format!(
                        "`{requested}` is a {extension} document ({} characters of text)\n\n{body}",
                        total
                    ))])
                }
                Err(error) => CallToolResult::error(vec![ContentBlock::text(format!(
                    "`{requested}` could not be read: {error}"
                ))]),
            };
        }

        if let Some(mime) = image_mime(&path) {
            return match std::fs::read(&path) {
                Ok(bytes) if bytes.len() > MAX_IMAGE_BYTES => CallToolResult::error(vec![
                    ContentBlock::text(format!(
                        "`{requested}` is {} bytes, larger than the {} byte image limit",
                        bytes.len(),
                        MAX_IMAGE_BYTES
                    )),
                ]),
                Ok(bytes) => {
                    use base64::Engine as _;
                    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
                    // The image goes first: several chat clients render only
                    // the first block of a tool result, and an image the model
                    // cannot see is useless. The hub appends its clock to the
                    // text block that follows.
                    CallToolResult::success(vec![
                        ContentBlock::image(encoded, mime),
                        ContentBlock::text(format!(
                            "{requested} (image, {mime}, {} bytes)",
                            bytes.len()
                        )),
                    ])
                }
                Err(error) => {
                    CallToolResult::error(vec![ContentBlock::text(format!("{error}"))])
                }
            };
        }
        match std::fs::read(&path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => {
                    let total = text.lines().count();
                    let window: Vec<&str> = text.lines().skip(offset - 1).take(limit).collect();
                    let last = if window.is_empty() {
                        offset.saturating_sub(1)
                    } else {
                        offset + window.len() - 1
                    };
                    let head = format!("{requested} (lines {offset}-{last} of {total})\n");
                    CallToolResult::success(vec![ContentBlock::text(format!(
                        "{head}{}",
                        numbered(&window.join("\n"), offset - 1)
                    ))])
                }
                Err(_) => CallToolResult::error(vec![ContentBlock::text(format!(
                    "`{requested}` is not UTF-8 text"
                ))]),
            },
            Err(error) => CallToolResult::error(vec![ContentBlock::text(format!("{error}"))]),
        }
    }

    fn write(&self, arguments: &Map<String, Value>) -> CallToolResult {
        let requested = match Self::param(arguments, "path", "write") {
            Ok(path) => path,
            Err(error) => return CallToolResult::error(vec![ContentBlock::text(error.to_string())]),
        };
        let content = match Self::param(arguments, "content", "write") {
            Ok(content) => content,
            Err(error) => return CallToolResult::error(vec![ContentBlock::text(error.to_string())]),
        };
        let path = match self.resolve(&requested) {
            Ok(path) => path,
            Err(message) => return CallToolResult::error(vec![ContentBlock::text(message)]),
        };
        if path.is_dir() {
            return CallToolResult::error(vec![ContentBlock::text(format!(
                "`{requested}` is a directory"
            ))]);
        }
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            return CallToolResult::error(vec![ContentBlock::text(format!(
                "cannot create parent: {error}"
            ))]);
        }
        match std::fs::write(&path, &content) {
            Ok(()) => CallToolResult::success(vec![ContentBlock::text(format!(
                "wrote {} bytes to `{requested}`",
                content.len()
            ))]),
            Err(error) => CallToolResult::error(vec![ContentBlock::text(format!("{error}"))]),
        }
    }

    fn edit(&self, arguments: &Map<String, Value>) -> CallToolResult {
        let requested = match Self::param(arguments, "path", "edit") {
            Ok(path) => path,
            Err(error) => return CallToolResult::error(vec![ContentBlock::text(error.to_string())]),
        };
        let old = match Self::param(arguments, "oldString", "edit") {
            Ok(old) => old,
            Err(error) => return CallToolResult::error(vec![ContentBlock::text(error.to_string())]),
        };
        let new = match Self::param(arguments, "newString", "edit") {
            Ok(new) => new,
            Err(error) => return CallToolResult::error(vec![ContentBlock::text(error.to_string())]),
        };
        let replace_all = arguments
            .get("replaceAll")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if old == new {
            return CallToolResult::error(vec![ContentBlock::text(
                "oldString and newString are identical — nothing to do",
            )]);
        }
        let path = match self.resolve(&requested) {
            Ok(path) => path,
            Err(message) => return CallToolResult::error(vec![ContentBlock::text(message)]),
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => return CallToolResult::error(vec![ContentBlock::text(format!("{error}"))]),
        };
        let matches = text.matches(old.as_str()).count();
        if matches == 0 {
            return CallToolResult::error(vec![ContentBlock::text(
                "oldString not found — no changes made",
            )]);
        }
        if matches > 1 && !replace_all {
            return CallToolResult::error(vec![ContentBlock::text(format!(
                "oldString matches {matches} times — set replaceAll to replace every occurrence"
            ))]);
        }
        let updated = text.replace(old.as_str(), new.as_str());
        match std::fs::write(&path, updated) {
            Ok(()) => CallToolResult::success(vec![ContentBlock::text(format!(
                "replaced {matches} occurrence{} in `{requested}`",
                if matches == 1 { "" } else { "s" }
            ))]),
            Err(error) => CallToolResult::error(vec![ContentBlock::text(format!("{error}"))]),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("BIONICHAT_FS_ROOT").ok())
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().expect("working directory"));
    if !base.is_absolute() {
        return Err("fs base must be an absolute path".into());
    }
    let service = FsServer {
        base: Arc::new(base),
    }
    .serve(stdio())
    .await?;
    service.waiting().await?;
    Ok(())
}
