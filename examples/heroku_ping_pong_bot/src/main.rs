// The version of Heroku ping-pong-bot, which uses a webhook to receive updates
// from Telegram, instead of long polling.

use teloxide::{dispatching::update_listeners, prelude::*, types::Update};

use std::{convert::Infallible, env, net::SocketAddr};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::Filter;

use reqwest::StatusCode;

#[tokio::main]
async fn main() {
    run().await;
}

async fn handle_rejection(error: warp::Rejection) -> Result<impl warp::Reply, Infallible> {
    log::error!("Cannot process the request due to: {:?}", error);
    Ok(StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn webhook<'a>(bot: AutoSend<Bot>) -> impl update_listeners::UpdateListener<Infallible> {
    // Heroku defines auto defines a port value
    let teloxide_token = env::var("TELOXIDE_TOKEN").expect("TELOXIDE_TOKEN env variable missing");
    let port: u16 = env::var("PORT")
        .expect("PORT env variable missing")
        .parse()
        .expect("PORT value to be integer");
    // Heroku host example .: "heroku-ping-pong-bot.herokuapp.com"
    let host = env::var("HOST").expect("have HOST env variable");
    let path = format!("bot{}", teloxide_token);
    let url = format!("https://{}/{}", host, path);

    bot.set_webhook(url).await.expect("Cannot setup a webhook");

    let (tx, rx) = mpsc::unbounded_channel();

    let server = warp::post()
        .and(warp::path(path))
        .and(warp::body::json())
        .map(move |json: serde_json::Value| {
            if let Ok(update) = Update::try_parse(&json) {
                tx.send(Ok(update)).expect("Cannot send an incoming update from the webhook")
            }

            StatusCode::OK
        })
        .recover(handle_rejection);

    let serve = warp::serve(server);

    let address = format!("0.0.0.0:{}", port);
    tokio::spawn(serve.run(address.parse::<SocketAddr>().unwrap()));
    UnboundedReceiverStream::new(rx)
}

async fn run() {
    teloxide::enable_logging!();
    log::info!("Starting heroku_ping_pong_bot...");

    let bot = Bot::from_env().auto_send();

    let cloned_bot = bot.clone();
    teloxide::repl_with_listener(
        bot,
        |message| async move {
            message.answer("pong").await?;
            respond(())
        },
        webhook(cloned_bot).await,
    )
    .await;
}
