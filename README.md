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
- Slack (Incoming Webhooks)
- Telegram (Telegram Bot)

### Chain Plugin
- Tendermint (for Cosmos SDK based Blockchain)
- Ethereum (PoC)

## UFC Features
### Basic Features
UFC is based on appbase-rs. It is forked by C++ appbase of EOS node. UFC basically uses JSON-RPC server developed by Parity and RocksDB.

### Optional Features
UFC provides the plugin to support to save data on MySQL database and MongoDB, and specially if you want to save data on MySQL, you could create JSON Schema file on proper path on project. (ex. ufc-appbase/schema/tm_mysql.json) UFC reads the file and auto creating table and insert query.

## How to use