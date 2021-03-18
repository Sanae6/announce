# announce-au
A reimplementation of the protocol and server Innersloth uses to push announcements to Among Us uses.

## Usage
1. Download the binary from [releases](https://github.com/Sanae6/announce/releases/tag/tag-master)
and store it in a folder
2. Download the [config.toml](https://github.com/Sanae6/announce/blob/master/config.toml) file and
put it in the same folder.
3. Change the information in the config file, like the id used for caching and the messages you want to be sent to your users.
4. Run the binary you downloaded, and start Among Us with your custom region selected.
5. Profit.

## Building
1. [Install Rust](https://rustup.rs/)
2. Customize `config.toml` however you want
2. Run `cargo run --features binary`
