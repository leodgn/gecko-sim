# gecko-sim — Design

Date : 2026-09-24
Statut : approuvé (voir historique de conversation pour le brainstorm complet)

## Contexte et but

Remplacer `Vtb` (simulateur RTL officiel du cours cs200, EPFL) par un émulateur
Rust rapide pour itérer sur `ressources/gol.s` (jeu de la vie en assembleur
RISC-V). `Vtb` prenait 21s/étape, réduit à 2,43s après optimisation — encore
trop lent. Projet également utilisé comme apprentissage de Rust par l'auteur
: **priorité à la clarté et à l'idiomatique plutôt qu'à la performance**, le
volume de calcul réel est faible (dizaines de milliers d'instructions RISC-V
par étape).

## Décisions de scope (verrouillées par ce document)

- Cœur RV32I : **`lib-rv32`** (`trmckay/lib-rv32`, MIT), **vendoré** dans le
  repo (voir section Cœur RV32I ci-dessous), pas réécrit à la main.
- Interface : **UI graphique native en Rust (egui/eframe)**, pas de terminal,
  pas de protocole DAP. Décision explicite : bien qu'il soit techniquement
  possible de faire parler notre émulateur en Debug Adapter Protocol pour
  brancher directement l'extension VS Code `cs200` existante dessus (elle
  lance `Vtb` comme serveur DAP et relaie des événements custom
  `boardUpdate`/`updateInput` à une webview Svelte), l'auteur préfère
  construire sa propre UI plutôt que de dépendre de ce protocole.
- Comportement voulu : **fidèle à l'extension `cs200` visuellement et dans sa
  logique matérielle**, mais sans les fonctionnalités de debugging
  (breakpoints, step) — comme une vraie carte qu'on allume et qui exécute son
  programme immédiatement, en continu, jusqu'à l'arrêt de l'appli.
- Mode de travail : **l'auteur écrit tout le code lui-même** (apprentissage
  Rust). Ce document fixe les décisions d'architecture ; `TODO.md` sert de
  feuille de route pédagogique, sans code.

## Cœur RV32I : `lib-rv32` (vendoré, avec correctif)

Spike de vérification effectué (voir historique) :

- Les traits `Memory`/`RegisterFile` (crate `lib-rv32-isa`, module `traits`)
  correspondent exactement au point d'accroche prévu : `fetch`,
  `read_word/half_word/byte`, `write_word/half_word/byte` pour `Memory` ;
  `read(num)`/`write(num, data)` pour `RegisterFile`.
- Le cœur d'exécution est une fonction libre `exec_one(pc: &mut u32, mem: &mut
  M, rf: &mut R) -> Result<(), RiscvError>` — un appel par instruction, aucun
  état caché. Il ne décode **que** RV32I pur (pas de M/A/C), malgré ce
  qu'annonce le README du dépôt.
- **Bug confirmé par exécution réelle** (pas seulement lecture de code) :
  dans `isa-sim/src/exec.rs`, la distinction `add`/`sub` (même `func3`,
  distingués normalement par `func7`) teste par erreur `decode_func3!(ir)` au
  lieu de `decode_func7!(ir)`. Conséquence : **`sub` s'exécute comme `add`**.
  Vérifié en encodant à la main `sub x5, x6, x7` avec `x6=10, x7=3` : résultat
  `13` (10+3) au lieu de `7` (10-3).
  `gol.s` utilise `sub` (ligne 232, décompte du timer de vitesse) — un vrai
  risque de correctness, pas hypothétique.
- Dépôt à l'arrêt depuis le 25 août 2021, mainteneur unique, MIT, ~720 lignes
  pour tout `isa-sim`. Publié sur crates.io (`lib-rv32-isa` 0.2.0,
  `lib-rv32-common` 0.2.0).

**Décision** : vendorer le code de `lib-rv32-isa` (les ~720 lignes) comme
module interne du projet (`src/cpu/`), corriger le bug avec un commentaire
expliquant le correctif. Pas de dépendance sur un dépôt à l'arrêt, code
entièrement lisible/auditable, correctif documenté et assumé (pas de version
patchée cachée à livrer).

## Architecture

Un seul crate binaire (pas de workspace, projet trop petit pour le justifier).

```
src/
  main.rs          — point d'entrée : charge le .bin, lance le thread CPU, lance eframe
  cpu/              — isa-sim vendoré (decode/exec RV32I) + le patch sub/add, documenté
  regfile.rs        — implémente RegisterFile ([u32; 32], x0 câblé à 0)
  bus.rs            — implémente Memory : dispatch par plage d'adresse vers RAM ou périphériques
  peripherals/
    leds.rs         — décode les commandes d'écriture, maintient le framebuffer 10×12×3
    seven_segs.rs
    buttons.rs      — bits + logique "front descendant" + clear-on-any-write
    random.rs       — xorshift32, graine fixe (même séquence à chaque relance)
  loader.rs         — lit le .bin, le place à 0x80000000
  ui.rs             — l'app eframe : dessine LEDs/7-seg/boutons/joystick/dip switches, capture les clics souris
```

### Flux d'exécution

Au démarrage : chargement du binaire, spawn d'un thread dédié qui boucle
`exec_one(pc, bus, regfile)` en continu (aussi vite que possible), pendant
que le thread principal fait tourner `eframe`. État partagé (framebuffer
LEDs, 7-seg, registre `BUTTONS`) derrière un `Arc<Mutex<...>>` : le thread
CPU écrit, le thread UI lit à chaque frame (~60 Hz) et écrit les entrées
utilisateur. Pas de synchronisation fine nécessaire (échelle : dizaines de
milliers d'instructions par étape de jeu) — clarté avant perf.

### Point d'entrée

Le `.bin` est déjà `objcopy`é (format brut, sans table de symboles). Pas
d'ambiguïté `_start` à résoudre (contrairement à ce que suggérait
`hardware-spec.md`) : **le PC démarre toujours à `0x80000000`** au
"power on".

### Mémoire (`bus.rs`)

Dispatch par plage d'adresse (voir `ressources/hardware-spec.md`, Table 2) :

| Adresse | Comportement |
|---|---|
| `0x40000000` | `RANDOM` (peripheral) |
| `0x50000000` | `LEDS` (peripheral, write-only) |
| `0x60000000` | `SEVEN_SEGS` (peripheral) |
| `0x70000004` | `BUTTONS` (peripheral) |
| `0x80000000` et suivants | RAM plate (code/data/pile) |
| `0x90001000`–`0x90001300` | RAM plate (état du jeu, GSA, variables custom) |
| ailleurs | erreur d'accès mémoire |

Deux régions RAM séparées (pas un unique `Vec` géant couvrant tout l'espace
d'adressage 32 bits) : une pour `0x80000000+` (code/data/pile), une pour
`0x90001000..0x90001300` (état du jeu). Tailles à choisir raisonnablement
généreuses.

### Périphériques

- **`LEDS` (0x50000000)** — write-only. Chaque écriture est une commande
  (row/col/couleur/valeur, voir `hardware-spec.md`), pas un état stocké.
  Framebuffer interne `[[u8; 12]; 10]` par couleur (r/g/b). Lecture renvoie
  toujours 0.
- **`SEVEN_SEGS` (0x60000000)** — 4 octets empaquetés dans un mot,
  lecture/écriture normale (comportement en lecture non documenté dans le
  PDF → RAM normale par défaut).
- **`BUTTONS` (0x70000004)** — 10 bits physiques, deux groupes de 5 dans
  l'UI (comme dans l'extension `cs200`) :
  - pavé directionnel : JT/JB/JL/JR/JC (bits 4,3,2,1,0)
  - rangée de boutons : BUTTON_0/BUTTON_1/BUTTON_2 (bits 6,5,7) + 2 bits non
    nommés dans le template (8,9)
  Sémantique : un clic souris met le bit à 1 sur le front descendant
  (relâché→pressé) ; il reste à 1 jusqu'à ce que le CPU écrive n'importe quoi
  dans le registre (efface tout d'un coup). **Interaction à la souris**
  (mousedown/mouseup), pas au clavier — fidèle à l'extension `cs200`
  (composants Svelte `PushButton`/`JoyStick` observés, tous pilotés par la
  souris).
- **`RANDOM` (0x40000000)** — xorshift32 (ou LCG) avec **graine fixe câblée
  en dur** dans le code. Confirmé dans `GameOfLife.pdf` section 3.4.1 : « It
  is therefore expected that the random number generator will return the
  same sequence of numbers each time the program is run. » → même séquence
  à chaque relance de notre émulateur. Pas de tentative de reproduire `Vtb`
  bit-à-bit (impossible sans son code source RTL).
- **Dip switches** — affichés dans l'UI pour la fidélité visuelle avec
  l'extension `cs200` (composant Svelte `dipSwitches`), mais **non câblés à
  une adresse MMIO** : ni le plan mémoire ni `gol.s` ne les utilisent. Widget
  interactif sans effet sur l'émulation, documenté comme tel en commentaire.

### Gestion d'erreurs

Une erreur CPU (opcode invalide, accès mémoire hors plan mémoire) arrête le
thread CPU et remonte l'erreur de façon visible dans l'UI — pas de crash
silencieux, utile puisque l'outil sert justement à déboguer `gol.s`.

### Tests

- Tests unitaires par périphérique : décodage `LEDS` (les 4 cas de sélection
  ligne/colonne), `BUTTONS` (front descendant + clear-on-any-write),
  `RANDOM` (déterminisme : deux instances fraîches donnent la même
  séquence).
- Test de non-régression basé sur `seed0` de `gol.s` (mentionné dans
  `CLAUDE.md`) : charger `gol.s` compilé, exécuter une génération, vérifier
  via le framebuffer LEDs que les 3 formes stables (2 blocs 2×2 + 1 ruche)
  sont identiques avant/après. Faisable en mode headless (le framebuffer est
  une structure de données pure, testable sans lancer `eframe`).
- Test du correctif `sub`/`add` sur le module `cpu/` vendoré.

## Rejeté / hors scope

- **Protocole DAP + réutilisation de l'extension `cs200`** : techniquement
  viable (vérifié : `Vtb` lie `libcppdap`, l'extension est un client DAP
  générique qui écoute un événement custom `boardUpdate` et envoie une
  requête custom `updateInput`), mais explicitement écarté par l'auteur au
  profit d'une UI propre en Rust.
- **Reproduction bit-exacte de `Vtb` pour `RANDOM`** : impossible sans le
  code source RTL du générateur matériel.
- **Cœur RV32I écrit à la main** : écarté, `lib-rv32` (vendoré + patché)
  suffit largement.
- **Debugging (breakpoints, step)** : hors scope, contraire au but recherché
  (vitesse, comportement "vraie carte qu'on allume").
