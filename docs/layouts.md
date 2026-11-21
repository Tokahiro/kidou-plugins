# Write a Home Layout

A Home layout draws the first page of Kidō (releasing soon).
A layout is Slint code in the folder `layout` beside the manifest.
It needs no build step:

```
my-layout/
├── plugin.toml
└── layout/
    ├── layout.slint      the layout, which exports the component `Layout`
    └── …                 its own .slint files and images
```

The five layouts in [`layouts/`](../layouts) are the reference; start from the one closest to what you want.
The contract's own files are in [`crates/layout/contract/`](../crates/layout/contract); this page sums them up.

## What Kidō Does, and What the Layout Does

Kidō keeps the focus and the keys.
It tells the layout which game is focused and in which zone, and the layout draws that.
The manifest picks the navigation Kidō gives the layout, from Kidō's own set:

```toml
[layout]
actions = "above"     # above | none: the selected game's actions, above the row
tabs = "none"         # none | stores | groups: tabs over the row, stepped with LT and RT
frame = "switcher"    # switcher | lanes | rail: how the pages show, which Kidō draws
art = "cover"         # cover | square: the art each game of the row comes with
scroll = "stage"      # stage | row: Kidō slides the row, or the layout scrolls it
```

`Layout` gets these from Kidō:

| Property | Holds |
| --- | --- |
| `tiles`, `focused`, `zone` | The row's games, the focused one, and where the focus is |
| `stage`, `hero` | The focused game's cover, and its title, tagline and facts |
| `tabs`, `tab` | The tabs' names in the player's language, and the chosen one |
| `empty`, `empty-words` | Whether the library has no game yet, and the words for that card |
| `theme` | The look's colours, the text scale, the card size and the edges to keep content between |

It gives these back:

| Property or callback | Says |
| --- | --- |
| `actions-x`, `actions-y` | Where Kidō draws the selected game's actions |
| `focused-place` | Where the focused game stands, for the launch animation |
| `activated(int)`, `tab-tapped(int)` | A tap on a game or on a tab |

Ready parts come with the contract, among them `Cover`, `RowCover`, `HeroTitle`, `Kicker`, `Facts`, `EmptyCard`, `Shoulder`, `Glass` and `Ring`.
`Kicker` and `Shoulder` came with contract 3.1.0: a layout that uses them says `api = "3.1.0"`.

## Contract Versions

Until Kidō's first release, the contract may change in any way, also within 3.x.
Such a change takes the next minor version, and a layout it breaks needs a fix to run again.
From the first release on, a change that breaks a layout built for the contract needs a new major version, such as 4.0.0.
A new part or property a layout may use is a new minor version.

## Reduced Motion

A player can turn motion down in Kidō's settings, and a layout must keep to that.
While `theme.reduced-motion` is true, a layout starts no animation: what would move changes at once.

Give the value its own animated property, and read that property only in full motion:

```slint
property <length> lifted: self.focused ? self.theme.focus-lift-cover : 0;
property <length> lift: self.theme.reduced-motion ? 0 : self.lifted;
animate lifted { duration: 130ms; easing: ease-out; }
y: self.rest-y + self.lift;
```

A test on `reduced-motion` inside the animated value isn't enough.
Slint still starts the animation when the focus moves, from 0 to 0, with nothing to see on screen.
Kidō's check can't find this, so look for it yourself in each `animate`.
The contract's parts, such as `RowCover` and `Ring`, keep the rule on their own.

## What Kidō Refuses

Kidō checks a layout before it installs it, and again after each update of Kidō.
A layout that breaks a rule isn't installed, and the refusal names each rule.
The main ones:

- No `FocusScope`, `TextInput`, `PopupWindow`, `Dialog`, `Timer`, menu or gesture-handler element, and no `changed` handler.
- It calls only `activated` and `tab-tapped`, and only in a `TouchArea`'s `clicked`.
- Each animation ends: at most 3 seconds and 10 iterations, both written as numbers. No call that isn't pure, such as `animation-tick()`.
- No `for` over a number and no `for` inside a `for`.
- Images are PNG, JPEG or SVG files of the layout, at most 4096 px a side, given to properties and never inside a `for`. No fonts: a layout uses Kidō's.
- At most 256 files, 8 MiB each and 32 MiB together, with names of `A-Z a-z 0-9 . _ -`.

## Pack and Try It

`kidouplug.exe` comes with Kidō.

```
kidouplug install my-layout                     install the folder as it is
kidouplug install my-layout --replace           install over the last try
kidouplug pack my-layout my-layout-1.0.0.kidouplug
```

`pack` writes the package and shows its SHA-256.
The same folder always gives the same file.

