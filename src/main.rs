use dotenvy::dotenv;
use rand::seq::SliceRandom;
use serde::Deserialize;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;
use std::env;
use std::fs;

#[derive(Deserialize, Debug)]
struct Config {
    questions_number: usize,
    lines: Vec<String>,
}

struct Handler {
    config: Config,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!verify" {
            let q_num = self.config.questions_number;
            let total_lines = self.config.lines.len();

            if q_num > total_lines {
                if let Err(why) = msg
                    .channel_id
                    .say(&ctx.http, "Увы, меня сломали. Не будет тебе верифа")
                    .await
                {
                    println!("Error sending message: {why:?}");
                }
                return;
            }

            let mut lines_arr: Vec<usize> = (0..total_lines).collect();

            {   // thread safety
                let mut rng = rand::rng();
                lines_arr.shuffle(&mut rng);
            }

            let mut shuffled_lines = Vec::new();
            for &index in lines_arr.iter().take(q_num) {
                let line = &self.config.lines[index];
                shuffled_lines.push(line.clone());
            }

            let message = shuffled_lines.join("\n\n");
            if let Err(why) = msg.channel_id.say(&ctx.http, message).await {
                println!("Err sending message: {why:?}");
            }
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let read_config = fs::read_to_string("config.json").expect("Err reading config file");
    let config: Config = serde_json::from_str(&read_config).expect("Err parsing config");

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    // Create a new instance of the Client, logging in as a bot.
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler { config })
        .await
        .expect("Err creating client");

    // Start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
