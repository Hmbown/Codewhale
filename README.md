# Codewhale

Codewhale is an open source coding agent for your terminal, built in Rust and
improved in public with the people who use it.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="brand/wordmark-inverted.svg">
  <img src="brand/wordmark.svg" alt="Codewhale" width="360">
</picture>

[简体中文](README.zh-CN.md) · [日本語](README.ja-JP.md) · [Tiếng Việt](README.vi.md) · [Bahasa Indonesia](README.id.md) · [한국어](README.ko-KR.md) · [Español](README.es-419.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md) · [Українська](README.uk.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [繁體中文](README.zh-TW.md) · [हिन्दी](README.hi.md) · [Türkçe](README.tr.md) · [Italiano](README.it.md) · [Polski](README.pl.md) · [العربية](README.ar.md) · [Català](README.ca.md)

[![CI](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml/badge.svg)](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/codewhale-cli?label=crates.io)](https://crates.io/crates/codewhale-cli)
[![npm](https://img.shields.io/npm/v/codewhale?label=npm)](https://www.npmjs.com/package/codewhale)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.gg/37gfS3ksug)

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/screenshot.webp">
  <img src="assets/screenshot.webp" alt="A Codewhale terminal session" width="720">
</picture>

## Install

macOS / Linux — install the official GitHub release:

```bash
curl -fsSL https://codewhale.net/install.sh | sh
"$HOME/.local/bin/codewhale"
```

Windows: download the matching installer or archive from
[GitHub Releases](https://github.com/Hmbown/CodeWhale/releases/latest).
For an existing direct install, run `codewhale update` (or `codewhale update --check`
to inspect it). The updater prints the executable path and keeps newer builds.


The first run helps you connect a provider or stay offline. Codewhale also
supports npm and Cargo as secondary packaging routes, plus Docker, Nix, Scoop,
Android/Termux, and an optional CNB mirror. Existing package-managed installs
receive migration instructions. See [installation and PATH help](docs/INSTALL.md).

Tab completion is one command per shell — `codewhale completion bash|zsh|fish|powershell|elvish`.
See [shell completions](docs/INSTALL.md#8-shell-completions).

## Use

Talk to Codewhale the same way you would talk to a teammate:

```text
Fix the failing tests and explain what changed.
```

Or run a task without opening the TUI:

```bash
codewhale exec "fix the failing tests and explain what changed"
```

Codewhale can read your repository, edit files, run commands, inspect results,
and keep working toward a goal. You decide how much access it has.

## Why Codewhale

- **Use the model you want.** Connect hosted providers or local models through
  Ollama, vLLM, or SGLang. Switch provider and model with `/model`.
- **Stay in control.** Plan is read-only. Ask, Auto-Review, and Full Access make
  approval behavior visible. `/undo` reverts the last turn and `/restore`
  returns the workspace to an earlier snapshot.
- **Keep long work organized.** Save sessions, set a durable `/goal`, review
  workflows before they run, and coordinate agents without turning their
  internal instructions into your transcript.
- **Extend the agent you already have.** Connect MCP servers and skills,
  configure hooks, and keep agent roles as readable files in your project or
  personal settings.

Run `/help` in the TUI for commands and keyboard shortcuts.

## Safety

Codewhale runs on your machine with the access you grant it. Approval modes and
repository rules limit what the agent may do; optional OS sandboxing adds a
stronger execution boundary where supported. Unknown model prices stay unknown
instead of being reported as free.

Read [authorization order](docs/AUTHORIZATION_ORDER.md) for the exact policy
stack and [configuration](docs/CONFIGURATION.md) for local settings.

## Documentation

- [Providers and local models](docs/PROVIDERS.md)
- [Agent teams](docs/FLEET.md)
- [MCP](docs/MCP.md), [hooks](docs/HOOKS.md), and [configuration](docs/CONFIGURATION.md)
- [Local web client](docs/WEB.md)
- [All documentation](docs)

## Join the community

Codewhale gets better when people use it, report what feels wrong, and help fix
it. If a provider is missing, a workflow is awkward, or the terminal UI gets in
your way, [open an issue](https://github.com/Hmbown/CodeWhale/issues). If you
know how to improve it, [open a pull request](CONTRIBUTING.md). First
contributions are welcome, and contributors keep credit for the work that
lands.

Join the [Discord](https://discord.gg/37gfS3ksug), or add Hunter on WeChat
(`hunterbown`) and ask to join the Whale Brothers group.

## Project history

Codewhale began as `deepseek-tui` and still preserves that configuration and
session compatibility. It is now provider-neutral and independently maintained;
it is not affiliated with any model provider.

Thanks to every contributor and to the open source communities that helped the
project grow. See [the contributor record](docs/CONTRIBUTORS.md).

## License

[MIT](LICENSE). Portions adapted from other open-source projects are recorded
in [third-party notices](docs/THIRD_PARTY_NOTICES.md).


## 🌐 Web Resources & Interactive Index
- [CATEGORY BOARDGAMES](https://iskillplay.web.app/category-boardgames.html)
- [RESIDENT EVIL PURGE OPERATION](https://skillplay.github.io/resident-evil-purge-operation.html)
- [CATEGORY MERGE](https://quizverses.github.io/category-merge.html)
- [CITY BUILDER](https://theskillquest.pages.dev/city-builder.html)
- [INDEX10](https://studyplayings.pages.dev/index10.html)
- [FASHION WEEK 2025](https://studyquests.github.io/fashion-week-2025.html)
- [MERGE FOOD PUZZLE](https://iskillquest.pages.dev/merge-food-puzzle.html)
- [CATEGORY MINECRAFT 3](https://thelearnquesters.pages.dev/category-minecraft-3.html)
- [WORD ART COLOR BOOK PUZZLE](https://studyquests.github.io/word-art-color-book-puzzle.html)
- [SORT BALLS CONES](https://studyquests.pages.dev/sort-balls-cones.html)
- [CATEGORY BIKE 2](https://themindzone.pages.dev/category-bike-2.html)
- [PICTURE BY PIECES](https://themindskillplayplay.pages.dev/picture-by-pieces.html)
- [HOLE PUZZLE](https://iskillplay.web.app/hole-puzzle.html)
- [CATEGORY BUBBLE SHOOTER](https://themindskillplayplay.pages.dev/category-bubble-shooter.html)
- [DRAGON HUNTER](https://learnquester.github.io/dragon-hunter.html)
- [HAMSTER COMBO IDLE](https://quizverses.pages.dev/hamster-combo-idle.html)
- [WILD WEST MATCH 3](https://thequizzone.pages.dev/wild-west-match-3.html)
- [ROBBIE BECOME A BEAST](https://quizverses.pages.dev/robbie-become-a-beast.html)
- [SUIKA KAWAII CAT MERGE GAME](https://themindskillplayplay.pages.dev/suika-kawaii-cat-merge-game.html)
- [FASHIONISTA AVATAR STUDIO DRESS UP](https://iskillplay.web.app/fashionista-avatar-studio-dress-up.html)
- [CATEGORY FOOTBALL](https://themindplay.pages.dev/category-football.html)
- [POLYGON SPACE](https://thequizzone.pages.dev/polygon-space.html)
- [SAMURAI VS YAKUZA BEAT EM UP](https://themindskillplayplay.pages.dev/samurai-vs-yakuza-beat-em-up.html)
- [BLOCK COMBO BLAST](https://studyplaying.github.io/block-combo-blast.html)
- [HIPPO SUPERMARKET](https://studyquests.pages.dev/hippo-supermarket.html)
- [REAL RACING 3D](https://studyplaying.github.io/real-racing-3d.html)
- [MAGIC PIANO MUSIC](https://learnquester.github.io/magic-piano-music.html)
- [CATEGORY RELAXING223](https://quizverses.github.io/category-relaxing223.html)
- [ZENITH RUSH](https://studyquests.github.io/zenith-rush.html)
- [CATEGORY CASUAL 6](https://themindskillplayplay.pages.dev/category-casual-6.html)
- [CATEGORY CASUAL 17](https://themindskillplayplay.pages.dev/category-casual-17.html)
- [CRAZY BUS STATION](https://themindplay.github.io/crazy-bus-station.html)
- [HORSE CHAMPS](https://themindskillplayplay.pages.dev/horse-champs.html)
- [OFFICE GOLF](https://studyplayings.web.app/office-golf.html)
- [CATEGORY PUZZLE](https://learnquester.pages.dev/category-puzzle.html)
- [STUNT CAR EXTREME 2](https://studyplayings.web.app/stunt-car-extreme-2.html)
- [AIR BLOCK](https://thequizzone.pages.dev/air-block.html)
- [PIN MASTER](https://studyquests.pages.dev/pin-master.html)
- [CAPYBARA BLOCK BLAST](https://iskillplay.web.app/capybara-block-blast.html)
- [BLOCKAPOLYPSE ZOMBIE SHOOTER](https://themindskillplayplay.pages.dev/blockapolypse-zombie-shooter.html)
- [CATEGORY IO](https://learnquester.github.io/category-io.html)
- [CATEGORY RUNNING](https://themindskillplayplay.pages.dev/category-running.html)
- [HOOP WORLD 3D](https://learnquester.pages.dev/hoop-world-3d.html)
- [DRIFTCLICKER](https://themindskillplayplay.pages.dev/driftclicker.html)
- [IDLE ARCHEOLOGY](https://studyplayings.web.app/idle-archeology.html)
- [JIGSAW M](https://quizverses.pages.dev/jigsaw-m.html)
- [SLIME ATTACK PUZZLE](https://thequizzone.pages.dev/slime-attack-puzzle.html)
- [TRIANGLE WAY](https://thequizzone.pages.dev/triangle-way.html)
- [STUNT CAR EXTREME 2](https://studyquests.github.io/stunt-car-extreme-2.html)
- [LOVE IN STYLE](https://themindplay.github.io/love-in-style.html)
- [KINGDOM MATCH](https://skillplay.github.io/kingdom-match.html)
- [CATEGORY FPS174](https://thelearnquester.web.app/category-fps174.html)
- [INDEX4](https://studyplaying.github.io/index4.html)
- [PIXEL PATH](https://studyquests.github.io/pixel-path.html)
- [HEROBALL ADVENTURES 2](https://thequizzone.pages.dev/heroball-adventures-2.html)
- [CATEGORY RUNNING](https://iskillquest.pages.dev/category-running.html)
- [CATEGORY 1 PLAYER139](https://iskillplay.web.app/category-1-player139.html)
- [SOUL NOT FOUND](https://thelearnquester.web.app/soul-not-found.html)
- [REVERSI](https://thequizzone.pages.dev/reversi.html)
- [PUSH IT 3D](https://thequizzone.pages.dev/push-it-3d.html)
- [SUDOKU BRAIN BLOCKS](https://studyquests.github.io/sudoku-brain-blocks.html)
- [SKIBIDI SURVIVOR RUSH](https://iskillplay.web.app/skibidi-survivor-rush.html)
- [PET SIMULATOR](https://themindskillplayplay.pages.dev/pet-simulator.html)
- [CATEGORY POINT AND CLICK124](https://thelearnquester.web.app/category-point-and-click124.html)
- [RUSSIAN DERBY CRASH](https://thequizzone.pages.dev/russian-derby-crash.html)
- [STACK BATTLEIO](https://skillplay.github.io/stack-battleio.html)
- [GUESS THE ITALIAN BRAINROT ANIMALS](https://learnquester.pages.dev/guess-the-italian-brainrot-animals.html)
- [BACKROOMS AMONG IMPOSTOR ROLLING GIANT](https://themindskillplayplay.pages.dev/backrooms-among-impostor-rolling-giant.html)
- [COCKTAILZ](https://themindskillplayplay.pages.dev/cocktailz.html)
- [CATEGORY HAPARA](https://thelearnquester.web.app/category-hapara.html)
- [PULL THE THREAD PUZZLE](https://themindskillplayplay.pages.dev/pull-the-thread-puzzle.html)
- [MY ARCADE CENTER](https://learnquesters.pages.dev/my-arcade-center.html)
- [CONQUERIO](https://themindskillplayplay.pages.dev/conquerio.html)
- [ONLINE PORTAL](https://studyplaying.github.io/)
- [CATEGORY ESCAPE 2](https://thelearnquesters.pages.dev/category-escape-2.html)
- [HAMMER MASTERCRAFT DESTROY](https://iskillquest.pages.dev/hammer-mastercraft-destroy.html)
- [DEMOLITION CAR ROPE AND HOOK](https://thequizzone.pages.dev/demolition-car-rope-and-hook.html)
- [LAST UFO DEFENSE](https://quizverses.github.io/last-ufo-defense.html)
- [BACKWOODS](https://quizverses-9d2f2.web.app/backwoods.html)
- [BLOCK CUT CLEANER](https://iskillplay.web.app/block-cut-cleaner.html)
- [BABY PIANO CHILDREN SONG](https://iskillquest.pages.dev/baby-piano-children-song.html)
- [ITALIAN ANIMAL ALCHEMY BRAINROT](https://theskillquest.pages.dev/italian-animal-alchemy-brainrot.html)
- [FASHION MAKEOVER DASH](https://thelearnquesters.pages.dev/fashion-makeover-dash.html)
- [CATEGORY TURN BASED30](https://iskillplay.web.app/category-turn-based30.html)
- [CATEGORY CONTROLLER](https://learnquesters.pages.dev/category-controller.html)
- [INDEX15](https://quizverses.pages.dev/index15.html)
- [CATEGORY MAHJONG CONNECT 2](https://iskillquest.pages.dev/category-mahjong-connect-2.html)
- [HOOK MASTER MAFIA CITY](https://skillplay.github.io/hook-master-mafia-city.html)
- [CATEGORY DRESS UP 2](https://learnquesters.pages.dev/category-dress-up-2.html)
- [CATEGORY LINKS](https://thelearnquester.web.app/category-links.html)
- [GEOMETRY PLATFORMER](https://themindskillplayplay.pages.dev/geometry-platformer.html)
- [HOLE PUZZLE](https://quizverses.pages.dev/hole-puzzle.html)
- [PRACTICE ON ME](https://iskillplay.web.app/practice-on-me.html)
- [CATEGORY STICKMAN](https://quizverses-9d2f2.web.app/category-stickman.html)
- [VEX HYPER DASH](https://studyquests.github.io/vex-hyper-dash.html)
- [CAR OUT JAM](https://iskillquest.pages.dev/car-out-jam.html)
- [CATEGORY HORROR](https://learnquesters.pages.dev/category-horror.html)
- [INDEX11](https://studyplaying.github.io/index11.html)
- [MAGIC BRICK WARS](https://iskillplay.web.app/magic-brick-wars.html)
- [FASHION PRINCESS DRESS UP FOR GIRLS](https://iskillplay.web.app/fashion-princess-dress-up-for-girls.html)
- [CANDY MONSTER RAFFI](https://skillplay.github.io/candy-monster-raffi.html)
- [MY DOGY VIRTUAL PET](https://iskillplay.web.app/my-dogy-virtual-pet.html)
- [CATEGORY MERGE GAME](https://themindskillplayplay.pages.dev/category-merge-game.html)
- [STICKMAN ARMY TEAM BATTLE](https://studyplayings.web.app/stickman-army-team-battle.html)
- [MATHEMATICS RACING](https://themindplay.github.io/mathematics-racing.html)
- [MOJO EMOJI](https://studyquests.pages.dev/mojo-emoji.html)
- [BUSY BEE HIVE](https://quizverses-9d2f2.web.app/busy-bee-hive.html)
- [NINJA WARS BATTLE SIMULATOR](https://skillplay.github.io/ninja-wars-battle-simulator.html)
- [COCKTAILZ](https://studyquests.github.io/cocktailz.html)
- [SITEMAP](https://brainquests-fb2c5.web.app/sitemap.html)
- [CATEGORY HORDE SURVIVAL67](https://iskillplay.web.app/category-horde-survival67.html)
- [CATEGORY MANAGEMENT GAME](https://iskillplay.web.app/category-management-game.html)
- [MONSTER DUELIST](https://iskillquest.pages.dev/monster-duelist.html)
- [CATEGORY CAR 3](https://iskillplay.web.app/category-car-3.html)
- [CATEGORY MAKEUP51](https://learnquesters.pages.dev/category-makeup51.html)
- [CATEGORY CASUAL 7](https://thelearnquester.web.app/category-casual-7.html)
- [HIDDEN OBJECTS ISLAND](https://studyquests.pages.dev/hidden-objects-island.html)
- [CATEGORY IDLE448](https://thelearnquester.web.app/category-idle448.html)
- [CONSTRUCTION SIMULATOR](https://studyquests.github.io/construction-simulator.html)
- [EARWAX CLINIC](https://quizverses.github.io/earwax-clinic.html)
- [GO TO ZERO](https://themindskillplayplay.pages.dev/go-to-zero.html)
- [CATEGORY DRAWING](https://thelearnquester.web.app/category-drawing.html)
- [CATEGORY STUNT128](https://themindplay.pages.dev/category-stunt128.html)
- [MUSIC TILES FLUFFY HOP BEAT](https://themindskillplayplay.pages.dev/music-tiles-fluffy-hop-beat.html)
- [FASHION WEEK 2025](https://thequizzone.pages.dev/fashion-week-2025.html)
- [CATEGORY POOL](https://iskillplay.web.app/category-pool.html)
- [INDEX26](https://quizverses.github.io/index26.html)
- [JIGSAW M](https://thequizzone.pages.dev/jigsaw-m.html)
- [MERMAIDCORE AESTHETICS](https://theskillquest.pages.dev/mermaidcore-aesthetics.html)
- [PIN PUZZLE LOVE STORY](https://themindzone.pages.dev/pin-puzzle-love-story.html)
