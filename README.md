# discord-bot
You will need a `src/bot.json` file with these contents:

```json
{
  "token": "your_unique_bot_token",
  "intents": ["list", "of", "intents", "in", "lower_snake_case"],
  "admins": ["user_with_admin_access_id", "another_admin_id"],
  "channel_blacklist": [
    "id_of_channel_bot_will_not_learn_from", 
    "another_id"
  ],
  "announcement_channels": [
    "id_of_channel_bot_will_send_announcements",
    "another_id"
  ]
}
```
