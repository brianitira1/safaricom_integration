use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use actix_cors::Cors;
use base64::{engine::general_purpose, Engine as _};
use reqwest::Client;
use serde_json::{json, Value};
use serde::Deserialize;
use std::env;
use chrono::Utc;

#[derive(Deserialize)]
struct StkPushInfo {
    phone_number: String,
    amount: String,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Define environment variables
    env::set_var("SECRET_KEY", "lk2J0CJ8nz44VYUj");
    env::set_var("CONSUMER_KEY", "GlczBB2hH6RPr3J0R5SuzatG76bz4ulC");

    // Initialize Actix web server
    HttpServer::new(|| {
        App::new()
            .wrap(Cors::permissive()) // Enable CORS
            .route("/", web::get().to(index))
            .route("/token", web::post().to(create_token))
            .route("/stkpush", web::post().to(stk_push))
    })
    .bind(("0.0.0.0", 5000))?
    .run()
    .await
}

async fn index() -> impl Responder {
    HttpResponse::Ok().body("Safaricom integration with Brian Itira")
}

async fn create_token() -> impl Responder {
    let secret_key = env::var("SECRET_KEY").unwrap();
    let consumer_key = env::var("CONSUMER_KEY").unwrap();
    let auth = general_purpose::STANDARD.encode(format!("{}:{}", consumer_key, secret_key));

    let client = Client::new();
    let res = client
        .get("https://sandbox.safaricom.co.ke/oauth/v1/generate?grant_type=client_credentials")
        .header("Authorization", format!("Basic {}", auth))
        .send()
        .await
        .unwrap();

    let token = res.text().await.unwrap();

    HttpResponse::Ok().content_type("application/json").body(token)
}

async fn stk_push(info: web::Json<StkPushInfo>) -> impl Responder {
    let secret_key = env::var("SECRET_KEY").unwrap();
    let consumer_key = env::var("CONSUMER_KEY").unwrap();
    let auth = general_purpose::STANDARD.encode(format!("{}:{}", consumer_key, secret_key));

    let token = match get_access_token(&auth).await {
        Ok(t) => t,
        Err(e) => return HttpResponse::InternalServerError().json(json!({
            "error": "Failed to get access token",
            "details": e.to_string()
        })),
    };

    let business_short_code = "174379";
    let pass_key = "bfb279f9aa9bdbcf158e97dd71a467cd2e0c893059b10f78e6b72ada1ed2c919";
    let timestamp = Utc::now().format("%Y%m%d%H%M%S").to_string();
    let password = general_purpose::STANDARD.encode(format!("{}{}{}", business_short_code, pass_key, timestamp));
    let callback_url = "https://mydomain.com/path";

    let request_payload = json!({
        "BusinessShortCode": business_short_code,
        "Password": password,
        "Timestamp": timestamp,
        "TransactionType": "CustomerPayBillOnline",
        "Amount": info.amount,
        "PartyA": info.phone_number,
        "PartyB": business_short_code,
        "PhoneNumber": info.phone_number,
        "CallBackURL": callback_url,
        "AccountReference": "Mpesa Test",
        "TransactionDesc": "Testing stk push",
    });

    let client = Client::new();
    let res = match client
        .post("https://sandbox.safaricom.co.ke/mpesa/stkpush/v1/processrequest")
        .header("Authorization", format!("Bearer {}", token))
        .json(&request_payload)
        .send()
        .await {
            Ok(response) => response,
            Err(e) => return HttpResponse::InternalServerError().json(json!({
                "error": "Failed to send STK push request",
                "details": e.to_string()
            })),
        };

    if res.status().is_success() {
        match res.json::<Value>().await {
            Ok(result) => HttpResponse::Ok().json(result),
            Err(e) => HttpResponse::InternalServerError().json(json!({
                "error": "Failed to parse response",
                "details": e.to_string()
            })),
        }
    } else {
        let error_message = res.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        HttpResponse::InternalServerError().json(json!({
            "error": "Failed to initiate STK push",
            "details": error_message
        }))
    }
}

async fn get_access_token(auth: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = Client::new();
    let res = client
        .get("https://sandbox.safaricom.co.ke/oauth/v1/generate?grant_type=client_credentials")
        .header("Authorization", format!("Basic {}", auth))
        .send()
        .await?;

    if res.status().is_success() {
        let result: Value = res.json().await?;
        Ok(result["access_token"].as_str().unwrap_or("").to_string())
    } else {
        Err(format!("Failed to get access token: {}", res.text().await?).into())
    }
}