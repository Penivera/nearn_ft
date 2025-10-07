use actix_web::{post,HttpResponse,Responder};
use near_sdk::AccountId;
use serde::{Deserialize,Serialize};
use std::env;
use dotenv::dotenv;

#[derive(Deserialize,Serialize)]
struct TokenTransferRequest{
    reciever_id:String,
    amount:String,
    memo:Option<String>
}

pub struct AccountConfig{
    account_id:AccountId,
    private_key:String,

}

impl AccountConfig{
    pub fn new()->Self{
        dotenv().ok();
        Self{
            account_id: env::var("NEAR_ACCOUNT_ID").expect("NEAR_ACCOUNT_ID NOT FOUND"),
            private_key:env::var("NEAR_PRIVATE_KEY").expect("NEAR_PRIVATE_KEY NOT FOUND")

        } 
    }
}


#[post("/transfer")]
async fn ft_transfer() -> impl Responder {
    HttpResponse::Ok().body("coming soon")

}
