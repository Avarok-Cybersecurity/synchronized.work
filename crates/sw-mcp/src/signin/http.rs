//! [`IssuerPort`] over HTTPS (reqwest + rustls with the platform's web PKI roots). The only
//! network code of sign-in; tests drive the flow through an in-process stand-in instead.

use super::flow::{IssuerPort, IssuerReply};

#[derive(Debug, Clone)]
pub struct HttpIssuer {
    client: reqwest::Client,
    base: String,
}

impl HttpIssuer {
    /// `issuer` is a canonical domain (`work.avarok.net`); every request goes to
    /// `https://<issuer>/…`, never plain HTTP. `timeout_s` bounds each request.
    pub fn new(issuer: &str, timeout_s: u64) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(timeout_s))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            base: format!("https://{issuer}"),
        })
    }
}

impl IssuerPort for HttpIssuer {
    async fn well_known(&self) -> Result<Vec<u8>, String> {
        let url = format!("{}/.well-known/citadel", self.base);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !response.status().is_success() {
            return Err(format!("{url}: HTTP {}", response.status()));
        }
        let body = response.bytes().await.map_err(|e| e.to_string())?;
        Ok(body.to_vec())
    }

    async fn complete(&self, json: Vec<u8>) -> Result<IssuerReply, String> {
        let url = format!("{}/sign-in/complete", self.base);
        let response = self
            .client
            .post(&url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(json)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let body = response.bytes().await.map_err(|e| e.to_string())?;
        Ok(IssuerReply {
            status,
            body: body.to_vec(),
        })
    }
}
