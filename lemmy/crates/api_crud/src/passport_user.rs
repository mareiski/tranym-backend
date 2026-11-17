use activitypub_federation::http_signatures::generate_actor_keypair;
use bcrypt::{hash, DEFAULT_COST};
use diesel::prelude::*;
use diesel_async::{scoped_futures::ScopedFutureExt, AsyncConnection, RunQueryDsl};
use lemmy_api_common::{
  context::LemmyContext,
  utils::{generate_inbox_url, generate_local_apub_endpoint, EndpointType},
};
use lemmy_db_schema::{
  newtypes::LocalUserId,
  schema::passport_account,
  source::{
    actor_language::SiteLanguage,
    local_user::{LocalUser, LocalUserInsertForm},
    person::{Person, PersonInsertForm},
  },
  traits::Crud,
  utils::get_conn,
};
use lemmy_db_views::structs::{LocalUserView, SiteView};
use lemmy_utils::error::{LemmyErrorType, LemmyResult};

pub async fn find_user_by_passport_identifier(
  context: &LemmyContext,
  identifier: &str,
) -> LemmyResult<Option<LocalUserView>> {
  let mut pool = context.pool();
  let conn = &mut get_conn(&mut pool).await?;
  let user_id: Option<LocalUserId> = passport_account::table
    .filter(passport_account::identifier.eq(identifier))
    .select(passport_account::local_user_id)
    .first(conn).await.optional()?;
  match user_id {
    Some(id) => Ok(LocalUserView::read(&mut pool, id).await?),
    None => Ok(None),
  }
}

pub async fn register_passport_user(
  context: &LemmyContext,
  identifier: &str,
) -> LemmyResult<LocalUserView> {
  let suffix: String = uuid::Uuid::new_v4().simple().to_string().chars().take(16).collect();
  let username = format!("t_{suffix}");
  let site_view = SiteView::read_local(&mut context.pool()).await?
    .ok_or(LemmyErrorType::LocalSiteNotSetup)?;

  let keys = generate_actor_keypair()?;
  let actor_id = generate_local_apub_endpoint(
    EndpointType::Person,
    &username,
    &context.settings().get_protocol_and_hostname(),
  )?;

  let person_form = PersonInsertForm {
    actor_id: Some(actor_id.clone()),
    inbox_url: Some(generate_inbox_url(&actor_id)?),
    private_key: Some(keys.private_key),
    ..PersonInsertForm::new(username, keys.public_key, site_view.site.instance_id)
  };
  let mut pool = context.pool();
  let languages = SiteLanguage::read(&mut pool, site_view.local_site.site_id).await?;
  let conn = &mut get_conn(&mut pool).await?;
  let identifier = identifier.to_string();
  let local_user_id = conn
    .transaction::<_, lemmy_utils::error::LemmyError, _>(|conn| {
      async move {
        let person = Person::create(&mut conn.into(), &person_form).await?;

        // The password is random and discarded; Passport proofs are the only supported login method.
        let random_password = uuid::Uuid::new_v4().to_string();
        let password = hash(random_password, DEFAULT_COST)?;
        let local_user_form = LocalUserInsertForm {
          accepted_application: Some(true),
          ..LocalUserInsertForm::new(person.id, password)
        };
        let local_user =
          LocalUser::create(&mut conn.into(), &local_user_form, languages).await?;

        let inserted = diesel::insert_into(passport_account::table)
          .values((
            passport_account::local_user_id.eq(local_user.id),
            passport_account::identifier.eq(&identifier),
          ))
          .on_conflict_do_nothing()
          .execute(conn)
          .await?;

        if inserted == 0 {
          Person::delete(&mut conn.into(), person.id).await?;
          passport_account::table
            .filter(passport_account::identifier.eq(&identifier))
            .select(passport_account::local_user_id)
            .first(conn)
            .await
            .optional()?
            .ok_or_else(|| LemmyErrorType::IncorrectLogin.into())
        } else {
          Ok(local_user.id)
        }
      }
      .scope_boxed()
    })
    .await?;

  LocalUserView::read(&mut pool, local_user_id)
    .await?
    .ok_or_else(|| LemmyErrorType::CouldntFindLocalUser.into())
}
