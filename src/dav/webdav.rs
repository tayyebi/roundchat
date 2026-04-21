/// WebDAV client.
///
/// Lists, uploads, and deletes files on a WebDAV server.

use anyhow::{Context, Result};
use base64::Engine;
use crate::models::RemoteFile;

fn build_url(url_template: &str, email: &str, path: &str) -> String {
    let base = url_template.replace("{email}", &urlencoding::encode(email).to_string());
    let base = if base.ends_with('/') {
        base
    } else {
        format!("{base}/")
    };
    format!("{base}{path}")
}

fn auth_header(email: &str, password: &str) -> String {
    let token = base64::engine::general_purpose::STANDARD
        .encode(format!("{email}:{password}"));
    format!("Basic {token}")
}

/// List all files in the user's WebDAV root directory.
pub async fn list_files(
    url_template: &str,
    email: &str,
    password: &str,
) -> Result<Vec<RemoteFile>> {
    let url = build_url(url_template, email, "");
    let auth = auth_header(email, password);
    let client = reqwest::Client::new();

    let body = r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:">
  <D:prop>
    <D:displayname/>
    <D:getcontentlength/>
    <D:getcontenttype/>
    <D:getlastmodified/>
  </D:prop>
</D:propfind>"#;

    let resp = client
        .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &url)
        .header("Authorization", &auth)
        .header("Depth", "1")
        .header("Content-Type", "application/xml; charset=utf-8")
        .body(body)
        .send()
        .await
        .context("WebDAV PROPFIND failed")?;

    if !resp.status().is_success() && resp.status().as_u16() != 207 {
        return Err(anyhow::anyhow!("WebDAV PROPFIND status: {}", resp.status()));
    }

    let xml = resp.text().await.context("WebDAV PROPFIND read body")?;
    Ok(parse_propfind(&xml, &url))
}

fn parse_propfind(xml: &str, base_url: &str) -> Vec<RemoteFile> {
    let mut files = Vec::new();
    for response in xml.split("<D:response>").skip(1) {
        let end = response.find("</D:response>").unwrap_or(response.len());
        let block = &response[..end];

        let href = xml_value(block, "D:href").unwrap_or_default();
        // Skip the root directory entry itself.
        if href.ends_with('/') {
            continue;
        }
        let name = xml_value(block, "D:displayname")
            .filter(|n| !n.is_empty())
            .or_else(|| href.split('/').last().map(|s| s.to_string()))
            .unwrap_or_default();

        if name.is_empty() {
            continue;
        }

        let size = xml_value(block, "D:getcontentlength")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0u64);
        let mime_type = xml_value(block, "D:getcontenttype")
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let last_modified = xml_value(block, "D:getlastmodified").unwrap_or_default();

        let file_url = if href.starts_with("http") {
            href
        } else {
            format!("{base_url}{href}")
        };

        files.push(RemoteFile {
            name,
            size,
            mime_type,
            url: file_url,
            last_modified,
        });
    }
    files
}

fn xml_value(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = xml.find(&open)?;
    // Find the '>' that closes the opening tag (may have attributes).
    let gt = xml[start..].find('>')? + start + 1;
    let end = xml[gt..].find(&close)? + gt;
    Some(xml[gt..end].trim().to_string())
}

/// Upload a file to WebDAV and return its URL.
pub async fn upload_file(
    url_template: &str,
    email: &str,
    password: &str,
    filename: &str,
    mime_type: &str,
    data: bytes::Bytes,
) -> Result<String> {
    let remote_url = build_url(
        url_template,
        email,
        &urlencoding::encode(filename).to_string(),
    );
    let auth = auth_header(email, password);
    let client = reqwest::Client::new();

    let resp = client
        .put(&remote_url)
        .header("Authorization", auth)
        .header("Content-Type", mime_type)
        .body(data)
        .send()
        .await
        .context("WebDAV PUT failed")?;

    if !resp.status().is_success() {
        return Err(anyhow::anyhow!(
            "WebDAV PUT status: {}",
            resp.status()
        ));
    }

    Ok(remote_url)
}

/// Delete a file by its URL.
pub async fn delete_file(
    url: &str,
    email: &str,
    password: &str,
) -> Result<()> {
    let auth = auth_header(email, password);
    let client = reqwest::Client::new();

    let resp = client
        .delete(url)
        .header("Authorization", auth)
        .send()
        .await
        .context("WebDAV DELETE failed")?;

    if !resp.status().is_success() && resp.status().as_u16() != 404 {
        return Err(anyhow::anyhow!(
            "WebDAV DELETE status: {}",
            resp.status()
        ));
    }

    Ok(())
}
