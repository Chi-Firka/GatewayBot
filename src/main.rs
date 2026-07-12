use dotenvy::dotenv;
use rand::seq::SliceRandom;
use serde::Deserialize;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;
use std::env;
use std::fs;
use serenity::all::{
    CreateMessage, CreateEmbed, CreateEmbedFooter,
    CreateButton, ChannelType, Interaction,
    ChannelId, Permissions, PermissionOverwrite,
    PermissionOverwriteType};
use serenity::builder::{
    CreateChannel,
    CreateInteractionResponse,
    CreateInteractionResponseMessage};

#[derive(Deserialize, Debug)]
struct Config {
    questions_number: usize,
    category_id: u64,
    lines: Vec<String>
}

struct Handler { config: Config }

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!init" {
            // huge perms check
            let mut is_admin = false;
            if let Some(guild_id) = msg.guild_id {
                if let Some(partial_member) = &msg.member {
                    // is owner?
                    if let Some(guild) = ctx.cache.guild(guild_id) {
                        if guild.owner_id == msg.author.id {
                            is_admin = true;
                        } else {
                            // roles check
                            for role_id in &partial_member.roles {
                                if let Some(role) = guild.roles.get(role_id) {
                                    if role.permissions.administrator() {
                                        is_admin = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if !is_admin {return}

            if let Err(why) = msg.channel_id
                .send_message(&ctx.http, CreateMessage::new()
                    .content("мяу")
                    .button(CreateButton::new("verify_button").label("Пройти верификацию")))
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

                    let category_id = ChannelId::new(self.config.category_id);
                    let mut category_permissions: Vec<PermissionOverwrite> = Vec::new();

                    if let Ok(channels) = guild_id.channels(&ctx.http).await {
                        let already_exists = channels.values()
                            .any(|ch| ch.name == target_channel_name);
                        if already_exists {
                            let _ = component.create_response(&ctx.http,
                              CreateInteractionResponse::Message(CreateInteractionResponseMessage::new()
                                  .content("У вас уже есть открытый канал верификации.")
                                  .ephemeral(true))).await;
                            return;
                        }

                        if let Some(category) = channels.get(&category_id) {
                            category_permissions = category.permission_overwrites.clone();
                        }
                    }

                    category_permissions.push(PermissionOverwrite {
                        allow: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::READ_MESSAGE_HISTORY,
                        deny: Permissions::empty(),
                        kind: PermissionOverwriteType::Member(component.user.id),
                    });

                    let builder = CreateChannel::new(&target_channel_name)
                        .kind(ChannelType::Text)
                        .category(category_id)
                        .permissions(category_permissions);

                    match guild_id.create_channel(&ctx.http, builder).await {
                        Ok(new_channel) => {
                            let q_num = self.config.questions_number;
                            let total_lines = self.config.lines.len();

                            let mut embed = CreateEmbed::new();

                            if q_num > total_lines {
                                let _ = new_channel.id.say(&ctx.http, "Увы, меня сломали. Верифа не будет.").await;
                            } else {
                                let mut lines_arr: Vec<usize> = (0..total_lines).collect();
                                {
                                    let mut rng = rand::rng();
                                    lines_arr.shuffle(&mut rng);
                                }

                                let mut shuffled_lines = Vec::new();
                                for (i, &index) in lines_arr.iter().take(q_num).enumerate() {
                                    let raw_line = &self.config.lines[index];
                                    let numbered_line = format!("{}. {}", i + 1, raw_line);
                                    shuffled_lines.push(numbered_line);
                                }

                                embed = CreateEmbed::new()
                                    .title("Ответьте на следующие вопросы для продолжения:")
                                    .description(shuffled_lines.join("\n\n"))
                                    .footer(CreateEmbedFooter::new("Это нужно для уверенности в том, что вы не бот."));
                            }

                            if let Err(why) = new_channel.id
                                .send_message(&ctx.http, CreateMessage::new()
                                    .content(format!("<@{}> :wave:", component.user.id))
                                    .embed(embed))
                                .await {
                                println!("Err sending embed: {why:?}");
                            }

                            let _ = component.create_response(&ctx.http,
                                  CreateInteractionResponse::Message(CreateInteractionResponseMessage::new()
                                      .content(format!("Канал создан: <#{}>", new_channel.id))
                                      .ephemeral(true))).await;
                        },
                        Err(why) => {
                            println!("Err creating a channel: {:?}", why);
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
