# ORBYN

A blue AI wireframe orb that roams your terminal to prevent screen burn-in.

> AI disclaimer: this project was written with AI assistance; review the code
> before relying on it.

```
    SSC+*######%######++
     SC+*#..**#%...+*##+.
      +*#   *##%   .**#..
     =+#   .*##%::  +**#..
    +*#%%%%%%%%%%%%%%%%%%++
     =+*#...*##%::..+*##..
      +*#   *##%   +**#..
       +*#  **#%   +*##..
       +*######%######+
```

Zero dependencies, single static binary. Ultron/Jarvis-flavoured globe with
bounce physics, spiky organic energy bursts, fading cmatrix-style trails, and
roaming telemetry text, so no pixel stays lit long enough to burn in. Blue
truecolor with automatic 256/16/mono fallback. Under 1% of one CPU core and
~3 MB RAM at 30 FPS.

## Install

```sh
cargo install --path .
```

## Usage

```sh
orbyn          # just run it
orbyn --help   # every option
```

Keys: `q`/`Ctrl-C` quit, `space` pause, `h` toggle telemetry, `+`/`-` speed.
Use a dark terminal theme. The terminal is restored on quit, panic, and
`SIGTERM`/`SIGHUP`.

## License

[DO WHATEVER YOU WANT LICENSE](LICENSE).
