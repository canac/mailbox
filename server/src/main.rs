#![warn(clippy::str_to_string, clippy::pedantic, clippy::nursery)]

mod cli;

use actix_web::dev::{Service, ServiceResponse};
use actix_web::error::{ErrorBadRequest, ErrorInternalServerError};
use actix_web::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue};
use actix_web::middleware::DefaultHeaders;
use actix_web::web::{self, Data, Json, Query, ServiceConfig};
use actix_web::{App, HttpResponse, HttpServer, Result, delete, get, post, put};
use anyhow::Context;
use clap::Parser;
use cli::Cli;
use database::{Database, Filter, MailboxInfo, Message, NewMessage, SqliteBackend, State};
use serde::Deserialize;
use std::sync::Arc;

type AppData = Arc<Database<SqliteBackend>>;

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
) -> Result<Json<Vec<MailboxInfo>>> {
    let mailboxes = data
        .load_mailboxes(filter.into_inner())
        .await
        .map_err(ErrorInternalServerError)?;
    Ok(Json(mailboxes))
}

#[get("/messages")]
async fn read_messages(data: Data<AppData>, filter: Query<Filter>) -> Result<Json<Vec<Message>>> {
    let messages = data
        .load_messages(filter.into_inner())
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
    let messages = data
        .add_messages(new_messages)
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
    let messages = data
        .change_state(filter.into_inner(), new_state.into_inner().new_state)
        .await
        .map_err(ErrorInternalServerError)?;
    Ok(Json(messages))
}

#[delete("/messages")]
async fn delete_messages(data: Data<AppData>, filter: Query<Filter>) -> Result<Json<Vec<Message>>> {
    if filter.matches_all() {
        return Err(ErrorBadRequest("Filter is required"));
    }
    let messages = data
        .delete_messages(filter.into_inner())
        .await
        .map_err(ErrorInternalServerError)?;
    Ok(Json(messages))
}

// Return a config factory function that can be passed to App::configure to setup all the data,
// routes and middleware for the app
fn get_config_factory(
    backend: SqliteBackend,
    auth_token: Option<&str>,
) -> anyhow::Result<impl FnOnce(&mut ServiceConfig) + Clone + use<>> {
    let db = Arc::new(Database::new(backend));
    let auth_header = auth_token
        .map(|token| {
            HeaderValue::from_str(format!("Bearer {token}").as_str())
                .context("Failed to parse header")
        })
        .transpose()?;
    let config_factory = |cfg: &mut ServiceConfig| {
        let app_data = Data::new(db);
        cfg.service(
            web::scope("")
                .wrap_fn(move |req, srv| {
                    if auth_header.is_none()
                        || req.headers().get("Authorization") == auth_header.as_ref()
                    {
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

    Ok(config_factory)
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let backend = SqliteBackend::new(cli.db_file).await?;
    let config_factory = get_config_factory(backend, cli.token.as_deref())?;
    HttpServer::new(move || App::new().configure(config_factory.clone()))
        .bind((if cli.expose { "0.0.0.0" } else { "127.0.0.1" }, cli.port))?
        .run()
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use actix_web::App;
    use actix_web::http::header;
    use actix_web::test::{TestRequest, call_service, init_service};

    use super::*;

    async fn make_config_factory() -> anyhow::Result<impl FnOnce(&mut ServiceConfig)> {
        get_config_factory(SqliteBackend::new_test().await?, None)
    }

    #[actix_web::test]
    async fn test_missing_authorization_header() {
        let config_factory =
            get_config_factory(SqliteBackend::new_test().await.unwrap(), Some("token")).unwrap();
        let app = App::new().configure(config_factory);
        let service = init_service(app).await;

        let req = TestRequest::get().uri("/messages").to_request();
        let res = call_service(&service, req).await;
        assert!(res.status().is_client_error());
    }

    #[actix_web::test]
    async fn test_invalid_authorization_header() {
        let config_factory =
            get_config_factory(SqliteBackend::new_test().await.unwrap(), Some("token")).unwrap();
        let app = App::new().configure(config_factory);
        let service = init_service(app).await;

        let req = TestRequest::get()
            .uri("/messages")
            .append_header((header::AUTHORIZATION, "Bearer invalid"))
            .to_request();
        let res = call_service(&service, req).await;
        assert!(res.status().is_client_error());
    }

    #[actix_web::test]
    async fn test_extraneous_authorization_header() {
        let app = App::new().configure(make_config_factory().await.unwrap());
        let service = init_service(app).await;

        let req = TestRequest::get()
            .uri("/messages")
            .append_header((header::AUTHORIZATION, "Bearer token"))
            .to_request();
        let res = call_service(&service, req).await;
        assert!(res.status().is_success());
    }

    #[actix_web::test]
    async fn test_valid_authorization_header() {
        let config_factory =
            get_config_factory(SqliteBackend::new_test().await.unwrap(), Some("token")).unwrap();
        let app = App::new().configure(config_factory);
        let service = init_service(app).await;

        let req = TestRequest::get()
            .uri("/messages")
            .append_header((header::AUTHORIZATION, "Bearer token"))
            .to_request();
        let res = call_service(&service, req).await;
        assert!(res.status().is_success());
    }

    #[actix_web::test]
    async fn test_cors_header() {
        let app = App::new().configure(make_config_factory().await.unwrap());
        let service = init_service(app).await;

        let req = TestRequest::get().uri("/messages").to_request();
        let res = call_service(&service, req).await;
        assert!(res.status().is_success());
        assert_eq!(res.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(), "*");
    }

    #[actix_web::test]
    async fn test_filter_ids() {
        let app = App::new().configure(make_config_factory().await.unwrap());
        let service = init_service(app).await;

        let req = TestRequest::get().uri("/messages?ids=1").to_request();
        let res = call_service(&service, req).await;
        assert!(res.status().is_success());

        let req = TestRequest::get().uri("/messages?ids=1,2,3").to_request();
        let res = call_service(&service, req).await;
        assert!(res.status().is_success());

        let req = TestRequest::get().uri("/messages?ids=1,2,a").to_request();
        let res = call_service(&service, req).await;
        assert!(res.status().is_client_error());
    }

    #[actix_web::test]
    async fn test_filter_mailbox() {
        let app = App::new().configure(make_config_factory().await.unwrap());
        let service = init_service(app).await;

        let req = TestRequest::get().uri("/messages?mailbox=foo").to_request();
        let res = call_service(&service, req).await;
        assert!(res.status().is_success());
    }

    #[actix_web::test]
    async fn test_filter_states() {
        let app = App::new().configure(make_config_factory().await.unwrap());
        let service = init_service(app).await;

        let req = TestRequest::get()
            .uri("/messages?states=unread")
            .to_request();
        let res = call_service(&service, req).await;
        assert!(res.status().is_success());

        let req = TestRequest::get()
            .uri("/messages?states=read,archived")
            .to_request();
        let res = call_service(&service, req).await;
        assert!(res.status().is_success());

        let req = TestRequest::get()
            .uri("/messages?states=unread,foo")
            .to_request();
        let res = call_service(&service, req).await;
        assert!(res.status().is_client_error());
    }

    #[actix_web::test]
    async fn test_filter_multiple() {
        let app = App::new().configure(make_config_factory().await.unwrap());
        let service = init_service(app).await;

        let req = TestRequest::get()
            .uri("/messages?ids=1,2,3&mailbox=foo&states=unread,read")
            .to_request();
        let res = call_service(&service, req).await;
        assert!(res.status().is_success());
    }

    #[actix_web::test]
    async fn test_delete_no_filter() {
        let app = App::new().configure(make_config_factory().await.unwrap());
        let service = init_service(app).await;

        let req = TestRequest::delete().uri("/messages").to_request();
        let res = call_service(&service, req).await;
        assert!(res.status().is_client_error());
    }

    #[actix_web::test]
    async fn test_messages() {
        let app = App::new().configure(make_config_factory().await.unwrap());
        let service = init_service(app).await;

        let req = TestRequest::get().uri("/messages").to_request();
        let res = call_service(&service, req).await;
        assert!(res.status().is_success());
    }

    #[actix_web::test]
    async fn test_mailboxes() {
        let app = App::new().configure(make_config_factory().await.unwrap());
        let service = init_service(app).await;

        let req = TestRequest::get().uri("/mailboxes").to_request();
        let res = call_service(&service, req).await;
        assert!(res.status().is_success());
    }

    #[actix_web::test]
    async fn test_create_single_message() {
        let app = App::new().configure(make_config_factory().await.unwrap());
        let service = init_service(app).await;

        let req = TestRequest::post()
            .uri("/messages")
            .append_header(header::ContentType::json())
            .set_payload(
                r#"{
  "mailbox": "my-script",
  "content": "Hello, world!",
  "state": "read"
}"#,
            )
            .to_request();
        let res = call_service(&service, req).await;
        assert!(res.status().is_success());
    }

    #[actix_web::test]
    async fn test_create_multiple_messages() {
        let app = App::new().configure(make_config_factory().await.unwrap());
        let service = init_service(app).await;

        let req = TestRequest::post()
            .uri("/messages")
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
        let res = call_service(&service, req).await;
        assert!(res.status().is_success());
    }

    #[actix_web::test]
    async fn test_update_messages() {
        let app = App::new().configure(make_config_factory().await.unwrap());
        let service = init_service(app).await;

        let req = TestRequest::put()
            .uri("/messages?states=unread")
            .append_header(header::ContentType::json())
            .set_payload(r#"{"new_state": "read"}"#)
            .to_request();
        let res = call_service(&service, req).await;
        assert!(res.status().is_success());
    }

    #[actix_web::test]
    async fn test_delete_messages() {
        let app = App::new().configure(make_config_factory().await.unwrap());
        let service = init_service(app).await;

        let req = TestRequest::delete()
            .uri("/messages?states=unread")
            .to_request();
        let res = call_service(&service, req).await;
        assert!(res.status().is_success());
    }
}
