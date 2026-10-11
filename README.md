<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/studio/rsm.svg">
    <img alt="rsm" src="docs/studio/rsm.svg" width="140">
  </picture>
</p>

<h1 align="center">rsm</h1>

<p align="center">
  <b>Free, open-source vector illustration — part of the Creative Studio creative suite.</b><br>
  <b>رسم متجهي مجاني ومفتوح المصدر — جزء من حزمة Creative Studio.</b>
</p>

<p align="center">
  <a href="https://rsm.studio.creative.ps/"><b>Try it in your browser · جرّبه في المتصفح</b></a> ·
  <a href="https://studio.creative.ps/">studio.creative.ps</a>
</p>

<p align="center">
  <img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-555555">
  <img alt="Based on VectorCraft" src="https://img.shields.io/badge/based%20on-VectorCraft-e8573f">
</p>

## Built on VectorCraft

rsm is a fork of **[VectorCraft](https://github.com/storytold/vectorcraft)**, the open-source,
clean-room vector illustration app written in Rust by the **[ArtCraft](https://getartcraft.com/)** team
and the VectorCraft contributors. Almost everything that makes this app work is their work. Thank you.

If you like rsm, please star and support the upstream project as well:
[github.com/storytold/vectorcraft](https://github.com/storytold/vectorcraft) ·
[getartcraft.com/apps](https://getartcraft.com/apps).

Creative Studio adds its own branding and, next, a full Arabic and English interface with right-to-left
layout. Our changes are listed in [`studio/CHANGES.md`](studio/CHANGES.md); we aim to send
improvements that help everyone, such as the Arabic translation, back upstream.

## The Creative Studio suite

| App | For | Based on | Status |
|---|---|---|---|
| **rsm** | Vector illustration | [VectorCraft](https://github.com/storytold/vectorcraft) | Branded fork |
| Creative Studio tlween | Image editing | [PhotoCraft](https://github.com/storytold/photocraft) | Planned |
| Creative Studio trteeb | Page layout | [DesignCraft](https://github.com/storytold/designcraft) | Planned (upstream in development) |
| Creative Studio tsweer | Video editing | [FilmCraft](https://github.com/storytold/filmcraft) | Planned |
| Creative Studio effectat | Motion graphics | [EffectCraft](https://github.com/storytold/effectcraft) | Planned |
| Creative Studio 7rrek | Character animation (tweens + frame by frame) | rsm | Planned |
| Creative Studio sharek | Collaborative design | to be decided | Planned |

## Quick start

Requires a recent stable Rust toolchain.

```sh
cargo run --release -p vectorcraft      # desktop app
cargo xtask bundle                      # macOS app bundle in dist/
cd apps/vectorcraft-web && trunk build --release   # web build
```

Features, file formats, the command API and the MCP server for agents are the same as upstream; see
the [upstream README](https://github.com/storytold/vectorcraft#readme), [`ROADMAP.md`](ROADMAP.md) and
[`docs/`](docs).

## Keeping up with upstream

```sh
git fetch upstream
git merge upstream/main
python3 studio/rebrand.py
```

## License and credits

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option, as upstream.
Copyright (c) 2026 ArtCraft Team and the VectorCraft contributors; modifications by the Creative Studio
contributors. Required notices are in [NOTICE](NOTICE), and every bundled asset is listed with its
licence in [ASSETS.md](ASSETS.md).

The Creative Studio name and logos in [`docs/studio/`](docs/studio/) belong to the Creative Studio project.
ArtCraft and VectorCraft are names of the upstream project, used here only to credit it; Creative Studio is
not affiliated with or endorsed by the ArtCraft team.

<sub>Adobe and Illustrator are trademarks or registered trademarks of Adobe Inc. in the United States
and/or other countries. Creative Studio is an independent open-source project and is not affiliated with,
sponsored by or endorsed by Adobe Inc.</sub>
