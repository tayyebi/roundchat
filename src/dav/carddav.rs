/// CardDAV client.
///
/// Fetches the user's address book via CardDAV PROPFIND and parses vCards
/// into the app's Contact model.

use anyhow::{Context, Result};
use base64::Engine;
use crate::models::Contact;

/// Fetch all contacts from the CardDAV URL (with {email} placeholder).
pub async fn fetch_all_contacts(
    url_template: &str,
    email: &str,
    password: &str,
) -> Result<Vec<Contact>> {
    let url = url_template.replace("{email}", &urlencoding::encode(email).to_string());
    let auth = base64::engine::general_purpose::STANDARD
        .encode(format!("{email}:{password}"));

    let client = reqwest::Client::new();

    // PROPFIND to list .vcf resources.
    let propfind_body = r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:carddav">
  <D:prop>
    <D:getetag/>
    <D:getcontenttype/>
  </D:prop>
</D:propfind>"#;

    let resp = client
        .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &url)
        .header("Authorization", format!("Basic {auth}"))
        .header("Depth", "1")
        .header("Content-Type", "application/xml; charset=utf-8")
        .body(propfind_body)
        .send()
        .await
        .context("CardDAV PROPFIND request failed")?;

    if !resp.status().is_success() && resp.status().as_u16() != 207 {
        return Err(anyhow::anyhow!("CardDAV PROPFIND status: {}", resp.status()));
    }

    let xml = resp.text().await.context("CardDAV PROPFIND read body")?;
    let hrefs = parse_href_list(&xml);

    // Derive base origin from URL.
    let base_origin = {
        let parsed = url::Url::parse(&url).context("parse CardDAV URL")?;
        let default_port = if parsed.scheme() == "https" { 443u16 } else { 80u16 };
        format!("{}://{}:{}", parsed.scheme(), parsed.host_str().unwrap_or(""), parsed.port().unwrap_or(default_port))
    };

    let mut contacts = Vec::new();
    for href in hrefs {
        let full_url = if href.starts_with("http") {
            href.clone()
        } else {
            format!("{}{}", base_origin, href)
        };
        match fetch_contact(&client, &full_url, &auth).await {
            Ok(Some(c)) => contacts.push(c),
            Ok(None) => {}
            Err(e) => tracing::warn!("CardDAV fetch {href}: {e}"),
        }
    }

    contacts.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    Ok(contacts)
}

fn parse_href_list(xml: &str) -> Vec<String> {
    let mut hrefs = Vec::new();
    // Simple regex-free extraction: find <D:href> values ending in .vcf.
    for cap in xml.split("<D:href>").skip(1) {
        if let Some(end) = cap.find("</D:href>") {
            let href = cap[..end].trim().to_string();
            if href.ends_with(".vcf") {
                hrefs.push(href);
            }
        }
    }
    hrefs
}

async fn fetch_contact(
    client: &reqwest::Client,
    url: &str,
    auth: &str,
) -> Result<Option<Contact>> {
    let resp = client
        .get(url)
        .header("Authorization", format!("Basic {auth}"))
        .send()
        .await
        .context("CardDAV GET vcard")?;

    if !resp.status().is_success() {
        return Ok(None);
    }
    let vcard = resp.text().await.context("vcard read body")?;
    Ok(parse_vcard(&vcard))
}

fn parse_vcard(vcard: &str) -> Option<Contact> {
    let uid = vcard_field(vcard, "UID").unwrap_or_default();
    let fn_val = vcard_field(vcard, "FN").unwrap_or_default();
    let email = vcard_email_field(vcard)?;
    let phone = vcard_field(vcard, "TEL");
    let avatar_url = vcard_field(vcard, "PHOTO");

    Some(Contact {
        id: if uid.is_empty() { email.clone() } else { uid },
        display_name: if fn_val.is_empty() { email.clone() } else { fn_val },
        email,
        phone,
        avatar_url,
    })
}

fn vcard_field(vcard: &str, field: &str) -> Option<String> {
    for line in vcard.lines() {
        let upper = line.to_uppercase();
        let field_upper = field.to_uppercase();
        if upper.starts_with(&field_upper) {
            if let Some(colon_pos) = line.find(':') {
                let prefix = &line[..colon_pos].to_uppercase();
                // Match "FIELD" or "FIELD;params"
                if prefix.as_str() == field_upper || prefix.starts_with(&format!("{field_upper};")) {
                    return Some(line[colon_pos + 1..].trim().to_string());
                }
            }
        }
    }
    None
}

fn vcard_email_field(vcard: &str) -> Option<String> {
    for line in vcard.lines() {
        let upper = line.to_uppercase();
        if upper.starts_with("EMAIL") {
            if let Some(colon_pos) = line.find(':') {
                return Some(line[colon_pos + 1..].trim().to_lowercase());
            }
        }
    }
    None
}
