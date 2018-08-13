## A note on using the Discord API (TOS)

There seems to be conflicting information on custom clients, there is very little about custom clients explicitly, however, there is some information on "selfbots". Im not sure whether not weechat-discord is a selfbot (but it probably is).

---

## Important

***There is potentially a risk from using weechat-discord, use it at your own risk, I am not responsible if your are banned***

**This program is _very_ likely against TOS, but there is no (?) colloquial evidence of Discord banning users for using custom clients and not spamming***

Now, the evidence I have found:

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