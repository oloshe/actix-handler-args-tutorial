mod auth;
mod user_data;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use user_data::UserData;

#[derive(Serialize, Deserialize)]
struct LoginDTO {
    pub id: i32,
    pub pwd: String,
}

/// 登陆
async fn login(body: web::Json<LoginDTO>) -> impl Responder {
    let token = auth::create_jwt(&body.id);
    HttpResponse::Ok().json(token)
}

/// 获取信息
/// 需要登陆
async fn get_info(user: UserData) -> impl Responder {
    println!("{:?}", user);
    HttpResponse::Ok().finish()
}

/// 获取公公消息
/// 无需登陆
/// 登陆前后拿到的数据不完全相同
async fn get_public_info(user: Option<UserData>) -> impl Responder {
    if let Some(user) = user {
        HttpResponse::Ok().json(format!("public data with {}", user.id))
    } else {
        HttpResponse::Ok().json("public data")
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let server = HttpServer::new(|| {
        App::new()
            .route("/login", web::post().to(login))
            .route("/info", web::post().to(get_info))
            .route("/public", web::post().to(get_public_info))
    })
    .bind("127.0.0.1:8000")?
    .run();

    server.await
}
