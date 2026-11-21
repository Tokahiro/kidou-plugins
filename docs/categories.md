# Plugin Categories

A plugin is a small folder with a manifest, `plugin.toml`, and what its category needs beside it.
Kidō (releasing soon) installs it from a package, a `.kidouplug` file.

Each plugin is in exactly one category, and each category has its own interface and rules.
A plugin that wants two things ships as two plugins.
A plugin with code runs in a sandbox and reaches only what its manifest asks for.

| Category | Adds | Built with | May ask for | Runs |
| --- | --- | --- | --- | --- |
| `store` | A store's or launcher's games, their launches and links; settings rows and quick-menu tiles | WebAssembly, `kidou:store` 2.x | Registry keys, folders, link schemes, art hosts | At start; dropped where its store isn't found |
| `source` | Art and facts for any game; settings rows and quick-menu tiles | WebAssembly, `kidou:source` 2.x | Art hosts, `https` hosts it asks itself | At its first call |
| `device` | Support for a device and its maker's app: settings rows and quick-menu tiles | WebAssembly, `kidou:device` 1.x | Registry keys, folders, link schemes | Only on a device it serves |
| `events` | Acts when a game starts or ends; settings rows and quick-menu tiles | WebAssembly, `kidou:events` 1.x | Registry keys, folders, link schemes | At the events it names |
| `layout` | A Home layout | Slint, the layout contract 3.x | Nothing | Only while chosen |
| `look` | A look: colours and lights | A look file, schema 1.x | Nothing | Only while chosen |

Rules every category keeps:

- No plugin draws its own window but a `layout`. A plugin gives data, and Kidō draws it with its own focus and gamepad rules.
- A `device` plugin reads its maker's software and opens it. It writes nothing.
- Only a `source` plugin asks hosts over the network, and only the `https` hosts it names.
- A grant can carry a reason for the player, in English and German, each at most 80 characters. A plugin on Kidō's list needs a clear one.

The interfaces of the WebAssembly categories are in [`wit/`](../wit).
A `store` or `source` plugin with tiles also exports `kidou:tiles` 1.x ([`wit/tiles`](../wit/tiles)), which gets the presses.
[`devices/onexconsole`](../devices/onexconsole) is a device plugin built on them: it shows OneXConsole's presets on a OneXPlayer and opens OneXConsole.
It isn't on Kidō's list yet.
[`sources/steamgriddb`](../sources/steamgriddb) is a source plugin: it looks up art at SteamGridDB with the player's own key.
[`sources/igdb`](../sources/igdb) gives a game's time to beat from IGDB with the player's own Twitch app.
It isn't on Kidō's list yet either.

## The Manifest

Every `plugin.toml` starts the same way:

```toml
id = "moonlight"         # the plugin's folder and package carry this name
name = "Mondlicht"
badge = "MOON"
version = "1.1.0"
author = "Kidō"
category = "layout"      # exactly one
api = "3.0.0"            # the version of the category's interface it's built for
```

A plugin built for `api = "3.0.0"` runs on every Kidō whose interface for that category is 3.0.0 or a later 3.x.
Kidō refuses a plugin built for another first number or for a later version than its own, a manifest with a key its category doesn't have, and a grant its category may not ask for.
The refusal names the rule.

