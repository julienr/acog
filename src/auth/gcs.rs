// Google Cloud Storage authentication
//
// This currently supports service account authentication only
use crate::Error;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
struct Claims {
    iss: String,
    scope: String,
    aud: String,
    iat: u64,
    exp: u64,
}

#[derive(Deserialize)]
struct ServiceAccount {
    private_key_id: String,
    private_key: String,
    client_email: String,
}

impl ServiceAccount {
    fn new() -> Result<ServiceAccount, Error> {
        let service_account_file_content = std::env::var("GOOGLE_SERVICE_ACCOUNT_CONTENT")?;
        let service_account: ServiceAccount = serde_json::from_str(&service_account_file_content)?;
        Ok(service_account)
    }
    fn make_access_token_jwt_assertion(&self) -> Result<String, Error> {
        // https://developers.google.com/identity/protocols/oauth2/service-account#jwt-auth
        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        header.typ = Some("JWT".to_string());
        header.kid = Some(self.private_key_id.clone());
        let unix_timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let claims = Claims {
            iss: self.client_email.clone(),
            scope: "https://www.googleapis.com/auth/devstorage.read_only".to_string(),
            aud: "https://oauth2.googleapis.com/token".to_string(),
            iat: unix_timestamp,
            exp: unix_timestamp + 3600,
        };
        let token = jsonwebtoken::encode(
            &header,
            &claims,
            &jsonwebtoken::EncodingKey::from_rsa_pem(self.private_key.as_bytes())?,
        )?;
        Ok(token)
    }

    async fn make_access_token(&self, client: &reqwest::Client) -> Result<AccessToken, Error> {
        let assertion = self.make_access_token_jwt_assertion()?;
        let form = reqwest::multipart::Form::new()
            .part("assertion", reqwest::multipart::Part::text(assertion))
            .part(
                "grant_type",
                reqwest::multipart::Part::text("urn:ietf:params:oauth:grant-type:jwt-bearer"),
            );
        let request_timestamp = Instant::now();
        let resp = client
            .post("https://oauth2.googleapis.com/token")
            .multipart(form)
            .send()
            .await?;
        let token_resp: TokenResponse = serde_json::from_str(&resp.text().await?)?;
        if token_resp.token_type != "Bearer" {
            return Err(Error::OtherError(format!(
                "Invalid token type {}",
                token_resp.token_type
            )));
        }
        Ok(AccessToken {
            token: token_resp.access_token,
            expiration: request_timestamp + Duration::new(token_resp.expires_in, 0),
        })
    }
}

pub struct GCSAuth {
    service_account: ServiceAccount,
    access_token: Option<AccessToken>,
}

impl GCSAuth {
    pub fn new() -> Result<GCSAuth, Error> {
        Ok(GCSAuth {
            service_account: ServiceAccount::new()?,
            access_token: None,
        })
    }

    pub async fn get_access_token(
        &mut self,
        client: &reqwest::Client,
    ) -> Result<&AccessToken, Error> {
        if self.access_token.is_none() || self.access_token.as_ref().unwrap().has_expired() {
            self.access_token = Some(self.service_account.make_access_token(client).await?)
        }
        Ok(self.access_token.as_ref().unwrap())
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
}

#[derive(Clone)]
pub struct AccessToken {
    pub token: String,
    pub expiration: Instant,
}

impl AccessToken {
    fn has_expired(&self) -> bool {
        // Use a 1 second buffer to renew a bit earlier
        self.expiration < (Instant::now() - Duration::from_secs(1))
    }
}

#[cfg(test)]
mod tests {
    use super::GCSAuth;

    #[tokio::test]
    async fn test_gcs_auth() {
        let mut auth = GCSAuth::new().unwrap();
        let client = reqwest::Client::builder().build().unwrap();
        let access_token = auth.get_access_token(&client).await.unwrap();
        // The fact we got a token already means we did get a 200 ok from the oauth endpoint
        // (validating most of the logic). So here just sanity check the token metadata
        assert_ne!(access_token.token, "");
        assert!(!access_token.has_expired());
    }
}
