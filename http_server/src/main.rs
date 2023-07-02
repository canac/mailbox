#![deny(clippy::pedantic)]

mod filter;

use crate::filter::Filter;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::error::{ErrorBadRequest, ErrorInternalServerError};
use actix_web::http::header::{HeaderValue, ACCESS_CONTROL_ALLOW_ORIGIN};
use actix_web::middleware::DefaultHeaders;
use actix_web::web::{self, Data, Json, Query, ServiceConfig};
use actix_web::{delete, get, post, put, HttpResponse, Result};
use anyhow::{anyhow, Context};
use database::{Database, Engine, Message, MessageFilter, NewMessage, State};
use futures::future::try_join_all;
use serde::Deserialize;
use shuttle_actix_web::ShuttleActixWeb;
use shuttle_secrets::SecretStore;
use std::collections::BTreeMap;
use std::sync::Arc;

type AppData = Arc<Database>;

#[derive(Deserialize)]
#[serde(untagged)]
enum CreateMessage {
    Message(NewMessage),
    Messages(Vec<NewMessage>),
}

#[get("/mailboxes")]
async fn read_mailboxes(
    data: Data<AppData>,
    filter: Query<Filter>,
) -> Result<Json<BTreeMap<String, usize>>> {
    let mailboxes = data
        .load_mailboxes(filter.into_inner().try_into().map_err(ErrorBadRequest)?)
        .await
        .map_err(ErrorInternalServerError)?
        .into_iter()
        .collect();
    Ok(Json(mailboxes))
}

#[get("/messages")]
async fn read_messages(data: Data<AppData>, filter: Query<Filter>) -> Result<Json<Vec<Message>>> {
    let messages = data
        .load_messages(filter.into_inner().try_into().map_err(ErrorBadRequest)?)
        .await
        .map_err(ErrorInternalServerError)?;
    Ok(Json(messages))
}

#[post("/messages")]
async fn create_messages(
    data: Data<AppData>,
    messages: Json<CreateMessage>,
) -> Result<Json<Vec<Message>>> {
    let new_messages = match messages.into_inner() {
        CreateMessage::Message(message) => vec![message],
        CreateMessage::Messages(messages) => messages,
    };
    let futures = new_messages
        .into_iter()
        .map(|message| data.add_message(message))
        .collect::<Vec<_>>();
    let messages = try_join_all(futures)
        .await
        .map_err(ErrorInternalServerError)?;
    Ok(Json(messages))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateMessages {
    new_state: State,
}

#[put("/messages")]
async fn update_messages(
    data: Data<AppData>,
    filter: Query<Filter>,
    new_state: Json<UpdateMessages>,
) -> Result<Json<Vec<Message>>> {
    let message_filter = filter.into_inner().try_into().map_err(ErrorBadRequest)?;
    let messages = data
        .change_state(message_filter, new_state.into_inner().new_state)
        .await
        .map_err(ErrorInternalServerError)?;
    Ok(Json(messages))
}

#[delete("/messages")]
async fn delete_messages(data: Data<AppData>, filter: Query<Filter>) -> Result<Json<Vec<Message>>> {
    let message_filter: MessageFilter = filter.into_inner().try_into().map_err(ErrorBadRequest)?;
    if message_filter.matches_all() {
        return Err(ErrorBadRequest("Filter is required"));
    }
    let messages = data
        .delete_messages(message_filter)
        .await
        .map_err(ErrorInternalServerError)?;
    Ok(Json(messages))
}

async fn get_config(
    database_engine: Engine,
    auth_token: String,
) -> std::result::Result<
    impl FnOnce(&mut ServiceConfig) + Send + Clone + 'static,
    shuttle_runtime::Error,
> {
    let database = Arc::new(Database::new(database_engine).await?);
    let auth_header = HeaderValue::from_str(format!("Bearer {auth_token}").as_str())
        .context("Failed to parse header")?;

    let config = move |cfg: &mut ServiceConfig| {
        let app_data = Data::new(database);
        cfg.service(
            web::scope("/api")
                .wrap_fn(move |req, srv| {
                    if req.headers().get("Authorization") == Some(&auth_header) {
                        srv.call(req)
                    } else {
                        Box::pin(async {
                            let res = HttpResponse::Forbidden().finish();
                            Ok(ServiceResponse::new(req.into_parts().0, res))
                        })
                    }
                })
                .wrap(DefaultHeaders::new().add((ACCESS_CONTROL_ALLOW_ORIGIN, "*")))
                .app_data(app_data)
                .service(read_mailboxes)
                .service(read_messages)
                .service(create_messages)
                .service(update_messages)
                .service(delete_messages),
        );
    };

    Ok(config)
}

#[shuttle_runtime::main]
async fn actix_web(
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
) -> ShuttleActixWeb<impl FnOnce(&mut ServiceConfig) + Send + Clone + 'static> {
    let database_url = secret_store
        .get("DATABASE_URL")
        .ok_or_else(|| anyhow!("Missing DATABASE_URL secret"))?;
    let auth_token = secret_store
        .get("AUTH_TOKEN")
        .ok_or_else(|| anyhow!("Missing AUTH_TOKEN secret"))?;

    let config = get_config(Engine::Postgres(database_url), auth_token).await?;
    Ok(config.into())
}

#[cfg(test)]
mod tests {
    use actix_web::http::header;
    use actix_web::test::{self, call_service, TestRequest};
    use actix_web::App;

    use super::*;

    async fn make_config() -> std::result::Result<
        impl FnOnce(&mut ServiceConfig) + Send + Clone + 'static,
        shuttle_runtime::Error,
    > {
        let auth_token = String::from("token");
        get_config(Engine::Sqlite(None), auth_token).await
    }

    #[actix_web::test]
    async fn test_missing_authorization_header() {
        let app = App::new().configure(make_config().await.unwrap());
        let app = test::init_service(app).await;

        let req = TestRequest::get().uri("/api/messages").to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_client_error());
    }

    #[actix_web::test]
    async fn test_invalid_authorization_header() {
        let app = App::new().configure(make_config().await.unwrap());
        let app = test::init_service(app).await;

        let req = TestRequest::get()
            .uri("/api/messages")
            .append_header((header::AUTHORIZATION, "Bearer invalid"))
            .to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_client_error());
    }

    #[actix_web::test]
    async fn test_cors_header() {
        let app = App::new().configure(make_config().await.unwrap());
        let app = test::init_service(app).await;

        let req = TestRequest::get()
            .uri("/api/messages")
            .append_header((header::AUTHORIZATION, "Bearer token"))
            .to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_success());
        assert_eq!(res.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(), "*");
    }

    #[actix_web::test]
    async fn test_filter_ids() {
        let app = App::new().configure(make_config().await.unwrap());
        let app = test::init_service(app).await;

        let req = TestRequest::get()
            .uri("/api/messages?ids=1")
            .append_header((header::AUTHORIZATION, "Bearer token"))
            .to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_success());

        let req = TestRequest::get()
            .uri("/api/messages?ids=1,2,3")
            .append_header((header::AUTHORIZATION, "Bearer token"))
            .to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_success());

        let req = TestRequest::get()
            .uri("/api/messages?ids=foo")
            .append_header((header::AUTHORIZATION, "Bearer token"))
            .to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_client_error());
    }

    #[actix_web::test]
    async fn test_filter_mailbox() {
        let app = App::new().configure(make_config().await.unwrap());
        let app = test::init_service(app).await;

        let req = TestRequest::get()
            .uri("/api/messages?mailbox=foo")
            .append_header((header::AUTHORIZATION, "Bearer token"))
            .to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_success());
    }

    #[actix_web::test]
    async fn test_filter_states() {
        let app = App::new().configure(make_config().await.unwrap());
        let app = test::init_service(app).await;

        let req = TestRequest::get()
            .uri("/api/messages?states=unread")
            .append_header((header::AUTHORIZATION, "Bearer token"))
            .to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_success());

        let req = TestRequest::get()
            .uri("/api/messages?states=read,archived")
            .append_header((header::AUTHORIZATION, "Bearer token"))
            .to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_success());

        let req = TestRequest::get()
            .uri("/api/messages?states=unread,foo")
            .append_header((header::AUTHORIZATION, "Bearer token"))
            .to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_client_error());
    }

    #[actix_web::test]
    async fn test_filter_multiple() {
        let app = App::new().configure(make_config().await.unwrap());
        let app = test::init_service(app).await;

        let req = TestRequest::get()
            .uri("/api/messages?ids=1,2,3&mailbox=foo&states=unread,read")
            .append_header((header::AUTHORIZATION, "Bearer token"))
            .to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_success());
    }

    #[actix_web::test]
    async fn test_delete_no_filter() {
        let app = App::new().configure(make_config().await.unwrap());
        let app = test::init_service(app).await;

        let req = TestRequest::delete()
            .uri("/api/messages")
            .append_header((header::AUTHORIZATION, "Bearer token"))
            .to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_client_error());
    }

    #[actix_web::test]
    async fn test_messages() {
        let app = App::new().configure(make_config().await.unwrap());
        let app = test::init_service(app).await;

        let req = TestRequest::get()
            .uri("/api/messages")
            .append_header((header::AUTHORIZATION, "Bearer token"))
            .to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_success());
    }

    #[actix_web::test]
    async fn test_mailboxes() {
        let app = App::new().configure(make_config().await.unwrap());
        let app = test::init_service(app).await;

        let req = TestRequest::get()
            .uri("/api/mailboxes")
            .append_header((header::AUTHORIZATION, "Bearer token"))
            .to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_success());
    }

    #[actix_web::test]
    async fn test_create_single_message() {
        let app = App::new().configure(make_config().await.unwrap());
        let app = test::init_service(app).await;

        let req = TestRequest::post()
            .uri("/api/messages")
            .append_header((header::AUTHORIZATION, "Bearer token"))
            .append_header(header::ContentType::json())
            .set_payload(
                r#"{
  "mailbox": "my-script",
  "content": "Hello, world!",
  "state": "read"
}"#,
            )
            .to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_success());
    }

    #[actix_web::test]
    async fn test_create_multiple_messages() {
        let app = App::new().configure(make_config().await.unwrap());
        let app = test::init_service(app).await;

        let req = TestRequest::post()
            .uri("/api/messages")
            .append_header((header::AUTHORIZATION, "Bearer token"))
            .append_header(header::ContentType::json())
            .set_payload(
                r#"[{
  "mailbox": "my-script",
  "content": "Hello, world!",
  "state": "archived"
}, {
  "mailbox": "my-script",
  "content": "Hello, universe!"
}]"#,
            )
            .to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_success());
    }

    #[actix_web::test]
    async fn test_update_messages() {
        let app = App::new().configure(make_config().await.unwrap());
        let app = test::init_service(app).await;

        let req = TestRequest::put()
            .uri("/api/messages?states=unread")
            .append_header((header::AUTHORIZATION, "Bearer token"))
            .append_header(header::ContentType::json())
            .set_payload(r#"{"new_state": "read"}"#)
            .to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_success());
    }

    #[actix_web::test]
    async fn test_delete_messages() {
        let app = App::new().configure(make_config().await.unwrap());
        let app = test::init_service(app).await;

        let req = TestRequest::delete()
            .uri("/api/messages?states=unread")
            .append_header((header::AUTHORIZATION, "Bearer token"))
            .to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_success());
    }
}
