<p align="center">
  <img src="assets/spellcode01.png" alt="L'icône de Spellcode : une étoile à quatre branches encadrée de crochets au-dessus, en dessous et de chaque côté, dessinée en traits pâles sur fond blanc." width="96">
</p>

<h1 align="center">Spellcode</h1>

<p align="center"><strong>Un terminal rapide et monochrome, pour macOS et Windows.</strong></p>

<p align="center">
  <a href="https://github.com/kawazoeh/Spellcode/actions"><img src="https://img.shields.io/github/actions/workflow/status/kawazoeh/Spellcode/ci.yml?branch=main&label=build" alt="État du build"></a>
  <a href="https://github.com/kawazoeh/Spellcode/releases"><img src="https://img.shields.io/github/v/release/kawazoeh/Spellcode" alt="Dernière version"></a>
  <img src="https://img.shields.io/badge/macOS-11%2B-black?logo=apple&logoColor=white" alt="macOS 11 ou plus récent">
  <img src="https://img.shields.io/badge/Windows-10%2B-0078D6?logo=windows&logoColor=white" alt="Windows 10 ou plus récent">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/kawazoeh/Spellcode" alt="Licence MIT"></a>
</p>

<p align="center"><a href="README.md">English</a> · Français</p>

Spellcode est un terminal à onglets qui sait se faire oublier. Écrit en Rust, il
repose sur [GPUI](https://gpui.rs), le framework d'interface accéléré par GPU des
créateurs de Zed, et dessine chaque panneau sur la carte graphique. Le chrome de
l'application est en noir et blanc ; votre shell, lui, garde ses 256 couleurs,
car les programmes plein écran n'attendent rien de moins.

La barre d'onglets **est** la barre de titre. Pas de bandeau gris supplémentaire
au-dessus du terminal : les boutons de fenêtre sont alignés avec le nom, la
barre est translucide sur le flou du système, et `+` ouvre un nouveau shell (ou
n'importe quel programme ajouté à la configuration). Un shell tourne déjà au
moment où la fenêtre s'affiche.

Et il est vif, parce qu'il refuse le travail inutile : un panneau ne se redessine
que lorsque le PTY produit vraiment de la sortie ou que le curseur clignote. Un
onglet au repos ne coûte donc rien. Les détails sont [sous le capot](#sous-le-capot),
si ça vous intéresse.

## Installation

Chaque version publie les six mêmes fichiers. Récupérez celui de votre machine
sur la [page des releases](https://github.com/kawazoeh/Spellcode/releases) :

| Plateforme            | Fichier                              |
| --------------------- | ------------------------------------ |
| macOS · Apple silicon | `Spellcode-macos-arm64.dmg`          |
| macOS · Apple silicon | `Spellcode-macos-arm64.zip`          |
| macOS · Intel         | `Spellcode-macos-x86_64.dmg`         |
| macOS · Intel         | `Spellcode-macos-x86_64.zip`         |
| Windows · x64         | `Setup-Spellcode-<version>-x64.exe`  |
| Windows · x64         | `Spellcode-windows-x86_64.zip`       |

**macOS.** Prenez le `.dmg`, ouvrez-le et glissez Spellcode dans Applications.
Le `.zip` est le même bundle sans l'image disque, si vous préférez. La version
est signée en ad-hoc, pas avec un Developer ID, donc Gatekeeper posera sa
question une fois : clic droit sur l'application, **Ouvrir**, puis confirmez.
Ensuite, un double-clic ordinaire suffit. Pour qu'elle s'ouvre sans rien
demander dès le premier lancement, il faudrait un Apple Developer ID payant et
une notarisation ; [docs/macos.md](docs/macos.md) détaille précisément ce qui est
signé et ce qui ne l'est pas.

**Windows.** L'installateur `Setup-…-x64.exe` ajoute une entrée au menu Démarrer,
un désinstalleur et une ligne dans *Programmes et fonctionnalités* ; le `.zip`
est là pour une version portable, si vous préférez ne rien installer. Aucun des
deux n'est signé, donc SmartScreen affiche « Windows a protégé votre PC » au
premier lancement : **Informations complémentaires**, puis **Exécuter quand
même**. Les notes de build sont dans [docs/windows.md](docs/windows.md).

## Compiler soi-même

Il faut Rust 1.85 ou plus récent, et c'est à peu près tout. Sous macOS, pas
besoin d'une installation complète de Xcode : le moteur de rendu `macos-blade`,
activé par défaut, compile les shaders de GPUI via Rust plutôt que via la chaîne
`metal` d'Xcode.

```sh
git clone https://github.com/kawazoeh/Spellcode spellcode
cd spellcode
cargo build --release
```

Pour obtenir un vrai bundle macOS (scellé et signé en ad-hoc, pour que le Finder
ne le déclare pas endommagé), puis une image disque :

```sh
scripts/bundle.sh   # target/Spellcode.app
scripts/dmg.sh      # Spellcode.dmg
```

Sous Windows, retirez le moteur de rendu propre à macOS et compilez le workspace.
GPUI compile ses shaders HLSL avec `fxc.exe` au moment du build : un SDK Windows
doit donc être installé et détectable ; [docs/windows.md](docs/windows.md) couvre
le reste.

```sh
cargo build --release --no-default-features
```

Le binaire se trouve dans `target/release/spellcode-app` (`spellcode-app.exe`
sous Windows). Lancez les tests avec :

```sh
cargo test --workspace
```

## Raccourcis clavier

| Raccourci                  | Action                                              |
| -------------------------- | --------------------------------------------------- |
| `+` (clic)                 | Nouvel onglet shell                                 |
| `+` (clic droit)           | Menu : un shell simple, puis les entrées configurées |
| onglet (clic gauche)       | Activer cet onglet                                  |
| onglet (clic droit)        | Renommer, icône, fond, réinitialiser, fermer        |
| `cmd+t`                    | Nouvel onglet shell                                 |
| `cmd+w`                    | Fermer l'onglet courant                             |
| `cmd+n` / `cmd+p`          | Onglet suivant / précédent                          |
| `cmd+=` / `cmd+-`          | Taille de police                                    |
| `cmd+0`                    | Réinitialiser la taille de police                   |
| `cmd+v`                    | Coller (avec bracketed paste si le programme le demande) |
| `cmd+k` (dans le terminal) | Effacer l'écran (envoie `Ctrl+L`)                   |
| `cmd+l` (dans le terminal) | Effacer jusqu'à la fin de la ligne (envoie `Ctrl+K`) |
| `cmd+r` (dans le terminal) | Remonter en haut de l'historique                    |
| `cmd+end` ou `cmd+down`    | Descendre en bas                                    |
| molette de la souris       | Parcourir l'historique                              |

`Entrée` ou `r` relance une session terminée. Les menus du clic droit reprennent
`cmd+w`, donc rien ne vous oblige à mémoriser un raccourci. La liste ci-dessus est
celle de macOS.

## Configuration

Tout se passe dans `~/.config/spellcode/config.toml`, créé au premier lancement
et entièrement facultatif. C'est aussi le seul endroit où le menu de nouvel
onglet regarde : un clic droit sur `+` donne un shell simple et exactement les
entrées que vous y avez déclarées, rien de plus.

```toml
[general]
# Le shell lancé par l'entrée « Shell ». Vide signifie $SHELL, puis /bin/zsh.
shell = ""
# Vide signifie « essayer la liste ci-dessous ». Une famille non installée est
# ignorée plutôt que de basculer silencieusement vers une police proportionnelle.
font_family = ""
font_size = 13
line_height = 1.4
scrollback = 10000
# Opacité de la couche noire posée sur le flou de macOS, pour le chrome de la
# fenêtre. Plus bas est plus transparent.
window_tint = 0.45
# Opacité du fond du terminal. Plus bas laisse davantage passer le flou.
terminal_opacity = 0.42
# Marge régulière entre le bord de la zone du terminal et la grille. Elle absorbe
# aussi le reste de sous-cellule, pour que le texte ne touche jamais le bord.
padding = 14

[[apps]]
name = "Tracker API"
command = "tracker-api"
args = []
cwd = "~/src/my-repo"           # optionnel, vaut le dossier courant par défaut
```

Si `font_family` reste vide, la première famille installée parmi `JetBrains
Mono`, `SF Mono`, `Menlo`, `Monaco`, `Cascadia Code`, `Fira Code` et
`monospace` l'emporte. Un nom absent est ignoré, jamais remplacé en douce par
une police proportionnelle. Les entrées dont l'exécutable n'est pas dans le
`PATH` s'affichent en grisé comme *non installé* plutôt que d'être masquées, pour
qu'un fichier de configuration puisse voyager d'une machine à l'autre. Supprimez
tous les blocs `[[apps]]` et vous n'aurez jamais qu'un shell.

## Sous le capot

### Structure

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

### Pourquoi c'est rapide

Un panneau n'est notifié que lorsque le PTY a produit de la sortie ou que le
curseur a clignoté : un terminal au repos ne fait donc rien, et un repaint complet
coûte à peu près ce qu'un curseur clignotant coûterait. Les cellules voisines qui
partagent premier plan, arrière-plan et graisse sont façonnées d'un seul tenant,
donc une grille de 200×50 donne quelques centaines de lignes au lieu de dix mille
glyphes, et GPUI met les dispositions en cache. La sortie est vidée par lots : un
thread de lecture remplit des blocs de 64 Kio, l'interface les récupère toutes
les 8 ms, et un programme qui inonde le PTY coûte une image au lieu d'une par
écriture.

Côté couleur, les cellules nommées, indexées et en couleur directe passent toutes
par la palette xterm complète, et les requêtes OSC reçoivent une réponse pour que
les shells et les invites détectent le vrai arrière-plan. Le chrome reste
strictement noir et blanc, le terminal garde ses couleurs.

## Contribuer

Les issues et les pull requests sont bienvenues. Lancez `cargo build --release`
et `cargo test --workspace` avant d'ouvrir une PR, et gardez chaque changement
concentré sur un seul point. Pas de guide de contribution au-delà de ça ;
demandez dans l'issue si quelque chose n'est pas clair.

## Sécurité

Un problème de sécurité ? Signalez-le en privé via les
[avis de sécurité](https://github.com/kawazoeh/Spellcode/security/advisories/new)
du dépôt plutôt que dans une issue publique, pour qu'un correctif puisse arriver
avant que le rapport ne soit visible.

## Licence

MIT. Voir [LICENSE](LICENSE).
