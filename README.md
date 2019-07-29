# Weechat Discord NG

A plugin that adds Discord to [Weechat](https://weechat.org/)

(Beta)

---

### Warning

***Usage of self-tokens is a violation of Discord's TOS***

This client makes use of the "user api" and is essentially a self-bot.
This client does not abuse the api however it is still a violation of the TOS.

Use at your own risk, using this program could get your account or ip disabled, banned, etc.

---


### Building

Dependencies:

* Weechat developer libraries. Usually called `weechat-dev`, or sometimes just `weechat` includes them.
* [Rust](https://www.rust-lang.org)

The makefile should give enough information for build commands. Here's the essentials:

    cd weechat-discord # or wherever you cloned it
    cargo build --release

This will produce a shared object called `target/release/libweecord.so` (or `.dylib` on macos). Place it in your weechat plugins directory, which is probably located at `~/.weechat/plugins` (may need to be created)

The Makefile has some automation that helps with development:

    make # (same as make all) just runs that `cargo build --release` command, produces weecord.so
    make install # builds and copies the .so to ~/.weechat/plugins, creating the dir if required
    make run # installs and runs `weechat -a` (-a means "don't autoconnect to servers")

Maybe important note: The previous version of this project, written in Go, used to get **really upset** when the .so was modified during the same weechat session, even if unloaded. When developing, make sure to completely quit weechat when updating the .so, just to be sure (otherwise you might get a SIGSEGV and hard crash).

### Usage

Due to some idiocracy on Discord's part, [you will need to obtain a login token](https://github.com/hammerandchisel/discord-api-docs/issues/69#issuecomment-223886862). 
You can either use a python script to find the tokens, or try and grab them manually.

#### Python Script

`find_token.py` is a simple python3 script to search the computer for localstorage databases. It will present a list of all found databases.

If ripgrep is installed it will use that, if not, it will use `find`.


#### Manual

In the devtools menu of the website and desktop app (ctrl+shift+i or ctrl+opt+i) Application tab > Local Storage on left, discordapp.com, token entry.

When this was written, discord deletes its token from the visible table, so you may need to refresh the page (ctrl/cmd+r) and grab the token as it is refreshing.


#### Setting up

First, you either need to load the plugin, I have it set to autoload.

Then, set that token:

    /discord token 123456789ABCDEF
   
This saves the discord token in `<weechatdir>/plugins.conf`, **so make sure not to commit this file or share it with anyone.**

Then, connect:

    /discord connect

If you want to always connect on load, you can enable autostart with:

    /discord autostart

Note you may also have to adjust a few settings for best use:

    ## doesn't work currently: weechat.completion.default_template -> append "|%(weecord_completion)"
    weechat.bar.status.items -> replace buffer_name with buffer_short_name
    plugins.var.python.go.short_name -> on (if you use go.py)

If you want a more irc-style interface, you can enable irc-mode:

    /discord irc-mode

In irc-mode, weecord will not automatically "join" every Discord channel.  You must join a channel using the
`/discord join <guild-name> <channel-name>` command.

Watched channels:  
You can use `/discord watch <guild-name> [<channel-name>]` to start watching a channel or entire guild.
This means that if a message is received in a watched channel, that channel will be joined and added to the nicklist.

Autojoin channels:  
You can use `/discord autojoin <guild-name> [<channel-name>]` to start watching a channel or entire guild.
Any channel or guild marked as autojoin will be automatically joined when weecord connects.

---

## MacOS

Weechat does not search for mac dynamic libraries (.dylib) by default, this can be fixed by adding dylibs to the plugin search path,

```
/set weechat.plugin.extension ".so,.dll,.dylib"
```