# Spellcode

Un terminal à onglets en noir et blanc, rendu par le GPU, pour macOS et Windows,
écrit en Rust avec [GPUI](https://gpui.rs).

[![Build status](https://img.shields.io/github/actions/workflow/status/kawazoeh/Spellcode/ci.yml?branch=main&label=build)](https://github.com/kawazoeh/Spellcode/actions)
[![License: MIT](https://img.shields.io/github/license/kawazoeh/Spellcode)](https://github.com/kawazoeh/Spellcode/blob/main/LICENSE)
[![Latest release](https://img.shields.io/github/v/release/kawazoeh/Spellcode)](https://github.com/kawazoeh/Spellcode/releases)

[English](README.md) · **Français**

## Pourquoi il existe

Un terminal passe l'essentiel de son temps à ne rien faire. Spellcode ne repeint
un panneau que lorsque son PTY a produit de la sortie ou que le curseur a
clignoté : un onglet ouvert mais silencieux ne coûte donc rien, et un repaint
complet coûte à peu près autant qu'un curseur qui clignote. La barre d'onglets
tient lieu de barre de titre, si bien qu'aucune rangée de chrome supplémentaire
ne mange de hauteur. Chaque onglet est un vrai PTY derrière un émulateur VT 256
couleurs complet, pour que les programmes plein écran se comportent normalement,
tandis que le chrome de l'application reste strictement en niveaux de gris. Il
n'y a pas de liste de lancement intégrée : un seul `config.toml` déclare les
entrées qui apparaissent dans le menu de nouvel onglet.

Il s'adresse aux développeurs qui veulent un terminal petit, rapide et pensé
pour le clavier, configurable avec un seul fichier TOML et compilable depuis un
unique workspace Rust.

## Installation

### Versions précompilées

Chaque version publie six artefacts :

| Plateforme           | Artefact                             | Type                             |
| -------------------- | ------------------------------------ | -------------------------------- |
| macOS, Apple silicon | `Spellcode-macos-arm64.dmg`          | Image disque par glisser-déposer |
| macOS, Apple silicon | `Spellcode-macos-arm64.zip`          | Bundle d'application zippé       |
| macOS, Intel         | `Spellcode-macos-x86_64.dmg`         | Image disque par glisser-déposer |
| macOS, Intel         | `Spellcode-macos-x86_64.zip`         | Bundle d'application zippé       |
| Windows, x64         | `Setup-Spellcode-<version>-x64.exe`  | Installateur (NSIS)              |
| Windows, x64         | `Spellcode-windows-x86_64.zip`       | Version portable                 |

Sur macOS, préférez le `.dmg` : ouvrez-le et glissez `Spellcode.app` sur le
raccourci `Applications`. Le `.zip` est le même bundle sans l'image disque,
conservé comme secours plus léger. Sur Windows, l'installateur
`Setup-…-x64.exe` ajoute une entrée au menu Démarrer, un désinstalleur et une
entrée dans *Programmes et fonctionnalités* ; le `.zip` est une version portable
que vous lancez depuis le dossier où vous l'avez extrait.

**Premier lancement.** Le bundle macOS est signé **en ad-hoc seulement** — sans
Developer ID ni notarisation — et les binaires Windows ne sont pas signés, donc
les deux systèmes préviennent au premier lancement :

- **macOS :** faites un clic droit sur l'application, choisissez **Ouvrir**,
  puis confirmez. Un simple double-clic affiche encore la boîte « développeur
  non identifié » ; la supprimer exigerait un Apple Developer ID et une
  notarisation, dont le projet ne dispose pas. [docs/macos.md](docs/macos.md)
  détaille ce qui est signé et ce qui ne l'est pas.
- **Windows :** quand SmartScreen affiche « Windows a protégé votre PC »,
  choisissez **Informations complémentaires**, puis **Exécuter quand même**.
  Voir [docs/windows.md](docs/windows.md).

### Compiler depuis les sources

Nécessite Rust (voir [Exigences de compilation](#exigences-de-compilation)).

```sh
git clone https://github.com/kawazoeh/Spellcode spellcode
cd spellcode
cargo build --release
```

Sur macOS, le binaire est `target/release/spellcode-app` ; pour obtenir un vrai
bundle d'application puis une image disque :

```sh
scripts/bundle.sh   # target/Spellcode.app, scellé et signé en ad-hoc
scripts/dmg.sh      # Spellcode.dmg autour de ce bundle
```

`scripts/bundle.sh` accepte un chemin d'icône en option et utilise par défaut
`assets/spellcode01.png` :

<img src="assets/spellcode01.png" alt="L'icône de l'application Spellcode : une étoile à quatre branches encadrée de marques en forme de crochets au-dessus, en dessous et de chaque côté, dessinée en traits pâles sur un carré blanc." width="96">

Sur Windows, retirez la fonctionnalité de rendu propre à macOS et compilez le
workspace :

```sh
cargo build --release --no-default-features
```

GPUI compile ses shaders HLSL avec `fxc.exe` au moment du build, donc le SDK
Windows doit être installé et détectable ; [docs/windows.md](docs/windows.md)
liste les exigences exactes. Le binaire est `target/release/spellcode-app.exe`.

Lancez la suite de tests avec :

```sh
cargo test --workspace
```

## Exigences de compilation

- **Rust 1.85 ou plus récent.** Le workspace utilise l'édition 2024.
- **macOS 11.0 ou plus récent.** La fenêtre dessine les boutons natifs
  au-dessus d'une surface translucide et floutée, et le bundle d'application
  déclare `LSMinimumSystemVersion` 11.0.
- **Aucune installation complète de Xcode n'est requise** pour le build macOS.
  Le moteur de rendu macOS par défaut de GPUI compile ses shaders avec la chaîne
  d'outils `metal` de Xcode, absente des Command Line Tools ; la fonctionnalité
  `macos-blade` activée par défaut utilise à la place le backend Blade, qui
  valide ses shaders WGSL à travers Rust. `scripts/bundle.sh` utilise `sips` et
  `iconutil`, tous deux fournis avec macOS, pour construire le `.icns`.
- **Windows** nécessite la chaîne d'outils MSVC et un SDK Windows fournissant
  `fxc.exe`. Compilez avec `--no-default-features`, car `macos-blade` est un
  moteur de rendu propre à macOS. Voir [docs/windows.md](docs/windows.md) pour
  les détails.
- **Linux** est compilé et testé en intégration continue avec
  `--no-default-features` (backends X11 et Wayland de GPUI), mais aucun binaire
  Linux n'est publié.

## Raccourcis clavier

| Raccourci               | Action                                            |
| ----------------------- | ------------------------------------------------- |
| `+` (clic)              | Nouvel onglet shell                               |
| `+` (clic droit)        | Menu : un shell simple, puis les entrées configurées |
| onglet (clic gauche)    | Activer cet onglet                                |
| onglet (clic droit)     | Renommer, icône, fond, réinitialiser, fermer      |
| `cmd+t`                 | Nouvel onglet shell                               |
| `cmd+w`                 | Fermer l'onglet courant                           |
| `cmd+n` / `cmd+p`       | Onglet suivant / précédent                        |
| `cmd+=` / `cmd+-`       | Taille de police                                  |
| `cmd+0`                 | Réinitialiser la taille de police                 |
| `cmd+v`                 | Coller (avec bracketed paste si le programme le demande) |
| `cmd+k` (dans le terminal) | Effacer l'écran (envoie `Ctrl+L`)             |
| `cmd+l` (dans le terminal) | Effacer jusqu'à la fin de la ligne (envoie `Ctrl+K`) |
| `cmd+r` (dans le terminal) | Remonter en haut de l'historique              |
| `cmd+end` ou `cmd+down` | Descendre en bas                                  |
| molette de la souris    | Parcourir l'historique                            |

Quand une session s'est terminée, `Entrée` ou `r` la relance. Les menus du clic
droit sont les équivalents souris de `cmd+w` : rien ici n'oblige à mémoriser un
raccourci. La liste ci-dessus est celle de macOS.

## Configuration

`~/.config/spellcode/config.toml` est créé au premier lancement. Il est
facultatif, et c'est l'unique source de vérité : le clic droit sur `+` liste
exactement les entrées qui y sont déclarées, à côté d'un shell simple.

```toml
[general]
# Le shell lancé par l'entrée « Shell ». Vide signifie $SHELL, puis /bin/zsh.
shell = ""
# Vide signifie « essayer la liste intégrée ci-dessous ». Une famille non
# installée est ignorée plutôt que de basculer silencieusement vers une police
# proportionnelle.
font_family = ""
font_size = 13
line_height = 1.4
scrollback = 10000
# Opacité de la couche noire posée sur le flou de macOS, pour le chrome de la
# fenêtre. Plus bas est plus transparent.
window_tint = 0.45
# Opacité du fond du terminal. Plus bas laisse davantage passer le flou.
terminal_opacity = 0.42
# Marge régulière entre le bord de la zone du terminal et la grille. Elle
# absorbe aussi le reste de sous-cellule, pour que le texte ne touche jamais
# le bord.
padding = 14

[[apps]]
name = "Tracker API"
command = "tracker-api"
args = []
cwd = "~/src/my-repo"           # optionnel, vaut le dossier courant par défaut
```

Quand `font_family` est vide, la première famille installée parmi `JetBrains
Mono`, `SF Mono`, `Menlo`, `Monaco`, `Cascadia Code`, `Fira Code` et
`monospace` est utilisée. Une famille nommée dans la configuration mais non
installée est ignorée plutôt que de basculer silencieusement vers une police
proportionnelle.

Les entrées dont l'exécutable est absent de `PATH` sont marquées *non installé*
dans le menu au lieu d'être masquées, ce qui permet de garder une liste qui
voyage entre plusieurs machines. Supprimez tous les blocs `[[apps]]` pour n'avoir
jamais qu'un shell.

## Apparence

- La fenêtre est translucide et une unique couche noire la recouvre entièrement,
  si bien que le flou se lit comme une seule surface derrière la barre d'onglets
  et les marges. La carte du terminal est totalement opaque par-dessus, pour que
  le texte reste lisible.
- Le chrome est strictement en niveaux de gris. Le terminal garde une vraie
  palette de 256 couleurs, car les programmes plein écran en dépendent.
- Les onglets portent une icône Font Awesome à gauche et une couleur de fond,
  toutes deux choisies dans le menu du clic droit. `Reset` les rétablit.
- Le point sur un onglet indique son état : vif quand la session tourne, atténué
  quand la vue est remontée dans l'historique, pâle une fois la session
  terminée.

## Structure

```
crates/
  spellcode-term/   pas d'interface : émulation VT et PTY
    term.rs         TerminalCore au-dessus d'alacritty_terminal
    pty.rs          lancement de processus, thread de lecture, entrée
  spellcode-app/    GPUI
    main.rs         création de la fenêtre, barre de titre transparente
    workspace.rs    barre d'onglets, panneaux, raccourcis globaux
    overlay.rs      menu contextuel, dialogue de renommage, sélecteurs d'icône et de couleur
    icon.rs         police Font Awesome embarquée et liste d'icônes
    terminal.rs     une session : rendu de la grille, curseur, encodage des touches
    keys.rs         frappe clavier vers séquences d'octets ANSI
    theme.rs        chrome monochrome, palette terminal 256 couleurs
    config.rs       chargement de config.toml
```

## Comment le terminal reste rapide

- **Ne jamais redessiner quand rien ne s'est passé.** Un panneau n'est notifié
  que lorsque le PTY a produit de la sortie ou que le curseur a clignoté : un
  terminal au repos ne coûte rien. Un repaint complet de la zone visible coûte
  alors à peu près ce que coûterait un curseur qui clignote.
- **Un seul appel de mise en forme par plage de style.** Les cellules adjacentes
  qui partagent premier plan, arrière-plan et graisse deviennent une seule
  chaîne : une grille de 200x50 forme donc quelques centaines de lignes mises en
  forme, pas dix mille glyphes. GPUI met les dispositions en cache, si bien
  qu'un repaint de texte inchangé est presque gratuit.
- **La sortie est vidée par lots.** Un thread de lecture remplit des blocs de
  64 Kio dans un canal ; l'interface interroge toutes les 8 ms et transmet d'un
  coup tout ce qu'elle trouve à l'émulateur, si bien qu'un programme qui
  inonde le PTY coûte une image, pas une image par écriture.
- **Vraies 256 couleurs, 24 bits.** Les cellules nommées, indexées et en couleur
  directe passent toutes par la palette xterm complète, et les requêtes de
  couleur OSC reçoivent une réponse pour que les shells et les invites détectent
  le vrai arrière-plan. Le chrome reste strictement noir et blanc, le terminal
  garde ses couleurs.

## Contribuer

Les issues et les pull requests sont bienvenues. Avant d'ouvrir une pull
request, lancez `cargo build --release` et `cargo test --workspace`, et gardez
des changements concentrés sur un seul point. Il n'y a pas de guide de
contribution au-delà de cela ; posez la question dans l'issue si quelque chose
n'est pas clair.

## Sécurité

Merci de signaler un problème de sécurité en privé via les
[avis de sécurité](https://github.com/kawazoeh/Spellcode/security/advisories/new)
de ce dépôt, plutôt que dans une issue publique, afin qu'un correctif puisse
être préparé avant que le rapport ne soit visible.

## Licence

MIT. Voir [LICENSE](LICENSE).
