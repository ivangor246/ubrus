# Ubrus

[English](../../README.md) · [Русский](README.ru.md) · **Čeština** · [Deutsch](README.de.md) · [Bahasa Indonesia](README.id.md) · [Italiano](README.it.md) · [Nederlands](README.nl.md) · [Português (Portugal)](README.pt-PT.md) · [Türkçe](README.tr.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [简体中文](README.zh-Hans.md) · [繁體中文](README.zh-Hant.md)

[![CI](https://github.com/ivangor246/ubrus/actions/workflows/ci.yml/badge.svg)](https://github.com/ivangor246/ubrus/actions/workflows/ci.yml)
[![Nejnovější vydání](https://img.shields.io/github/v/release/ivangor246/ubrus)](https://github.com/ivangor246/ubrus/releases/latest)
[![Ubuntu 24.04 / 26.04](https://img.shields.io/badge/Ubuntu-24.04%20%7C%2026.04-E95420)](#instalace)
[![GNOME Shell 46 / 50](https://img.shields.io/badge/GNOME%20Shell-46%20%7C%2050-4A86CF)](#instalace)
[![Licence](https://img.shields.io/badge/license-MIT%20or%20Apache--2.0-blue)](#projekt)

Nastavte si v GNOME pro každý monitor vlastní fotku, animaci (GIF nebo WebP) nebo video ve smyčce. Na monitoru, kde okna zcela zakryjí pozadí, se přehrávání pozastaví.

**[Stáhnout pro Ubuntu (.deb, amd64)](https://github.com/ivangor246/ubrus/releases/latest/download/ubrus-amd64.deb)** · [Všechny soubory vydání](https://github.com/ivangor246/ubrus/releases/latest)

![Knihovna tapet a nastavení monitoru](../assets/editor-cs.png)

## Instalace

Otevřete stažený balíček DEB v Centru aplikací nebo v jeho složce spusťte `sudo apt install ./ubrus-amd64.deb`.

Jednu fotku můžete ihned nastavit pro všechny monitory. Chcete-li animaci nebo jinou tapetu na každém monitoru, odhlaste se z GNOME, znovu se přihlaste a povolte přibalené rozšíření.

```sh
gnome-extensions enable wallpaper@ubrus.org
```

Používáte Snap nebo samostatný archiv? Nejprve v Ubrus otevřete **Nastavení** a zkopírujte příkaz k instalaci rozšíření.

## Nastavení tapety

V Ubrus zvolte **Importovat soubory…** a přidejte obrázky nebo videa. Vyberte miniaturu a monitor, případně upravte přiblížení a klikněte na **Použít**. Po zavření okna tapeta zůstane na ploše.

## Projekt

[Sestavení ze zdrojového kódu](../development.md) · [Nahlásit chybu](https://github.com/ivangor246/ubrus/issues) · [MIT](../../LICENSE-MIT) nebo [Apache 2.0](../../LICENSE-APACHE)

**Štítky** · [wallpaper-manager](https://github.com/topics/wallpaper-manager) · [animated-wallpaper](https://github.com/topics/animated-wallpaper) · [gnome-shell-extension](https://github.com/topics/gnome-shell-extension) · [rust](https://github.com/topics/rust)
