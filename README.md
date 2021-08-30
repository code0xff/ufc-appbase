# UFC-APPBASE
Open Source Universal Framework for Chains based on Appbase

## What is UFC
UFC is open source framework for supporting blockchain service. You can easily create your own blockchain framework using UFC.

## UFC Spec
### Basic
- [Rust](https://www.rust-lang.org/)
- [appbase-rs](http://github.com/turnpike/appbase-rs.git)
- [JSON-RPC 2.0 by Parity](https://github.com/paritytech/jsonrpc.git)
- [RocskDB](https://rocksdb.org/)

### Optional
- MySQL
- MongoDB
- Email (SMTP)
- [Slack (Incoming Webhooks)](https://api.slack.com/messaging/webhooks)
- [Telegram (Telegram Bot)](https://core.telegram.org/bots)

### Chain Plugin
- Tendermint (for Cosmos SDK based Blockchain)
- Ethereum (PoC)

## UFC Features
### Basic Features
UFC is based on appbase-rs. It ported C++ appbase of EOS node. UFC basically uses JSON-RPC server developed by Parity and RocksDB.

### Optional Features
UFC provides the plugin to support to save data on MySQL database and MongoDB, and specially if you want to save data on MySQL DB, you could create JSON Schema file on proper path in this project directory. (ex. ufc-appbase/schema/tm_mysql.json) UFC reads the file and automatically creating table and insert query.

## How to use
UFC has Tendermint and Ethereum Plugin, so if you want to monitor or archive block and tx of Cosmos SDK based blockchain or Ethereum, clone this project and run UFC.

### Run UFC
```shell
cargo run --package ufc-appbase --bin ufc-appbase -- --config-dir .
```

### Request example to subscribe using JSON-RPC
```shell
curl --location --request POST 'localhost:8080' \
--header 'Content-Type: application/json' \
--data-raw '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tm_subscribe",
    "params": { 
        "target": "block",
        "sub_id": "cosmoshub-4",
        "start_height": 7364551,
        "nodes": ["https://cosmoshub-4--lcd--full.datahub.figment.io/apikey/..."]
    }
}'
```

### Save to MySQL DB
1. download MariaDB docker image and run docker image
```shell
docker pull mariadb
docker run -d -p 3306:3306 -e MYSQL_ROOT_PASSWORD=mariadb -e MYSQL_DATABASE=ufc --name mariadb mariadb
```

2. edit config.toml or add flag on UFC running command
```toml
[tendermint]
block-mysql-sync=true # save cosmos block to mysql db
tx-mysql-sync=true # save cosmos tx to mysql db

[ethereum]
block-mysql-sync=true # save ethereum block to mysql db
tx-mysql-sync=true # save ethereum tx to mysql db

[app]
plugin=["MySqlPlugin"]
```
or
```shell
# add to save cosmos block to mysql db
cargo run --package ufc-appbase --bin ufc-appbase -- --config-dir . --plugin MySqlPlugin --tm-block-mysql-sync  
```

3. request to subscribe block or tx

### Save to MongoDB
1. download MongoDB docker image and run docker image
```shell
docker pull mongo
docker run -d -p 27017:27017 -e MONGO_INITDB_ROOT_USERNAME=root -e MONGO_INITDB_ROOT_PASSWORD=mongodb --name mongo
```

2. edit config.toml or add flag on UFC running command
```toml
[tendermint]
block-mongo-sync=true # save cosmos block to mongodb
tx-mongo-sync=true # save cosmos tx to mongodb

[ethereum]
block-mongo-sync=true # save ethereum block to mongodb
tx-mongo-sync=true # save ethereum tx to mongodb

[app]
plugin=["MongoPlugin"]
```
or
```shell
# add to save cosmos block to mongo db
cargo run --package ufc-appbase --bin ufc-appbase -- --config-dir . --plugin MongoPlugin --tm-block-mongo-sync 
```

3. request to subscribe block or tx

### Publish RabbitMQ message
1. download RabbitMQ docker image and run docker image
```shell
docker pull rabbitmq
docker run -d --name rabbitmq -p 5672:5672 -p 15672:15672 --restart=unless-stopped -e RABBITMQ_DEFAULT_USER=rabbitmq -e RABBITMQ_DEFAULT_PASS=rabbitmq rabbitmq:management
```

2. edit config.toml or add flag on UFC running command
```toml
[tendermint]
block-rabbit-mq-publish=true # publish cosmos block message
tx-rabbit-mq-publish=true # publish cosmos tx message

[ethereum]
block-rabbit-mq-publish=true # publish cosmos block message
tx-rabbit-mq-publish=true # publish cosmos tx message

[app]
plugin=["RabbitPlugin"]
```
or
```shell
# add to publish cosmos block message
cargo run --package ufc-appbase --bin ufc-appbase -- --config-dir . --plugin RabbitPlugin --tm-block-rabbit-mq-publish 
```

3. request to subscribe block or tx

### Send Email through Gmail
1. [set up your account to send email](https://support.google.com/mail/answer/7126229?hl=en)

2. add sending email code
```rust
// code example
let email_msg = EmailMsg::new(String::from("to_address@domain.dev"), String::from("Test Subject"), String::from("Email Contents"));
let _ = email_channel.send(email_msg);
```

3. edit config.toml or add flag on UFC running command
```toml
[email]
smtp-username="smtp_username"
smtp-password="smtp_password"
smtp-relay="smtp.gmail.com"
from="NoBody <nobody@domain.dev>"
reply-to="NoBody <nobody@domain.dev>"

[app]
plugin=["EmailPlugin"]
```
or
```shell
cargo run --package ufc-appbase --bin ufc-appbase -- --config-dir . --plugin EmailPlugin --smtp-username [username] --smtp-password [password]
```

### Send Slack
1. create workspace and add channel
- [Create Workspace Guide](https://slack.com/intl/en-kr/help/articles/206845317-Create-a-Slack-workspace)
- [Create Channel Guide](https://slack.com/intl/en-kr/help/articles/201402297-Create-a-channel)

2. add slack app and activate incoming webhooks
- [Setup App Guide](https://api.slack.com/authentication/basics)
- [Setup Incoming Hooks Guide](https://api.slack.com/messaging/webhooks)

3. add sending slack code
```rust
// code example
let slack_msg = SlackMsg::new(String::from("https://hooks.slack.com/services/slack_hook_address"), String::from("Slack Test Message"));
let _ = slack_channel.send(slack_msg);
```

4. edit config.toml or add flag on UFC running command
```toml
[app]
plugin=["SlackPlugin"]
```
or
```shell
cargo run --package ufc-appbase --bin ufc-appbase -- --config-dir . --plugin SlackPlugin
```

### Send Telegram
1. create telegram bot and get telegram bot token

2. add sending telegram
```rust
// code example
let telegram_msg = TelegramMsg::new(String::from("chat_id"), String::from("Test Text"));
let _ = telegram_channel.send(telegram_msg);
```

3. edit config.toml or add flag on UFC running command
```toml
[telegram]
bot-token="TELEGRAM_BOT_TOKEN"

[app]
plugin=["TelegramPlugin"]
```
or
```shell
cargo run --package ufc-appbase --bin ufc-appbase -- --config-dir . --plugin TelegramPlugin --telegram-bot-token [TELEGRAM_BOT_TOKEN]
```

## Add New Plugin
To add plugins to support other blockchains, please refer to the existing tendermint or ethereum plugins.

If you look at the two plugins, you will realize that the parts that need to communicate directly with the blockchain node were written individually, but most of the functions utilized other plugins.

Build your own blockchain framework with UFC!