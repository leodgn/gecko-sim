# Feuille de route — gecko-sim

Pédagogique, pas un plan d'exécution automatique : **c'est toi qui écris le
code**. Coche au fur et à mesure. Demande de l'aide à n'importe quelle étape
si tu bloques — l'idée est que je t'explique le *comment* (concepts Rust,
pièges, API) sans écrire l'implémentation à ta place.

Design complet : `docs/superpowers/specs/2026-09-24-gecko-sim-design.md`.
Référence matérielle : `ressources/hardware-spec.md`.

## 0. Setup

- [x] `cargo init` (crate binaire, pas de workspace).
- [ ] Ajouter `eframe`/`egui` comme dépendance (juste pour vérifier que ça
      compile et affiche une fenêtre vide — pas de logique encore).
      `cargo add eframe` : fait. Reste à lancer
      `cargo run --example eframe_check` (fichier jetable déjà écrit,
      `examples/eframe_check.rs`) et vérifier que la fenêtre s'ouvre. Une
      fois confirmé, supprimer `examples/eframe_check.rs`.
- [x] Récupérer les sources de `lib-rv32-isa` pour préparer le vendoring de
      l'étape 2 — cloné directement dans `lib-rv32/` à la racine du projet
      (ajouté à `.gitignore`, ce n'est qu'un dossier de travail temporaire).

*Concepts Rust : structure d'un crate binaire, `Cargo.toml`, `cargo run`.*

## 1. Register file

- [ ] Écrire un type qui implémente le trait `RegisterFile` de `lib-rv32`
      (`read(num: u8) -> Result<u32, RiscvError>`, `write(num, data)`).
      Stockage : un tableau de 32 `u32`. `x0` doit toujours lire 0, même
      après une écriture dessus (le hardware RISC-V l'exige).
- [ ] Test unitaire : écrire dans `x0`, vérifier qu'une lecture renvoie 0.
      Écrire dans un autre registre, vérifier qu'on relit la bonne valeur.

*Concepts Rust : `trait` + `impl` pour un type, tableaux fixes `[T; N]`,
`Result`.*

## 2. Vendorer `isa-sim` + corriger le bug sub/add

- [ ] Copier les fichiers de `lib-rv32-isa/src/` (decode.rs, exec.rs,
      traits.rs, error.rs, util.rs — ~720 lignes au total) dans un module
      `src/cpu/` de ton projet. Adapter les imports (plus besoin de
      `lib_rv32_common`, tout est dans ton crate maintenant).
- [ ] Localiser le bug dans la branche add/sub de `exec_one` (recherche
      `FUNC7_SUB`) : le code re-teste `decode_func3!(ir)` au lieu de
      `decode_func7!(ir)` pour distinguer `add` de `sub`. Corriger, et
      laisser un commentaire qui explique le bug d'origine (bon pour un
      rapport de projet : bug trouvé et corrigé dans une lib tierce).
- [ ] Test : encoder à la main une instruction `sub` (ou utiliser un
      assembleur RISC-V si tu en as un sous la main), l'exécuter contre
      un register file bidon, vérifier que le résultat est bien une
      soustraction et pas une addition.

*Concepts Rust : modules (`mod`), visibilité (`pub`), macros (tu n'as pas
besoin de comprendre `macro_rules!` en détail, juste de savoir les
utiliser).*

## 3. Bus mémoire (RAM plate, sans périphériques pour l'instant)

- [ ] Écrire un type `Bus` qui implémente le trait `Memory` de `lib-rv32`
      (`fetch`, `read_word/half_word/byte`, `write_word/half_word/byte`).
      Pour l'instant : uniquement deux régions RAM plates (voir le plan
      mémoire dans le spec) — pas encore de dispatch vers les
      périphériques.
- [ ] Attention à la traduction adresse → index dans le `Vec`/tableau
      (l'adresse `0x80000000` ne doit pas être l'index 0 littéral d'un
      `Vec` de plusieurs Go — calcule un offset par région).
- [ ] Test : écrire un mot à une adresse, le relire, vérifier l'égalité.
      Tester aussi `read_byte`/`read_half_word` sur un mot qu'on vient
      d'écrire (attention à l'endianness — RISC-V est little-endian).

*Concepts Rust : `Vec<u8>`, indexation, gestion d'erreurs avec `Result`
pour les accès hors plage.*

## 4. Chargeur de binaire

- [ ] Lire un fichier `.bin` (`std::fs::read`) et le copier dans la RAM du
      `Bus` à partir de `0x80000000`.
- [ ] Boucle d'exécution minimale : `pc = 0x80000000`, boucle qui appelle
      `exec_one(&mut pc, &mut bus, &mut regfile)` en `loop {}`, affiche
      l'erreur et s'arrête si `exec_one` renvoie `Err`.
- [ ] Test manuel : écrire un mini programme assembleur RISC-V (quelques
      instructions RV32I), l'assembler en `.bin` (`riscv64-unknown-elf-gcc
      -march=rv32i -mabi=ilp32 ...` + `objcopy`, voir le Makefile du cours),
      le charger, vérifier via des logs que les registres ont les bonnes
      valeurs à la fin.

*Concepts Rust : `std::fs`, `io::Result`, boucles infinies contrôlées,
`std::process::exit` ou `panic!` pour un arrêt propre sur erreur.*

## 5. Périphériques (un par un, avec test unitaire à chaque fois)

Fais-les dans cet ordre (du plus simple au plus utile pour valider vite) :

- [ ] **`RANDOM`** (`0x40000000`) : xorshift32 avec graine fixe câblée en
      dur. Test : deux instances fraîches produisent la même séquence de
      lectures.
- [ ] **`BUTTONS`** (`0x70000004`) : logique "front descendant" (un clic
      met le bit à 1, il reste à 1) + "n'importe quelle écriture efface
      tout le registre". Test : simuler un clic, lire le registre,
      simuler une écriture CPU, vérifier que tout est à 0.
- [ ] **`SEVEN_SEGS`** (`0x60000000`) : lecture/écriture normale d'un mot
      de 4 octets. Test trivial (écrire/relire).
- [ ] **`LEDS`** (`0x50000000`) : le plus complexe des 4 — décoder les 4
      cas de sélection ligne/colonne dans `hardware-spec.md` et mettre à
      jour un framebuffer `[[u8; 12]; 10]` par couleur. Écriture only,
      lecture renvoie toujours 0. Teste chaque cas de sélection
      séparément (toutes lignes+colonnes, une colonne, une ligne, une
      seule LED).
- [ ] Brancher les 4 périphériques dans `Bus::read_*`/`write_*` par plage
      d'adresse (remplace le TODO du bus par un vrai dispatch).

*Concepts Rust : `match` sur des plages/bits, opérations bit à bit (`&`,
`|`, `<<`, `>>`), tests unitaires (`#[test]`, `assert_eq!`).*

## 6. Multithreading : CPU en continu + état partagé

- [ ] Faire tourner la boucle d'exécution dans un thread dédié
      (`std::thread::spawn`).
- [ ] Partager le framebuffer LEDs, l'état 7-seg, et le registre BUTTONS
      entre le thread CPU et le futur thread UI via `Arc<Mutex<...>>`.
- [ ] Vérifier que ça compile et tourne sans deadlock (le thread CPU ne
      doit jamais garder le verrou plus longtemps que nécessaire — prendre
      le lock, lire/écrire, relâcher, pas de lock qui traverse toute
      l'itération).

*Concepts Rust clé pour ce projet : `Arc`, `Mutex`, `std::thread`, `move`
closures. C'est probablement la partie la plus "nouvelle" si tu viens de
Scala — pas d'acteurs ici, juste du partage d'état classique verrouillé.*

## 7. UI (`egui`/`eframe`)

- [ ] Squelette d'app `eframe` qui tourne en boucle et lit l'état partagé
      à chaque frame (pas besoin de VSync particulier, `egui` gère ça).
- [ ] Dessiner la grille de LEDs 12×10 en couleur (rectangles colorés,
      voir `LedArray.svelte` de l'extension `cs200` pour l'inspiration
      visuelle exacte si tu veux coller au rendu).
- [ ] Dessiner les 4 afficheurs 7-segments (table `font_data` dans
      `gol.s` pour interpréter les motifs de segments).
- [ ] Dessiner le pavé directionnel (5 boutons) + la deuxième rangée de 5
      boutons + les dip switches (widget visuel seulement, pas câblé).
      Boutons cliquables à la souris (mousedown → bit à 1, mouseup/leave
      → rien de spécial côté émulateur, le bit reste jusqu'à clear CPU).
- [ ] Bouton "charger un .bin" (file picker ou argument CLI, à toi de
      choisir) qui (re)lance l'émulation.

*Concepts Rust : `egui::Context`, immediate mode (le code de dessin
s'exécute à chaque frame, pas de retained widget tree comme dans une UI
classique — assez différent de ce que tu as pu voir ailleurs).*

## 8. Test de non-régression avec `gol.s`

- [ ] Charger `gol.s` compilé avec `seed0`, exécuter une génération
      (headless, sans passer par `eframe` — juste la boucle CPU + le
      framebuffer), vérifier que les 3 formes stables (2 blocs 2×2 + 1
      ruche) sont identiques avant/après.

## 9. Polish (optionnel, une fois que tout marche)

- [ ] Affichage d'erreur propre dans l'UI si le CPU plante (opcode
      invalide, accès mémoire hors plan mémoire) au lieu d'un crash du
      process.
- [ ] Vitesse d'affichage : si le rendu 60 Hz peine à suivre un CPU qui
      tourne à pleine vitesse, réfléchir à un throttle ou à un simple
      "dernier état visible" sans bloquer le thread CPU.
