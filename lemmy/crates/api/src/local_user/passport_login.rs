use actix_web::{web::{Data, Json}, HttpRequest};
use super::login::check_registration_application;
use lemmy_api_common::{
  claims::Claims,
  context::LemmyContext,
  person::{LoginResponse, PassportLogin},
  utils::check_user_valid,
};
use lemmy_api_crud::passport_user::{find_user_by_passport_identifier, register_passport_user};
use lemmy_db_schema::RegistrationMode;
use lemmy_db_views::structs::SiteView;
use lemmy_utils::error::{LemmyErrorType, LemmyResult};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct VerifyRequest<'a> {
  proofs: &'a serde_json::Value,
  #[serde(rename = "queryResult")]
  query_result: &'a serde_json::Value,
  #[serde(rename = "expectedNonce")]
  expected_nonce: &'a str,
}

#[derive(Deserialize)]
struct VerifyResponse {
  verified: bool,
  #[serde(rename = "userId")]
  user_id: Option<String>,
}

async fn verify_proof(
  context: &LemmyContext,
  data: &PassportLogin,
) -> LemmyResult<String> {
  let url = std::env::var("ZKPASSPORT_VERIFY_URL").map_err(|error| {
    LemmyErrorType::Unknown(format!("ZKPASSPORT_VERIFY_URL is not configured: {error}"))
  })?;
  let secret = std::env::var("ZKPASSPORT_INTERNAL_SECRET").unwrap_or_default();
  let response = context.client().post(url)
    .header("X-Internal-Secret", secret)
    .json(&VerifyRequest { proofs: &data.proofs, query_result: &data.query_result, expected_nonce: &data.nonce })
    .send().await?.error_for_status()?.json::<VerifyResponse>().await?;
    
  match (response.verified, response.user_id) {
    (true, Some(id)) if !id.trim().is_empty() => Ok(id),
    _ => Err(LemmyErrorType::IncorrectLogin.into()),
  }
}

pub async fn passport_login(
  Json(data): Json<PassportLogin>,
  req: HttpRequest,
  context: Data<LemmyContext>,
) -> LemmyResult<Json<LoginResponse>> {

  // 1. Consume the nonce FIRST (single use, even if verification later fails),
  //    then verify with the internal service, which checks the bound nonce.
  super::passport_nonce::consume_passport_nonce(&context, &data.nonce).await?;
  let identifier = verify_proof(&context, &data).await?;
  let site_view = SiteView::read_local(&mut context.pool()).await?
    .ok_or(LemmyErrorType::LocalSiteNotSetup)?;
  
    // 2. Login or Register with the identifier
  let user = match find_user_by_passport_identifier(&context, &identifier).await? {
    // Existing user -> log in
    Some(user) => {
      check_user_valid(&user.person)?;
      check_registration_application(&user, &site_view.local_site, &mut context.pool()).await?;
      user
    }
    // No account -> register
    None => {
     // TODO make another registration mode for passport registration mode closed or open
      if site_view.local_site.registration_mode != RegistrationMode::Open {
        return Err(LemmyErrorType::RegistrationClosed.into());
      }
      if site_view.local_site.require_email_verification {
        return Err(LemmyErrorType::EmailRequired.into());
      }
      register_passport_user(&context, &identifier).await?
    }
  };

  // 4. Issue the JWT (no password / email / totp checks: the passport proof is the credential)
  let jwt = Claims::generate(user.local_user.id, req, &context).await?;
  Ok(Json(LoginResponse { jwt: Some(jwt), verify_email_sent: false, registration_created: false }))
}
