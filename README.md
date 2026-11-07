# Inari: Innovation Artificial Intelligence

An unofficial implementation of [Innovation Ultimate](https://asmadigames.com/innovation)
by Carl Chudyk.

This repo has three parts:

- **`engine/`**: a rules engine for two-player games, supporting every
  expansion (Echoes, Figures, Cities, Unseen, Artifacts) in any combination
- **A neural-network AI opponent**: the engine's network and search, with the
  trained weights in `weights/`
- **`app/`**: Inari, a desktop app for hotseat play or play against the AI

## Download

Windows builds are on the [releases page](https://github.com/convexish/inari/releases).
Unzip and run `inari.exe`; the AI is built in.

## Building from source

This repo uses [pixi](https://pixi.sh) for dependency management and tasks.

```sh
pixi run play-app        # run the app from WSL
pixi run build-windows   # cross-build inari.exe (from Linux or WSL)
```

## License

The code is [MIT](LICENSE) licensed. Card texts (`data/`, the engine's
catalogs and `engine/reference/`) are © Asmadi Games and are not covered by
that license; this project is not affiliated with or endorsed by Asmadi Games.
The fonts in `app/assets/fonts/` are under the SIL Open Font License.
