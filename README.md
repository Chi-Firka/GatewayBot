# Gateway Bot

---

- Powered by Serenity
- Used for "gateway" Discord moderation
- Created to fight with spammers

## Usage / building from source:
1. **Clone the repository:**
   ```
   git clone 
   ```
2. **Build the repository:**
   </br></br>
   for regular usage:
   ```
   cargo build --release
   ```
   for developing:
   ```
   cargo build
   ```
   or use `cargo run`/`cargo run release`
   </br></br>
3. Create `.env` file and paste your bot's token there:
   ```
   DISCORD_TOKEN=...
   ```
4. Set up your `config.json`.
5. It's time to set up your Discord bot, if you didn't.
6. Run the program.

## Discord bot setup:
1. [Visit Discord Developer Portal](https://discord.com/developers/applications) and create an application.
2. Navigate to the `Bot` section and `Reset Token`. Copy it.
3. Turn on the following Intents (still the `Bot` section):
   ![img.png](readme-imgs/img-intents.png)
4. Navigate to `Installation` tab, uncheck `User Install`, give necessary permissions:
   ![img.png](readme-imgs/img-installation.png)
5. Copy `Install link` and invite your bot somewhere.

## Available commands:
- `!verify` - send 3 random strings from `config.json` to a Discord channel the command was called from.