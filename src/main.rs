use actix::prelude::*;
use actix_redis::{Command, RedisActor};
use actix_web::{middleware, web, App, Error as AWError, HttpResponse, HttpServer};
use futures::future::join_all;
use redis_async::{resp::RespValue, resp_array};
use serde::Deserialize;

mod helpers;

async fn get_product(
    info: web::Json<helpers::Product>,
    redis: web::Data<Addr<RedisActor>>
) -> Result<HttpResponse, AWError> {
    // let info = info.into_inner();

    // let _res = redis.send(Command(resp_array!["GET", info.one]));

    Ok(HttpResponse::Ok().json(info.0))
}

async fn create_product(
    info: web::Json<helpers::Product>,
    redis: web::Data<Addr<RedisActor>>,
) -> Result<HttpResponse, AWError> {
    let info = info.into_inner();

    let setting = redis.send(Command(resp_array!["SET", info.id.to_string(), serde_json::to_string(&info).unwrap()]));

    // Creates a future which represents a collection of the results of the futures
    // given. The returned future will drive execution for all of its underlying futures,
    // collecting the results into a destination `Vec<RespValue>` in the same order as they
    // were provided. If any future returns an error then all other futures will be
    // canceled and an error will be returned immediately. If all futures complete
    // successfully, however, then the returned future will succeed with a `Vec` of
    // all the successful results.
    let res: Vec<Result<RespValue, AWError>> =
        join_all(vec![setting].into_iter())
            .await
            .into_iter()
            .map(|item| {
                item.map_err(AWError::from)
                    .and_then(|res| res.map_err(AWError::from))
            })
            .collect();

    // successful operations return "OK", so confirm that all returned as so
    if !res
        .iter()
        .all(|res| matches!(res,Ok(RespValue::SimpleString(x)) if x == "OK"))
    {
        Ok(HttpResponse::InternalServerError().finish())
    } else {
        Ok(HttpResponse::Ok().body("successfully cached values"))
    }
}

async fn delete_product(info: web::Json<helpers::Product>, redis: web::Data<Addr<RedisActor>>) -> Result<HttpResponse, AWError> {
    let res = redis
        .send(Command(resp_array![
            "DEL",
            info.id.to_string()
        ]))
        .await?;

    match res {
        Ok(RespValue::Integer(x)) if x == 3 => {
            Ok(HttpResponse::Ok().body("successfully deleted values"))
        }
        _ => {
            println!("---->{:?}", res);
            Ok(HttpResponse::InternalServerError().finish())
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=trace,actix_redis=trace");
    env_logger::init();

    HttpServer::new(|| {
        let redis_addr = RedisActor::start("127.0.0.1:6379");

        App::new()
            .data(redis_addr)
            .wrap(middleware::Logger::default())
            .service(
                web::resource("/product")
                    .route(web::get().to(get_product))
                    .route(web::post().to(create_product))
                    .route(web::delete().to(delete_product))
            )
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}