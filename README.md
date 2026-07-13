# Gateway Bot

---

- Powered by [Serenity](https://github.com/serenity-rs/serenity)
- Used to assist Discord admins with onboarding verification
- Created as a tool that will help you fight against spammers

### The goal
is to create a convenient tool to ease onboarding moderation
on a Discord server, to protect it from spammers without
AutoMod setup, which is also pretty restricted in its functionality.

### How it is intended to work:
1. Someone joins your Discord server.
2. They answer some onboarding questions without having access
   to the rest of the server.
3. Admins are making decision whether to verify the member 
   and give the access to the server, or not.

---

## Discord bot setup:
1. [Visit Discord Developer Portal](https://discord.com/developers/applications) and create an application.
2. Navigate to the `Bot` section and `Reset Token`. Copy it.
3. Turn on the following Intents (still the `Bot` section):
   ![img.png](readme-imgs/img-intents.png)
4. Navigate to `Installation` tab, uncheck `User Install`, give necessary permissions:
   ![img.png](readme-imgs/img-installation.png)
5. Copy `Install link` and invite your bot somewhere.
6. Type and send `!init` anywhere on your Discord server.

## Usage / building from source:
1. **Clone the repository:**
   ```
   git clone https://github.com/Chi-Firka/GatewayBot.git
   ```
2. Create a `.env` file in the root directory and paste your token:
   ```
   DISCORD_TOKEN=...
   ```
3. **Build and run:**
   </br></br>
   for production:
   ```
   cargo run --release
   ```
   for developing:
   ```
   cargo run
   ```

> [!TIP]
> Create an attachments/ folder to automatically 
> include random spoiler attachments in verification messages.

---

## Available commands:
- `!init` - sends the initial greeting message with the verify button.
  Recommended to run in your entry/rules channel.

## Config format:
```json5
{
  // the number of strings that will be chosen RANDOMLY
  "questions_number": 3,
  // some obvious stuff
  "delete_delay_seconds": 60,
  "auto_delete_on_verdict": true,
  // the category for channel creation
  "category_id": 1234567890123456789,
  // self-explanatory imo
  "log_channel_id": 1122334455667788990,
  // roles that are allowed to interact with accept/deny buttons
  "moderator_role_ids": [1112223334445556667, 7788899900011122233],
  // roles for a verified member
  "verified_role_ids": [9998887776665554443],
  // the string array itself
  "lines": [
    "first str",
    "second str",
    "third str",
    // and so on...
  ]
}
```

## Troubleshooting:

If you can't find your issue, please check your `config.json`.

- The `!init` command doesn't do anything
  - Ensure you are the server owner or have the administrator permissions.
  - Check if the bot has access to view that channel and send messages there.
- Clicking "accept" doesn't assign roles to the user
  - Drag the bot's role higher that roles you want to give someone upon verification.