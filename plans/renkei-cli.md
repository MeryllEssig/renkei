# Plan : Renkei CLI (`rk`)

> Source PRD : `./PRD.md` — Package Manager pour Workflows Agentiques

## Decisions architecturales

Decisions durables qui s'appliquent a toutes les phases :

- **Langage** : Rust, binaire unique `rk`
- **Trait Backend** : `Backend` avec methodes `name()`, `detect_installed()`, `deploy_skill()`, `deploy_hook()`, `deploy_agent()`, `register_mcp()`. Seul `ClaudeBackend` en v1.
- **Manifeste** : `renkei.json` — champs obligatoires : `name` (scope `@scope/nom`), `version` (semver), `description`, `author`, `license`, `backends`. Optionnels : `keywords`, `mcp`, `requiredEnv`, `workspace`.
- **Convention over config** : artefacts decouverts depuis `skills/`, `hooks/`, `agents/`. Pas de champ `artifacts` dans le manifeste.
- **Chemins de deploiement (hardcodes)** :
  - Skills → `~/.claude/skills/renkei-<name>/SKILL.md`
  - Hooks → merge dans `~/.claude/settings.json`
  - Agents → `~/.claude/agents/<name>.md`
  - MCP → merge dans `~/.claude.json`
- **Stockage local** :
  - `~/.renkei/cache/@scope/name/<version>.tar.gz` (archives immutables)
  - `~/.renkei/install-cache.json` (mapping packages → artefacts deployes)
  - `rk.lock` a la racine du projet (lockfile commitable)
- **Home directory injectable** : toute fonction lisant/ecrivant `~/.claude/` ou `~/.renkei/` accepte un chemin de base configurable (struct `Config` avec `home_dir: PathBuf`) pour permettre les tests en tempdir.
- **Hooks tracking** : dans `install-cache.json`, jamais dans le JSON du backend. Le JSON backend reste 100% natif.
- **Fail-fast + rollback** : chaque installation est atomique. Collecte des ecritures dans un `Vec`, rollback en ordre inverse sur erreur.
- **Crates principaux** : `clap` (derive), `serde` + `serde_json`, `semver`, `tar` + `flate2`, `sha2`, `tempfile`, `thiserror`, `dialoguer`, `colored`, `dirs`. Dev : `assert_cmd`, `predicates`.

---

## Phase 1 : Squelette CLI + Manifest + Deploiement local de skills

**User stories** : 4, 5, 9

### What to build

Le plus fin tracer bullet possible : `rk install ./chemin-local/` deploie un skill unique depuis un dossier local vers Claude Code.

Couvre de bout en bout : parsing CLI (clap) → lecture et validation du manifeste `renkei.json` → decouverte des skills par convention (`skills/`) → creation de l'archive `.tar.gz` dans le cache → deploiement du skill vers `~/.claude/skills/renkei-<name>/SKILL.md` → ecriture de `install-cache.json`.

Structure du projet Rust creee from scratch : `Cargo.toml`, `src/main.rs`, modules pour manifest, artifact, backend, cache, install, error.

### Acceptance criteria

- [ ] `cargo build` produit un binaire `rk`
- [ ] `rk install ./fixture/` avec un `renkei.json` valide et `skills/review.md` deploie le fichier vers `~/.claude/skills/renkei-review/SKILL.md`
- [ ] `rk install ./fixture/` avec un manifeste invalide (champ manquant, scope incorrect, semver invalide) echoue avec un message d'erreur descriptif
- [ ] L'archive `~/.renkei/cache/@scope/name/<version>.tar.gz` est creee
- [ ] `install-cache.json` contient l'entree du package avec les chemins deployes
- [ ] Tests unitaires : parsing manifeste (valide, champs manquants, mauvais scope, mauvais semver), decouverte artefacts, deploiement skill
- [ ] Test d'integration : `rk install ./fixture/` end-to-end en tempdir

---

## Phase 2 : Rollback + Agents + Reinstall

**User stories** : 11, 14

### What to build

Ajout du mecanisme de rollback atomique : pendant l'installation, chaque ecriture filesystem est enregistree. Sur erreur, toutes les ecritures sont annulees en ordre inverse.

Support des agents : decouverte depuis `agents/`, deploiement vers `~/.claude/agents/<name>.md`.

Support du reinstall : si un package est deja dans `install-cache.json`, ses anciens artefacts sont supprimes avant de redeployer la nouvelle version.

### Acceptance criteria

- [ ] Installation d'un package avec 2 skills et 1 agent deploie les 3 fichiers aux bons chemins
- [ ] Si un artefact echoue pendant le deploiement, tous les artefacts deja deployes sont supprimes (rollback)
- [ ] `rk install` sur un package deja installe supprime les anciens artefacts et deploie les nouveaux
- [ ] `install-cache.json` est mis a jour correctement apres reinstall
- [ ] Tests : rollback (deploy 2/3, erreur sur le 3e, assert les 2 premiers supprimes), multi-skill, agent deploy, reinstall

---

## Phase 3 : Hooks — deploiement + traduction d'evenements

**User stories** : 10

### What to build

Deploiement des hooks : decouverte depuis `hooks/*.json`, parsing du format Renkei abstrait (`event`, `matcher`, `command`, `timeout`), traduction vers les evenements natifs Claude Code (`before_tool` → `PreToolUse`, etc.), merge dans `~/.claude/settings.json`.

Le merge dans `settings.json` doit respecter la structure reelle : chaque cle d'evenement mappe vers un tableau d'objets `[{ "matcher": "...", "hooks": [{ "type": "command", "command": "...", "timeout": N }] }]`. Le merge append sans ecraser les hooks existants.

Tracking des hooks dans `install-cache.json`. Rollback etendu pour supprimer les hooks du settings.json en cas d'erreur.

### Acceptance criteria

- [ ] Les 11 evenements Renkei sont traduits correctement (`before_tool` → `PreToolUse`, `after_tool` → `PostToolUse`, etc.)
- [ ] `rk install` d'un package avec hooks merge les entrees dans `settings.json`
- [ ] Les hooks existants dans `settings.json` ne sont pas ecrases
- [ ] Le rollback retire uniquement les hooks du package en echec
- [ ] `install-cache.json` trace quels hooks appartiennent a quel package
- [ ] Tests : traduction des 11 evenements, parsing hooks JSON, merge settings.json (vide, existant, append), rollback hooks

---

## Phase 4 : MCP + Warnings variables d'environnement

**User stories** : 12, 13

### What to build

Enregistrement MCP : lecture du champ `mcp` du manifeste, merge dans `~/.claude.json` (section `mcpServers`). Tracking dans `install-cache.json`. Rollback MCP.

Verification des variables d'environnement : apres installation reussie, chaque variable de `requiredEnv` est verifiee. Warning affiche (pas bloquant) pour les variables manquantes.

### Acceptance criteria

- [ ] `rk install` d'un package avec `mcp` enregistre les serveurs dans `~/.claude.json`
- [ ] Les serveurs MCP existants ne sont pas ecrases
- [ ] Les variables d'environnement manquantes declenchent un warning (pas une erreur)
- [ ] Les variables presentes ne generent pas de warning
- [ ] Rollback retire les serveurs MCP du package en echec
- [ ] Tests : merge claude.json, tracking MCP, verification env vars

---

## Phase 5 : Installation Git (SSH, HTTPS, tags) + Detection backend

**User stories** : 1, 2, 3, 6, 7, 8

### What to build

Parsing de la source : distinguer chemin local vs Git SSH (`git@...`) vs Git HTTPS (`https://...`). Execution de `git clone --depth 1` dans un tempdir, avec support `--tag` / `--branch`. Apres clone, delegation au pipeline d'installation existant. Nettoyage du tempdir dans tous les cas.

Detection de backend : `ClaudeBackend::detect_installed()` verifie l'existence de `~/.claude/`. Avant installation, verification que les `backends` du package correspondent aux backends detectes. Erreur si incompatible, sauf avec `--force`.

Extraction du SHA du commit clone pour usage futur (lockfile).

### Acceptance criteria

- [ ] `rk install git@github.com:user/repo` clone et installe
- [ ] `rk install https://github.com/user/repo` clone et installe
- [ ] `rk install git@... --tag v1.0.0` clone le tag specifique
- [ ] Le tempdir est nettoye apres installation (succes ou echec)
- [ ] Un package avec `backends: ["cursor"]` sur une machine sans Cursor echoue avec message clair
- [ ] `--force` permet d'installer malgre l'incompatibilite backend
- [ ] Le SHA du commit est extrait et stocke
- [ ] Tests : parsing source (SSH, HTTPS, local), detection backend, compatibilite, force override

---

## Phase 6 : Gestion des conflits + Renommage interactif

**User stories** : 15, 16, 17, 18

### What to build

Avant chaque deploiement de skill, verification dans `install-cache.json` si un autre package possede deja un skill du meme nom.

Comportements selon le contexte :
- **TTY** : prompt interactif (`dialoguer`) pour choisir un nouveau nom
- **Non-TTY** : erreur avec exit code 1
- **`--force`** : ecrasement silencieux

Sur renommage : deployer sous le nouveau nom (`renkei-<nouveau>/SKILL.md`), mettre a jour le champ `name` dans le frontmatter du skill, persister le mapping `nom-original → nom-deploye` dans `install-cache.json`.

### Acceptance criteria

- [ ] Installation de 2 packages avec un skill du meme nom declenche la detection de conflit
- [ ] En mode TTY, le prompt propose un renommage et deploie sous le nouveau nom
- [ ] En mode non-TTY, erreur avec exit code 1
- [ ] Avec `--force`, le dernier installe ecrase le premier
- [ ] Le frontmatter du skill renomme contient le nouveau nom
- [ ] `install-cache.json` contient le mapping de renommage
- [ ] Tests : detection conflit, renommage frontmatter, mapping persistance

---

## Phase 7 : `rk list`

**User stories** : 19, 20

### What to build

Commande `rk list` : lecture de `install-cache.json`, affichage tabulaire de tous les packages installes avec nom, version, source, types d'artefacts.

Distinction visuelle entre sources Git (`[git]`) et locales (`[local]`). Gestion du cas vide ("No packages installed").

### Acceptance criteria

- [ ] `rk list` affiche tous les packages installes avec nom, version, source
- [ ] Les packages Git et locaux sont distingues visuellement
- [ ] Sans packages installes, message explicite
- [ ] Exit code 0 dans tous les cas
- [ ] Tests : formatage sortie, cas vide, sources mixtes

---

## Phase 8 : `rk doctor`

**User stories** : 21, 22, 23, 24, 25

### What to build

Commande `rk doctor` executant une serie de checks de sante :

1. Backends installes (dossier de config existe)
2. Fichiers deployes existent toujours
3. Variables d'environnement requises presentes
4. Skills modifies localement (hash SHA-256 vs archive en cache)
5. Hooks toujours presents dans `settings.json`
6. MCP configs toujours dans `~/.claude.json`

Sortie : checkmark/croix par check, groupes par package. Exit code 0 si tout sain, 1 si probleme.

### Acceptance criteria

- [ ] `rk doctor` sur un environnement sain retourne exit code 0
- [ ] Fichier deploye supprime → signale, exit code 1
- [ ] Skill modifie localement → signale la modification
- [ ] Variable d'environnement manquante → signale
- [ ] Hook manquant dans settings.json → signale
- [ ] MCP manquant dans claude.json → signale
- [ ] Tests : chaque check individuellement, exit codes, formatage

---

## Phase 9 : Lockfile

**User stories** : 30, 31, 32, 33, 34

### What to build

Apres chaque `rk install <source>`, generation/mise a jour de `rk.lock` dans le repertoire courant. Format JSON : `lockfileVersion: 1`, packages avec `version`, `source`, `tag` (optionnel), `resolved` (SHA commit), `integrity` (SHA-256 de l'archive).

`rk install` sans arguments : detection de `rk.lock` dans le cwd, lecture, reinstallation de chaque package depuis le cache ou re-clone au commit exact. Verification d'integrite : hash de l'archive cache vs hash du lockfile.

### Acceptance criteria

- [ ] `rk install <source>` genere/met a jour `rk.lock` dans le cwd
- [ ] Le lockfile contient version, source, tag, resolved (SHA), integrity (SHA-256)
- [ ] `rk install` (sans args) avec `rk.lock` installe les versions exactes
- [ ] Archive corrompue dans le cache → erreur d'integrite
- [ ] `rk install` sans args et sans `rk.lock` → erreur explicite
- [ ] Tests : serialisation/deserialisation lockfile, calcul SHA-256, round-trip install → lockfile → clean → install-from-lockfile, verification integrite

---

## Phase 10 : `rk package`

**User stories** : 26, 27, 28, 29

### What to build

Commande `rk package` executee depuis un dossier package : validation du manifeste, scan des dossiers conventionnes, creation d'une archive `<name>-<version>.tar.gz` contenant uniquement `renkei.json`, `skills/`, `hooks/`, `agents/`, `scripts/`.

Flag `--bump patch|minor|major` : increment de version dans `renkei.json` avant archivage, reecriture du manifeste.

Affichage resume : liste des fichiers inclus, nombre, taille de l'archive.

### Acceptance criteria

- [ ] `rk package` cree `<name>-<version>.tar.gz` avec le bon contenu
- [ ] L'archive exclut tout sauf `renkei.json`, `skills/`, `hooks/`, `agents/`, `scripts/`
- [ ] `rk package --bump minor` incremente la version minor dans `renkei.json`
- [ ] `rk package` dans un dossier sans `renkei.json` → erreur
- [ ] Resume affiche avec liste des fichiers et taille
- [ ] Tests : contenu archive, bump version (patch/minor/major), validation, resume

---

## Phase 11 : Workspace

**User stories** : support workspace (PRD section "Workspace")

### What to build

Detection de workspace : un `renkei.json` racine avec champ `workspace` listant les sous-dossiers membres. Chaque membre a son propre `renkei.json` et ses dossiers conventionnes.

`rk install ./workspace/` installe chaque membre independamment. Chaque membre est cache, deploye et tracke separement.

`rk install` sans arguments dans un contexte workspace sans `rk.lock` → erreur avec message guidant vers `rk install --link .`.

### Acceptance criteria

- [ ] `rk install ./workspace/` installe tous les membres listes dans le champ `workspace`
- [ ] Chaque membre apparait independamment dans `rk list`
- [ ] Chaque membre a sa propre entree dans le lockfile
- [ ] `rk install` sans args dans un workspace sans lockfile → erreur avec guidance
- [ ] Tests : detection workspace, enumeration membres, installation independante, message d'erreur

---

## Phase 12 : CI/CD + Migration

**User stories** : 35, 36

### What to build

GitHub Actions : workflow de release sur tag push — matrice de compilation croisee (Linux x86_64/aarch64, macOS x86_64/aarch64, Windows x86_64). Publication des binaires en GitHub Release.

Workflow CI : tests + clippy + fmt sur chaque PR.

Commande `rk migrate <path>` : scan d'une structure renkei-old existante, generation d'un `renkei.json` valide, reorganisation des fichiers dans les dossiers conventionnes.

### Acceptance criteria

- [ ] Workflow release produit des binaires pour les 5 targets
- [ ] Workflow CI execute tests, clippy, fmt
- [ ] `rk migrate` genere un `renkei.json` valide depuis l'ancien format
- [ ] Le package migre passe `rk package` sans erreur
- [ ] Tests : migration ancien format → nouveau format valide
