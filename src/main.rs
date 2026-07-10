use dotenvy::dotenv;
use rand::seq::SliceRandom;
use serde::Deserialize;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;
use std::env;
use std::fs;
use serenity::all::{
    CreateMessage, CreateEmbed,
    CreateEmbedFooter, CreateButton,
    ChannelType, Interaction
};
use serenity::builder::{
    CreateChannel,
    CreateInteractionResponse,
    CreateInteractionResponseMessage
};

#[derive(Deserialize, Debug)]
struct Config {
    questions_number: usize,
    lines: Vec<String>
}

struct Handler { config: Config }

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!init" {
            if let Err(why) = msg.channel_id
                .send_message(&ctx.http, CreateMessage::new()
                    .content("мяу")
                    .button(CreateButton::new("verify_button").label("Пройти верификацию")))
                .await {println!("Error sending embed: {why:?}")};
        }
        if msg.content == "!verify" {
            let q_num = self.config.questions_number;
            let total_lines = self.config.lines.len();

            if q_num > total_lines {
                if let Err(why) = msg.channel_id
                    .say(&ctx.http, "Увы, меня сломали. Верифа не будет.")
                    .await {println!("Error sending message: {why:?}")} return;
            }

            let mut lines_arr: Vec<usize> = (0..total_lines).collect();
            {   // thread safety
                let mut rng = rand::rng();
                lines_arr.shuffle(&mut rng);
            }

            let mut shuffled_lines = Vec::new();
            for (i, &index) in lines_arr.iter().take(q_num).enumerate() {
                let raw_line = &self.config.lines[index];
                let numbered_line = format!("{}. {}", i+1, raw_line);
                shuffled_lines.push(numbered_line);
            }

            let embed = CreateEmbed::new()
                .title("Ответьте на следующие вопросы для продолжения:")
                .description(shuffled_lines.join("\n\n"))
                .footer(CreateEmbedFooter::new("Это нужно для уверенности в том, что вы не бот."));

            if let Err(why) = msg.channel_id
                .send_message(&ctx.http, CreateMessage::new().embed(embed))
                .await {println!("Error sending embed: {why:?}")};
        }
    }
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Component(component) = interaction {
            if component.data.custom_id == "verify_button" {
                if let Some(guild_id) = component.guild_id {

                    let target_channel_name = format!("{}-{}",
                        component.user.name,
                        component.user.id);

                    if let Ok(channels) = guild_id.channels(&ctx.http)
                        .await {
                            let already_exists = channels.values()
                                .any(|ch| ch.name == target_channel_name);
                            if already_exists {
                                let _ = component.create_response(&ctx.http,
                                    CreateInteractionResponse::Message(CreateInteractionResponseMessage::new()
                                        .content("У вас уже есть открытый канал верификации.")
                                        .ephemeral(true))).await;
                                return;
                            }
                    }

                    let builder = CreateChannel::new(
                        format!("{target_channel_name}")).kind(ChannelType::Text);

                    match guild_id.create_channel(&ctx.http, builder).await {
                        Ok(new_channel) => {
                            let _ = component.create_response(&ctx.http,
                                CreateInteractionResponse::Message(CreateInteractionResponseMessage::new()
                                    .content(format!("Канал создан: <#{}>", new_channel.id))
                                    .ephemeral(true))).await;
                        },
                        Err(why) => {
                            println!("Ошибка создания канала: {:?}", why);
                        }
                    }
                }
            }
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
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILDS;
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler{config}).await.expect("Err creating client");
    if let Err(why) = client.start().await {println!("Client error: {why:?}")}
}
