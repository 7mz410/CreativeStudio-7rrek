# Releases (Creative Studio)

Tag push (`v*`) on a craft app repo builds the unsigned packages through the reusable workflow
`.github/workflows/build-release.yml` in this repo, then creates or updates the GitHub Release.
A tag with a `-` is a pre-release (`v0.0.2-test`). Caller: `.github/workflows/studio-release.yml`
(rsm); upstream's `release.yml` runs only on the `release` branch and never on tags.

## Assets

`<app>-<version>-<os>-<arch>[-portable].<ext>` plus `SHA256SUMS.txt` (the launcher, mansa, parses these):

| Platform | Asset | Built by |
|---|---|---|
| macOS (aarch64) | `<app>-<version>-macos-aarch64.dmg` | `macos_script` |
| Windows x64 | `<app>-<version>-windows-x64.msi`, `<app>-<version>-windows-x64-portable.zip` | `windows_script` (MSI and zip), or the generic zip when `windows_script` is empty |
| Linux x86_64 | `<app>-<version>-linux-x86_64.tar.gz` | `linux_script` |

Every packaging script writes `<package>-<version>-...`; the workflow renames it to `<app>-...`
and drops the `*-cli-*` archives. Unsigned: Gatekeeper and SmartScreen warn (see the release notes).

## Inputs

| Input | Meaning |
|---|---|
| `app` | Asset prefix and launcher stem (`photocraft`). Release title too. |
| `tag` | Release tag. Version = tag without `v`. |
| `package` | Name the packaging scripts give their files (`<package>-<version>-...`), i.e. the cargo package of the GUI app. |
| `version_var` | Env var the app's `packaging/env.sh` reads the version from (`<NAME>_VERSION`). |
| `bundle` | macOS app folder inside the `.dmg`, checked after packaging. |
| `binary` | Executable checked in the tarball, and zipped on Windows when `windows_script` is empty. Default: `package`. |
| `macos_script` / `macos_args` | Default `packaging/macos/package.sh`, `--arch aarch64`. |
| `linux_script` / `linux_args` | Default `packaging/linux/package.sh`, `--formats tar`. |
| `windows_script` / `windows_args` | Default empty (generic zip). Apps: `packaging/windows/package.ps1`, `-Arch x64`. |

## Per-app inputs

Caller snippet (every app repo, with `permissions: { contents: write }`):

```yaml
uses: 7mz410/CreativeStudio-7rrek/.github/workflows/build-release.yml@main
with:
  app: <app>
  tag: ${{ github.ref_name }}
  package: <package>
  version_var: <VERSION_VAR>
  bundle: <Bundle>.app
  windows_script: packaging/windows/package.ps1
```

| Repo | app | package | version_var | bundle | windows_script | Status |
|---|---|---|---|---|---|---|
| rsm (this repo) | rsm | vectorcraft | VECTORCRAFT_VERSION | rsm.app | (generic zip) | tested |
| 7rrek | 7rrek | vectorcraft | VECTORCRAFT_VERSION | 7rrek.app | packaging/windows/package.ps1 | not run |
| tlween | tlween | photocraft | PHOTOCRAFT_VERSION | tlween.app | packaging/windows/package.ps1 | not run |
| lamasat | lamasat | lightcraft | LIGHTCRAFT_VERSION | lamasat.app | packaging/windows/package.ps1 | not run |
| tsweer | tsweer | filmcraft | FILMCRAFT_VERSION | tsweer.app | packaging/windows/package.ps1 | not run |
| effectat | effectat | effectcraft | EFFECTCRAFT_VERSION | effectat.app | packaging/windows/package.ps1 | not run |
| trteeb | trteeb | designcraft | DESIGNCRAFT_VERSION | trteeb.app | packaging/windows/package.ps1 | not run |
| sf7aat | sf7aat | pdfcraft | PDFCRAFT_VERSION | sf7aat.app | packaging/windows/package.ps1 | not run (zip carries models\) |
| klmat | klmat | wordcraft | WORDCRAFT_VERSION | klmat.app | packaging/windows/package.ps1 | not run |
| arqam | arqam | gridcraft | GRIDCRAFT_VERSION | arqam.app | packaging/windows/package.ps1 | not run |
| 3urood | 3urood | deckcraft | DECKCRAFT_VERSION | 3urood.app | packaging/windows/package.ps1 | not run |
| muhandes | cadcraft | cadcraft | CADCRAFT_VERSION | CADCraft.app | packaging/windows/package.ps1 | not run |
| sharek | figcraft | figcraft | FIGCRAFT_VERSION | FigCraft.app | packaging/windows/package.ps1 | not run (its release.yml also sets the version with xtask) |
| aswat | soundcraft | soundcraft | SOUNDCRAFT_VERSION | SoundCraft.app | packaging/windows/package.ps1 | not run |

Each row assumes the app's scripts keep their current names and arguments (`--arch aarch64` on
macOS, `--formats tar` on Linux, `-Arch x64` on Windows). Check them on the first test tag of each
repo before a real release. Apps that do not yet pass `version_var` through their env.sh need the
same change first.
