Weechat Discord
===============

A plugin for Weechat that provides access to Discord.

Ditch the slow Electron client and use your terminal!

(Still in Beta)

## A note on using the Discord API (TOS)

There seems to be conflicting information on custom clients, there is very little about custom clients explicitly, however, there is some information on "selfbots". Im not sure whether not weechat-discord is a selfbot (but it probably is).

---

## Important

***There is potentially a risk from using weechat-discord, use it at your own risk, I am not responsible if your are banned***

**This program is _very_ likely against TOS, but there is no (?) colloquial evidence of Discord banning users for using custom clients and not spamming***

If you understand the risk: <a href="#building">Building</a>

Now, the evidence:

#### For

["jhgg"](https://github.com/jhgg) on [Github (2016) said](https://github.com/discordapp/discord-api-docs/issues/69)

```
"Self bots aren't supported."
"I repeat. Do not call /api/login or /api/auth/mfa/totp. 
Instead, log in on the discord client, pop open the web inspector, (ctrl/cmd shift I), and type localStorage.token in the console to get your auth token."
```

and,
```
"We're not interested in banning self bots. Some users use them to do cool things."
```

#### Against

Recently, [allthefoxes](https://www.reddit.com/user/allthefoxes) said on [reddit](https://www.reddit.com/r/discordapp/comments/9435e8/discord_wont_let_you_have_your_own_token/e3ifppw) (2018)

```
"This is definitely against our API Terms of Service.
Using any kind of selfbot could lead to account termination."
```

However...

#### Indeterminate

[allthefoxes](https://www.reddit.com/user/allthefoxes) _also_ said on [reddit](https://www.reddit.com/r/discordapp/comments/9435e8/discord_wont_let_you_have_your_own_token/e3ifppw) (2018)

```
No, we remove it to protect our users from common scams and account takeover situations.
...
```

In response to [D0cR3d](https://www.reddit.com/user/D0cR3d) (2018)
```
That's exactly why they probably removed it, for people like you. Selfbots ARE against the terms of service.
```

---


With that in mind:

### Building

Dependencies:

* Weechat developer libraries. Usually called `weechat-dev`, or sometimes just `weechat` includes them.

The makefile should give enough information for build commands. Here's the essentials:

    cd weechat-discord # or wherever you cloned it
    cargo build --release

This will produce a shared object called `target/release/libweecord.so` (or `.dylib` on macos). Place it in your weechat plugins directory, which is probably located at `~/.weechat/plugins` (may need to be created)

The Makefile has a tiny bit of automation that helps with development:

    make # (same as make all) just runs that `cargo build --release` command, produces weecord.so
    make install # builds and copies the .so to ~/.weechat/plugins, creating the dir if required
    make run # installs and runs `weechat -a` (-a means "don't autoconnect to servers")

Maybe important note: The previous version of this project, written in Go, used to get **really upset** when the .so was modified during the same weechat session, even if unloaded. When developing, make sure to completely quit weechat when updating the .so, just to be sure (otherwise you might get a SIGSEGV and hard crash).


### Using

Due to some idiocracy on Discord's part, [you will need to obtain a login token](https://github.com/hammerandchisel/discord-api-docs/issues/69#issuecomment-223886862). 
You can either use a python script to find the tokens, or try and grab them manually.

#### Python Script

`find_token.py` is a simple python3 script to search the computer for localstorage databases. It will present a list of all found databases.

If ripgrep is installed it will use that, if not, it will use `find`.


#### Manual

In the devtools menu of the website and desktop app (ctrl+shift+i or ctrl+opt+i) Application tab > Local Storage on left, discordapp.com, token entry.

When this was written, discord deletes its token from the visible table, so you may need to refresh the page (ctrl/cmd+r) and grab the token as it is refreshing.

Set that token:

    /discord token 123456789ABCDEF

Then, connect:

    /discord connect

Note you may also have to adjust a few settings for best use:

    ## doesn't work currently: weechat.completion.default_template -> append "|%(weecord_completion)"
    weechat.bar.status.items -> replace buffer_name with buffer_short_name
    plugins.var.python.go.short_name -> on (if you use go.py)

---

## MacOS

Weechat does not search for mac dynamic libraries (.dylib) by default, this can be fixed by adding dylibs to the plugin search path,

```
/set weechat.plugin.extension ".so,.dll,.dylib"
```