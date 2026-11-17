use actix_web::web::{Data, Json};
use chrono::{Duration, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use lemmy_api_common::context::LemmyContext;
use lemmy_db_schema::{schema::passport_nonce, utils::get_conn};
use lemmy_utils::error::{LemmyErrorType, LemmyResult};

const NONCE_TTL_MINUTES: i64 = 5;

#[derive(serde::Serialize)]
pub struct PassportNonceResponse {
  pub nonce: String,
}

pub async fn passport_nonce(context: Data<LemmyContext>) -> LemmyResult<Json<PassportNonceResponse>> {
  let nonce = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
  let mut pool = context.pool();
  let conn = &mut get_conn(&mut pool).await?;
  let cutoff = Utc::now() - Duration::minutes(NONCE_TTL_MINUTES);

  // opportunistic cleanup of expired nonces
  diesel::delete(passport_nonce::table.filter(passport_nonce::published_at.lt(cutoff)))
    .execute(conn)
    .await?;

  diesel::insert_into(passport_nonce::table)
    .values(passport_nonce::nonce.eq(&nonce))
    .execute(conn)
    .await?;

  Ok(Json(PassportNonceResponse { nonce }))
}

pub(super) async fn consume_passport_nonce(
  context: &LemmyContext,
  nonce: &str,
) -> LemmyResult<()> {
  let mut pool = context.pool();
  let conn = &mut get_conn(&mut pool).await?;
  let cutoff = Utc::now() - Duration::minutes(NONCE_TTL_MINUTES);

  let deleted = diesel::delete(
    passport_nonce::table
      .filter(passport_nonce::nonce.eq(nonce))
      .filter(passport_nonce::published_at.gt(cutoff)),
  )
  .execute(conn)
  .await?;
  
  if deleted == 0 {
    return Err(LemmyErrorType::IncorrectLogin.into());
  }
  Ok(())
}
