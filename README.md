# Nickracker

# NOTE: This is incomplete, experimental and does not yet work.

Nickracker is a simple windows utility to help you crack minotaur vaults in the [Project Gorgon](https://store.steampowered.com/app/342940/Project_Gorgon) MMORPG.
It periodically (Every half a second) takes screenshot of the game's window, using the same windows API that OBS uses, and then it looks at that image and tries to find an open window for a vault puzzle. When it finds one, it looks at the already attempted solutions and generates new guesses using the solver. The program has 2 windows: One is a normal window, and has some settings and information for users, and the other is a transparent, always-on-top, frameless and borderless window that is meant to sit on top of the game and act as a sort of overlay. When the program finds an open vault and generates a guess, it uses this overlay window to give the user a visual indication of which symbols it thinks will work best (The guess that the solver generated).

The solver has been run on all possible 20736 puzzles (In 269.78s, ~5 mins) and the worst case takes 12 guesses ([10, 11, 1, 2]/[11, 2, 4, 4]).

## Is this allowed?

I have asked in the official discord, but have received no official response yet. Use at your own risk.

## Attribution/Thanks

- Niph for [PgSurveyor](https://github.com/dlebansais/PgSurveyor-Disclosed), which is what inspired me to work on this
- McBreezy for pointing out that the minotaur puzzles are essentially a specific case of [Mastermind](<https://en.wikipedia.org/wiki/Mastermind_(board_game)>)
- [screenshot-rs](https://github.com/robmikh/screenshot-rs), an MIT-licensed rust tool which has nice code for the modern windows 10 graphics window capture API
- [wcap](https://github.com/mmozeiko/wcap/), C++ window capture tool which was similarly helpful
- [This](https://www.researchgate.net/publication/30485793_Yet_another_Mastermind_strategy) 2005 paper titled "Yet another Mastermind strategy" (Barteld Pieter Kooi) and [this](https://dspace.library.uu.nl/handle/1874/367005) (This one has a _great_ "Literature review" section) 2018 paper "Genetic Algorithms Playing Mastermind" (Oijen, V. van) which helped me understand the problem
- [This](https://gist.github.com/scvalex/910500/1a79b293c9334d76f7d0ef589f8ca40519caa0d0) mastermind solver in C
- [This](https://stackoverflow.com/a/31339634/8414238) stackoverflow answer for click-through win32 windows
- [This](https://stackoverflow.com/a/65876605/8414238) amazing stackoverflow answer about win32 window userdata

## Building and basic docs

[here](./docs.md)

## Licensing

The code in this repository is available under any of the following licenses, at your choice: MIT OR Apache-2.0 OR BSL-1.0 OR MPL-2.0 OR Zlib OR Unlicense
