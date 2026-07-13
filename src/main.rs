use std::path::Path;
use std::collections::HashMap;
use dotenvy::dotenv;
use rand::seq::SliceRandom;
use serde::Deserialize;
use serenity::all::{ButtonStyle, ChannelId, ChannelType, ComponentInteraction, CreateAllowedMentions, CreateAttachment, CreateButton, CreateEmbed, CreateEmbedFooter, CreateMessage, Http, Interaction, PermissionOverwrite, PermissionOverwriteType, Permissions, RoleId, Timestamp, UserId};
use serenity::async_trait;
use serenity::builder::{
    CreateChannel, CreateInteractionResponse, CreateInteractionResponseMessage,
};
use serenity::model::channel::Message;
use serenity::prelude::*;
use std::env;
use std::fs;
use std::sync::Arc;
use rand::prelude::IndexedRandom;

#[derive(Deserialize, Debug)]
struct Config {
    questions_number: usize,
    delete_delay_seconds: u64,
    auto_delete_on_verdict: bool,
    category_id: u64,
    log_channel_id: u64,
    moderator_role_ids: Vec<u64>,
    verified_role_ids: Vec<u64>,
    lines: Vec<String>,
}

struct Handler {
    config: Config,
    active_deletions: Arc<Mutex<HashMap<ChannelId, tokio::sync::oneshot::Sender<()>>>>
}

fn is_admin(ctx: &Context, msg: &Message) -> bool {
    if let Some(guild_id) = msg.guild_id {
        if let Some(partial_member) = &msg.member {
            // owner and roles check
            if let Some(guild) = ctx.cache.guild(guild_id) {
                if guild.owner_id == msg.author.id {
                    return true;
                }
                for role_id in &partial_member.roles {
                    if let Some(role) = guild.roles.get(role_id) {
                        if role.permissions.administrator() {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

fn is_moderator(config: &Config, interaction: &ComponentInteraction) -> bool {
    let partial_member = match &interaction.member {
        Some(member) => member,
        None => return false,
    };
    partial_member
        .roles
        .iter()
        .any(|role_id| config.moderator_role_ids.contains(&role_id.get()))
}

async fn extract_user_id_from_channel(ctx: &Context, channel_id: ChannelId) -> Option<UserId> {
    let channel = channel_id.to_channel(&ctx.http).await.ok()?;
    let guild_channel = channel.guild()?;
    let channel_name = guild_channel.name();
    let user_id_str = channel_name.split('-').last()?;
    let target_user_id = user_id_str.parse::<u64>().ok()?;
    Some(UserId::new(target_user_id))
}

async fn start_channel_deletion_timer(
    channel_id: ChannelId,
    config: &Config,
    delay_override: u64,
    active_deletions: Arc<Mutex<HashMap<ChannelId, tokio::sync::oneshot::Sender<()>>>>,
    http: Arc<Http>,
) {
    let mut deletions = active_deletions.lock().await;

    if deletions.contains_key(&channel_id) {
        return;
    }

    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    deletions.insert(channel_id, cancel_tx);
    drop(deletions);

    let delay;
    if delay_override > 0 {
        delay = delay_override;
    } else {
        delay = config.delete_delay_seconds;
    }

    let deletions_clone = active_deletions.clone();

    tokio::spawn(async move {
        tokio::select! {
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(delay)) => {
                let mut deletions = deletions_clone.lock().await;
                deletions.remove(&channel_id);
                drop(deletions);

                let _ = channel_id.delete(&http).await;
            }
            _ = cancel_rx => {}
        }
    });
}

async fn send_log(ctx: &Context, log_channel_id: u64, text: String) {
    let log_channel = ChannelId::new(log_channel_id);
    // mute all pings
    let allowed_mentions = CreateAllowedMentions::new();
    let _ = log_channel
        .send_message(
            &ctx.http,
            CreateMessage::new()
                .content(text)
                .allowed_mentions(allowed_mentions)
        )
        .await;
}

async fn get_random_attachment() -> Option<CreateAttachment> {
    let path = Path::new("attachments");
    let entries = fs::read_dir(path).ok()?;
    let mut files = Vec::new();

    for entry in entries.flatten() {
        if let Ok(file_type) = entry.file_type() {
            if file_type.is_file() {
                files.push(entry.path());
            }
        }
    }
    if files.is_empty() {
        return None;
    }

    let chosen_path = {
        let mut rng = rand::rng();
        files.choose(&mut rng)?.clone()
    };

    // make a spoiler
    let file_name = chosen_path.file_name()?.to_str()?;
    let spoiler_name = format!("SPOILER_{}", file_name);
    let mut attachment = CreateAttachment::path(chosen_path).await.ok()?;
    attachment.filename = spoiler_name;

    Some(attachment)
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!init" {
            if is_admin(&ctx, &msg) == false {
                return;
            }

            if let Err(why) = msg
                .channel_id
                .send_message(
                    &ctx.http,
                    CreateMessage::new()
                        .content("Для получения доступа к общим каналам сервера, \
                                  вам необходимо пройти простую верификацию.\n\
                                  Для вас автоматически будет подобрано несколько вопросов. \
                                  После того, как вашу верификацию одобрят, вы сможете начать общение.")
                        .button(CreateButton::new("verify_button").label("Начать")),
                )
                .await
            {
                println!("Err sending embed: {why:?}")
            };
        }
    }
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let component = match interaction {
            Interaction::Component(comp) => comp,
            _ => return,
        };
        let guild_id = match component.guild_id {
            Some(guild) => guild,
            None => return,
        };

        if component.data.custom_id == "verify_button" {
            let target_channel_name = format!("{}-{}", component.user.name, component.user.id);
            let category_id = ChannelId::new(self.config.category_id);
            let mut category_permissions: Vec<PermissionOverwrite> = Vec::new();

            if let Ok(channels) = guild_id.channels(&ctx.http).await {
                let already_exists = channels
                    .values()
                    .any(|ch| ch.parent_id == Some(category_id)
                        && ch.name == target_channel_name);

                if already_exists {
                    let _ = component
                        .create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("У вас уже есть открытый канал верификации.")
                                    .ephemeral(true),
                            ),
                        )
                        .await;
                    return;
                }

                if let Some(category) = channels.get(&category_id) {
                    category_permissions = category.permission_overwrites.clone();
                }
            }

            category_permissions.push(PermissionOverwrite {
                allow: Permissions::VIEW_CHANNEL
                    | Permissions::SEND_MESSAGES
                    | Permissions::READ_MESSAGE_HISTORY,
                deny: Permissions::empty(),
                kind: PermissionOverwriteType::Member(component.user.id),
            });

            let builder = CreateChannel::new(&target_channel_name)
                .kind(ChannelType::Text)
                .category(category_id)
                .permissions(category_permissions);

            if let Ok(new_channel) = guild_id.create_channel(&ctx.http, builder).await {
                // generate questions
                let mut q_num = self.config.questions_number;
                let total_lines = self.config.lines.len();
                if q_num > total_lines {
                    q_num = total_lines;
                };

                let mut lines_arr: Vec<usize> = (0..total_lines).collect();
                {   // thread safety
                    let mut rng = rand::rng();
                    lines_arr.shuffle(&mut rng);
                }

                let mut shuffled_lines = Vec::new();
                for (i, &index) in lines_arr.iter().take(q_num).enumerate() {
                    let raw_line = &self.config.lines[index];
                    let numbered_line = format!("{}. {}", i + 1, raw_line);
                    shuffled_lines.push(numbered_line);
                }

                let embed = CreateEmbed::new()
                    .title("Ответьте на следующие вопросы для продолжения:")
                    .description(shuffled_lines.join("\n\n"))
                    .footer(CreateEmbedFooter::new("Это проверка на то, что вы не бот."));
                // send messages
                let mut message_builder = CreateMessage::new()
                    .content(format!(
                        "<@{}> :wave:\n\
                        -# Аккаунт создан <t:{}:R>\n",
                        component.user.id,
                        component.user.id.created_at().unix_timestamp()
                    ))
                    .embed(embed)
                    .button(
                        CreateButton::new("accept_button")
                            .emoji('🛡')
                            .label("Принять")
                            .style(ButtonStyle::Success)
                    )
                    .button(
                        CreateButton::new("deny_button")
                            .emoji('🛡')
                            .label("Отклонить")
                            .style(ButtonStyle::Danger)
                    )
                    .button(
                        CreateButton::new("initiate_delete_button")
                            .emoji('🗑')
                            .label("Удалить")
                            .style(ButtonStyle::Secondary)
                    );

                if let Some(attachment) = get_random_attachment().await {
                    message_builder = message_builder.add_file(attachment);
                }

                let _ = new_channel.id.send_message(&ctx.http, message_builder).await;

                let _ = component
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(format!("Канал создан: <#{}>", new_channel.id))
                                .ephemeral(true),
                        ),
                    )
                    .await;
                // logging
                let log_text = format!(
                    "❔ Участник <@{}> (`{}`) запросил верификацию <t:{}:R>",
                    component.user.id, component.user.name, Timestamp::now().unix_timestamp()
                );
                send_log(&ctx, self.config.log_channel_id, log_text).await;
            }
            return;
        }

        let is_accept = component.data.custom_id == "accept_button";
        let is_deny = component.data.custom_id == "deny_button";
        let is_init_delete = component.data.custom_id == "initiate_delete_button";
        let is_delete_now = component.data.custom_id == "delete_now_button";
        let is_cancel_delete = component.data.custom_id == "cancel_delete_button";

        const AUTO_DELETE_DELAY_OVERRIDE: u64 = 2;
        if is_accept || is_deny {
            if !is_moderator(&self.config, &component) {
                let _ = component
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content("Недостаточно прав.")
                                .ephemeral(true),
                        ),
                    )
                    .await;
                return;
            }

            let target_user = match extract_user_id_from_channel(&ctx, component.channel_id).await {
                Some(id) => id,
                None => return,
            };

            let timestamp = Timestamp::now().unix_timestamp();

            // update the message after the first interaction
            // add status and disable the buttons
            let status_text = if is_accept {
                format!("✅ Принято модератором <@{}>", component.user.id)
            } else {
                format!("❌ Отклонено модератором <@{}>", component.user.id)
            };

            let new_content = format!(
                "{}\n\n{} <t:{timestamp}:R>",
                component.message.content, status_text
            );

            let _ = component
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new()
                            .content(new_content)
                            .button(
                                CreateButton::new("accept_button")
                                    .emoji('🛡')
                                    .label("Принять")
                                    .style(ButtonStyle::Success)
                                    .disabled(true),
                            )
                            .button(
                                CreateButton::new("deny_button")
                                    .emoji('🛡')
                                    .label("Отклонить")
                                    .style(ButtonStyle::Danger)
                                    .disabled(true),
                            )
                            .button(
                                CreateButton::new("initiate_delete_button")
                                    .emoji('🗑')
                                    .label("Удалить")
                                    .style(ButtonStyle::Secondary)
                            ),
                    ),
                )
                .await;
            // remove the member from channel perms
            let _ = component
                .channel_id
                .delete_permission(&ctx.http, PermissionOverwriteType::Member(target_user))
                .await;
            // get member's name from cache
            let target_user_obj = target_user.to_user(&ctx.http).await;
            let target_user_name = match &target_user_obj {
                Ok(user) => format!("(`{}`)", user.name),
                Err(_) => String::new(),
            };

            if is_accept {
                // give verified roles
                for role_raw in &self.config.verified_role_ids {
                    let _ = ctx
                        .http
                        .add_member_role(guild_id, target_user, RoleId::new(*role_raw), None)
                        .await;
                }
                let log_text = format!(
                    "✅ Участник <@{target_user}> {target_user_name} **принят** модератором <@{}> <t:{timestamp}:R>",
                    component.user.id
                );
                send_log(&ctx, self.config.log_channel_id, log_text).await;
            } else if is_deny {
                let log_text = format!(
                    "❌ Участник <@{target_user}> {target_user_name} **отклонён** модератором <@{}> <t:{timestamp}:R>",
                    component.user.id
                );
                send_log(&ctx, self.config.log_channel_id, log_text).await;
            }

            if self.config.auto_delete_on_verdict {
                let deletions = self.active_deletions.lock().await;
                if !deletions.contains_key(&component.channel_id) {
                    drop(deletions);

                    let delay_override = self.config.delete_delay_seconds * AUTO_DELETE_DELAY_OVERRIDE;

                    start_channel_deletion_timer(
                        component.channel_id,
                        &self.config,
                        delay_override,
                        self.active_deletions.clone(),
                        ctx.http.clone(),
                    ).await;

                    let delete_timestamp = Timestamp::now().unix_timestamp() + delay_override as i64;

                    let _ = component.channel_id.send_message(&ctx.http,
                              CreateMessage::new()
                                  .content(format!("Этот канал автоматически удалится <t:{delete_timestamp}:R>."))
                                  .button(
                                      CreateButton::new("delete_now_button")
                                          .label("Удалить сейчас")
                                          .style(ButtonStyle::Danger)
                                  )
                                  .button(
                                      CreateButton::new("cancel_delete_button")
                                          .label("Отменить автоудаление")
                                          .style(ButtonStyle::Secondary)
                                  ),
                    ).await;
                } else {
                    drop(deletions);
                }
            }
        }

        let channel_id = component.channel_id;

        if is_init_delete || is_delete_now || is_cancel_delete {
            if !is_moderator(&self.config, &component) {
                let _ = component
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content("Недостаточно прав.")
                                .ephemeral(true),
                        ),
                    )
                    .await;
                return;
            }

            if is_init_delete {
                let deletions = self.active_deletions.lock().await;
                if deletions.contains_key(&channel_id) {
                    let _ = component.create_response(&ctx.http, CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("Удаление уже в процессе!")
                            .ephemeral(true)
                    )).await;
                    return;
                }
                drop(deletions);

                start_channel_deletion_timer(
                    channel_id,
                    &self.config,
                    0,
                    self.active_deletions.clone(),
                    ctx.http.clone()
                ).await;

                let delay = self.config.delete_delay_seconds;
                let delete_timestamp = Timestamp::now().unix_timestamp() + delay as i64;

                let _ = component.create_response(&ctx.http, CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(format!("Этот канал будет удалён <t:{delete_timestamp}:R>."))
                        .button(
                            CreateButton::new("delete_now_button")
                                .label("Удалить сейчас")
                                .style(ButtonStyle::Danger)
                        )
                        .button(
                            CreateButton::new("cancel_delete_button")
                                .label("Отменить")
                                .style(ButtonStyle::Secondary)
                        ),
                ),
                ).await;
            }

            if is_delete_now {
                let mut deletions = self.active_deletions.lock().await;
                deletions.remove(&channel_id);
                drop(deletions);
                let _ = channel_id.delete(&ctx.http).await;
            }

            if is_cancel_delete {
                let mut deletions = self.active_deletions.lock().await;
                if let Some(cancel_tx) = deletions.remove(&channel_id) {
                    let _ = cancel_tx.send(());
                }
                drop(deletions);
                let _ = channel_id.delete_message(&ctx.http, component.message.id).await;
                let _ = component.create_response(&ctx.http, CreateInteractionResponse::Acknowledge).await;
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

    // what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILDS;
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler {
            config,
            active_deletions: Arc::new(Mutex::new(HashMap::new()))
        })
        .await
        .expect("Err creating client");
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}")
    }
}
