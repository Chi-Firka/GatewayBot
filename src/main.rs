use dotenvy::dotenv;
use rand::seq::SliceRandom;
use serde::Deserialize;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;
use std::env;
use std::fs;
use serenity::all::{CreateEmbed, CreateEmbedFooter};

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
                if let Err(why) = msg.channel_id
                    .say(&ctx.http, "Увы, меня сломали. Не будет тебе верифа")
                    .await {println!("Error sending message: {why:?}")} return;
            }

            let mut lines_arr: Vec<usize> = (0..total_lines).collect();

            {   // thread safety
                let mut rng = rand::rng();
                lines_arr.shuffle(&mut rng);
            }

            let mut shuffled_lines = Vec::new();
            for (i, &index) in lines_arr.iter().take(q_num).enumerate() {
                let line = &self.config.lines[index];
                let number = format!("{}. {}", i+1, line);
                shuffled_lines.push(number);
            }

            let embed = CreateEmbed::new()
                .title("Ответьте на следующие вопросы для продолжения:")
                .description(shuffled_lines.join("\n\n"))
                .footer(CreateEmbedFooter::new("Это нужно для уверенности в том, что вы не бот."));
            if let Err(why) = msg.channel_id.send_message(&ctx.http,
                                     // how TF does this work
                                     serenity::builder::CreateMessage::new().embed(embed))
                                     .await {println!("Error sending embed: {why:?}")};
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN")
        .expect("Expected a token in the environment");
    let read_config = fs::read_to_string("config.json")
        .expect("Err reading config file");
    let config: Config = serde_json::from_str(&read_config)
        .expect("Err parsing config");

    // what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler{config}).await.expect("Err creating client");
    if let Err(why) = client.start().await {println!("Client error: {why:?}")}
}
