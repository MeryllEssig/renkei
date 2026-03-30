# Renkei — Package Manager

> CLI de distribution de workflows agentiques. Installe, versionne et partage des workflows pour outils IA (Claude Code, Cursor, ...).

---

## 1. Vision

Renkei est un **package manager pour workflows agentiques** — des ensembles coherents de skills, hooks, agents, configurations MCP et scripts qui resolvent un probleme concret dans un outil IA.

Un workflow n'est pas un skill isole. C'est un **package complet** : un skill qui orchestre, des hooks qui reagissent, des agents specialises, des configurations MCP, des scripts d'execution. Le package est l'unite de distribution.

L'utilisateur installe `rk`, pointe vers un repo Git ou un dossier local, et le workflow est deploye aux bons endroits, pret a l'emploi.

```
rk install git@github.com:meryll/mr-review
rk install ./mon-workflow/
rk list
rk doctor
```

---

## 2. Positionnement

### Ce que Renkei est

- Un **package manager CLI** ecrit en Rust
- Un outil de **distribution et deploiement** de workflows agentiques
- **Agnostique au contenu** des packages : skills, hooks, agents, scripts, configs MCP — dans n'importe quel langage
- **Open source** (la CLI)

### Ce que Renkei n'est pas

- Un runtime d'execution de workflows
- Un orchestrateur de MCPs
- Un framework de patterns agentiques
- Un outil d'observabilite

Renkei **sert** des workflows. Il ne les **execute** pas.

---

## 3. Architecture

### Le package

Un package Renkei est un dossier contenant un manifeste `renkei.json` et des artefacts.

```
mr-review/
├── renkei.json              # Manifeste (identite, artefacts, config)
├── skills/
│   └── mr-review/
│       └── SKILL.md         # Skill Claude Code / Cursor
├── hooks/
│   └── ReviewNotification.json
├── agents/
│   └── review-agent.md
├── scripts/
│   └── post-review.sh       # Scripts arbitraires
└── README.md
```

### Le manifeste `renkei.json`

```json
{
  "name": "@meryll/mr-review",
  "version": "1.2.0",
  "description": "Review a GitLab merge request with structured analysis",
  "author": "meryll",
  "license": "MIT",
  "keywords": ["gitlab", "review", "merge-request"],
  "artifacts": {
    "skills": ["skills/mr-review/SKILL.md"],
    "hooks": ["hooks/ReviewNotification.json"],
    "agents": ["agents/review-agent.md"]
  },
  "mcp": {
    "gitlab-mcp": {
      "command": "bun",
      "args": ["src/index.ts"],
      "env": {
        "GITLAB_URL": "${GITLAB_URL}",
        "GITLAB_API_KEY": "${GITLAB_API_KEY}"
      }
    }
  },
  "requiredEnv": [
    {
      "name": "GITLAB_API_KEY",
      "description": "GitLab personal access token with api scope"
    },
    {
      "name": "GITLAB_URL",
      "description": "GitLab instance URL"
    }
  ],
  "backends": ["claude", "cursor"]
}
```

#### Champs

| Champ | Type | Requis | Description |
|-------|------|--------|-------------|
| `name` | string | oui | Nom scope du package (`@scope/nom`) |
| `version` | string | oui | Version semver |
| `description` | string | oui | Description courte |
| `author` | string | oui | Auteur ou organisation |
| `license` | string | oui | Licence (SPDX identifier) |
| `keywords` | string[] | non | Tags pour la recherche |
| `artifacts` | object | oui | Fichiers a deployer par type |
| `artifacts.skills` | string[] | non | Chemins relatifs des skills |
| `artifacts.hooks` | string[] | non | Chemins relatifs des hooks |
| `artifacts.agents` | string[] | non | Chemins relatifs des agents |
| `mcp` | object | non | Configurations MCP a enregistrer |
| `requiredEnv` | object[] | non | Variables d'environnement requises |
| `backends` | string[] | oui | Backends supportes (`claude`, `cursor`, ...) |

### Deploiement — convention over config

Quand `rk install` deploie un package, chaque type d'artefact va a un emplacement predetermine selon le backend. L'utilisateur n'a rien a configurer.

| Artefact | Claude Code | Cursor |
|----------|-------------|--------|
| Skills | `~/.claude/skills/renkei-<name>/SKILL.md` | `.cursor/skills/<name>/` |
| Hooks | Merge dans `~/.claude/settings.json` | N/A |
| Agents | `~/.claude/agents/<name>.md` | N/A |
| MCP config | Merge dans `~/.claude.json` | Merge dans `.cursor/mcp.json` |

Le prefixe `renkei-` sur les skills cree un namespace clair et evite les collisions avec les skills natifs.

### Gestion des conflits

Un **cache de redirection** (fichier local) track ou chaque artefact a ete installe. En cas de conflit (deux packages avec un skill du meme nom) :

1. `rk` detecte la collision
2. Prompt interactif : l'utilisateur choisit de renommer le skill
3. Le renommage met a jour le `name` dans le frontmatter du skill
4. Le cache enregistre le mapping (nom original → nom deploye)

### Multi-backend

Une interface `Backend` definit les operations de deploiement :

```
trait Backend {
    fn name() -> &str;
    fn detect_installed() -> bool;
    fn deploy_skill(source: &Path, name: &str) -> Result<()>;
    fn deploy_hook(config: &HookConfig) -> Result<()>;
    fn deploy_agent(source: &Path, name: &str) -> Result<()>;
    fn register_mcp(name: &str, config: &McpConfig) -> Result<()>;
}
```

En v1, seul `ClaudeBackend` est implemente. L'interface est la pour permettre d'ajouter `CursorBackend` et d'autres sans refactorer.

---

## 4. CLI `rk`

### Langage et distribution

- **Langage** : Rust
- **Distribution** : binaires compiles pour Linux / macOS / Windows, publies en GitHub Releases
- **Zero dependance** : un seul fichier executable, rien a installer
- **Licence** : open source

### Commandes v1

| Commande | Description |
|----------|-------------|
| `rk install <source>` | Installe un package depuis une URL Git (SSH/HTTPS) ou un chemin local |
| `rk list` | Liste les packages installes avec leurs versions et sources |
| `rk doctor` | Diagnostic : deps manquantes, env vars absentes, artefacts modifies |
| `rk package` | Collecte les artefacts declares dans `renkei.json`, valide, cree l'archive |

#### `rk install`

```bash
# Depuis un repo Git (SSH)
rk install git@github.com:meryll/mr-review

# Depuis un repo Git (HTTPS)
rk install https://github.com/meryll/mr-review

# Version specifique (tag Git)
rk install git@github.com:meryll/mr-review --tag v1.2.0

# Depuis un chemin local
rk install ./mon-workflow/
rk install /chemin/absolu/vers/package/
```

Flow d'installation :

1. Clone le repo (ou copie le dossier local) dans un cache temporaire
2. Lit `renkei.json` → valide le manifeste
3. Detecte le backend installe (Claude Code, Cursor, ...)
4. Verifie la compatibilite (`backends` dans le manifeste)
5. Deploie chaque artefact a l'emplacement conventionnel
6. Enregistre les configurations MCP si declarees
7. Met a jour le cache de redirection + le lockfile
8. Affiche les variables d'environnement requises manquantes

#### `rk list`

```
$ rk list

Installed packages:
  @meryll/mr-review        1.2.0    git@github.com:meryll/mr-review (v1.2.0)
  @meryll/issue-solving     1.0.3    ./local/issue-solving
  @renkei/git-workflow      0.5.0    git@gitlab.com:acme-org/git-workflow (v0.5.0)
```

#### `rk doctor`

```
$ rk doctor

Checking packages...
  @meryll/mr-review@1.2.0          OK
  @meryll/issue-solving@1.0.3      OK
  @renkei/git-workflow@0.5.0       WARN  Skill modified locally

Checking environment...
  GITLAB_API_KEY                   OK
  GITLAB_URL                       OK
  REDMINE_API_KEY                  MISSING  Required by @meryll/issue-solving

Checking backends...
  Claude Code                      OK (detected)
  Cursor                           NOT FOUND

1 issue found.
```

#### `rk package`

```bash
rk package                     # Valide et cree l'archive
rk package --bump patch        # 1.2.0 → 1.2.1
rk package --bump minor        # 1.2.0 → 1.3.0
rk package --bump major        # 1.2.0 → 2.0.0
```

Flow :

1. Lit `renkei.json` dans le dossier courant
2. Valide que tous les fichiers declares dans `artifacts` existent
3. Bumpe la version si `--bump` est passe
4. Cree une archive `<name>-<version>.tar.gz`
5. Affiche un resume (fichiers inclus, taille, version)

### Commandes v2+ (avec registry)

| Commande | Description |
|----------|-------------|
| `rk publish` | Pousse l'archive vers le registry |
| `rk search <query>` | Recherche dans le registry |
| `rk install @scope/name` | Installation par nom (resolution via registry) |
| `rk update [pkg]` | Mise a jour vers la derniere version compatible |
| `rk uninstall <pkg>` | Supprime les artefacts deployes et nettoie le cache |
| `rk info <pkg>` | Details d'un package (description, deps, versions, auteur) |
| `rk init` | Scaffolding interactif d'un nouveau package |
| `rk diff <pkg>` | Diff entre les artefacts deployes et l'archive originale |
| `rk reset <pkg>` | Restaure les artefacts depuis l'archive originale |
| `rk fork <pkg> --scope <s>` | Coupe le lien, cree un nouveau package sous un autre scope |

---

## 5. Lockfile

Le lockfile `rk.lock` est genere automatiquement par `rk install` et **commitable dans le repo projet**. Il fige les versions exactes installees pour que tous les membres de l'equipe travaillent avec les memes versions.

```json
{
  "lockfileVersion": 1,
  "packages": {
    "@meryll/mr-review": {
      "version": "1.2.0",
      "source": "git@github.com:meryll/mr-review",
      "tag": "v1.2.0",
      "integrity": "sha256-abc123def456..."
    },
    "@meryll/issue-solving": {
      "version": "1.0.3",
      "source": "./local/issue-solving",
      "integrity": "sha256-789ghi012jkl..."
    }
  }
}
```

Un utilisateur qui clone le projet et lance `rk install` (sans arguments) lit le lockfile et installe les versions exactes declarees.

---

## 6. Registry

### v1 — Git-based (zero infra)

En v1, il n'y a pas de registry centralise. Les packages sont distribues via :

- **Repos Git** : un repo par package, versions = tags Git
- **Fichiers locaux** : dossiers ou archives `.tar.gz`
- **Partage manuel** : Slack, email, drive, whatever

C'est suffisant pour une equipe de 2-10 personnes.

### v2 — Registry centralise

Quand l'adoption le justifie :

1. **Index centralize** : un service HTTP qui mappe `@scope/name` → URL source + metadata
2. `rk publish` uploade l'archive + met a jour l'index
3. `rk search` / `rk install @scope/name` resolvent via l'index
4. Auth par token API (`rk login` / `rk logout`)

#### Scoped packages

```
@meryll/mr-review          # workflow de Meryll
@acme-corp/mr-review       # workflow different, meme nom, scope different
@renkei/git-workflow       # package officiel (scope reserve)
```

- Le scope `@renkei/` est reserve aux packages officiels
- Chaque utilisateur/organisation a son scope
- Enregistrement du scope a la premiere publication

### v3 — Site web public

- Frontend : browse, search, package pages, README rendu
- Stats d'installation, profils auteur
- **Closed source**

A ne construire que si l'ecosysteme le justifie.

---

## 7. Stockage local

```
~/.renkei/
├── cache/                           # Archives telechargees (immutables)
│   └── @meryll/
│       └── mr-review/
│           ├── 1.0.0.tar.gz
│           └── 1.2.0.tar.gz
├── install-cache.json               # Mapping : package → artefacts deployes
├── config.json                      # Config locale (registries, preferences)
└── rk.lock                          # Lockfile global (hors-projet)

# Artefacts deployes (mutables — le LLM et l'utilisateur les modifient) :
~/.claude/skills/renkei-*/SKILL.md
~/.claude/agents/*.md
~/.claude/settings.json              # Hooks merges
~/.claude.json                       # MCP configs mergees
```

Le cache est **par version, immutable**. Les artefacts deployes sont **mutables**. Le cache de redirection (`install-cache.json`) track le lien entre les deux.

---

## 8. Decisions architecturales

### Skills > MCPs

Les skills (markdown + scripts integres) deviennent la primitive dominante pour les outils IA :

- Moins de contexte consomme que les tool calls MCP
- Plus de marge de manoeuvre en engineering
- Portables (Claude Code et Cursor supportent les skills)
- Pas de serveur a maintenir

Les MCPs ne disparaissent pas — ils restent utiles pour les integrations lourdes ou les offres commerciales. Mais Renkei ne construit pas d'outillage specifique pour les MCPs. Un package peut **declarer** une configuration MCP dans `renkei.json`, et `rk install` l'enregistre. C'est tout.

### Convention over config

Les destinations de deploiement sont hardcodees par backend. Pas de champ "destination" dans le manifeste. Moins de surface d'erreur, moins de decisions pour le createur de package.

### Rust

- Binaire natif, zero dependance a l'installation
- Compilation croisee Linux / macOS / Windows
- Distribution via GitHub Releases
- Signal de serieux pour l'adoption

### Clean break

Le codebase actuel (renkei-old) reste comme reference. Le nouveau Renkei repart de zero en Rust. Les skills existants seront packagees en workflows Renkei une fois la CLI fonctionnelle.

### Pas de sur-ingenierie

- Pas de workflow engine / runtime
- Pas de pattern library
- Pas de compilateur workflow → skill
- Pas de systeme de dependances inter-workflows
- Pas d'observabilite

Chaque feature ajoutee doit repondre a un besoin concret et immediat.

---

## 9. Multi-backend

### Strategie

**Claude-first, others-compatible.**

Le denominateur commun entre les outils IA est : **skills (markdown + scripts) + configurations MCP**. Renkei deploie ces artefacts au bon endroit selon le backend detecte.

### Matrice de support

| Artefact | Claude Code | Cursor | Codex | Gemini |
|----------|-------------|--------|-------|--------|
| Skills | SKILL.md | Skills | AGENTS.md | ? |
| Hooks | settings.json events | N/A | N/A | N/A |
| Agents | agents/*.md | N/A | N/A | N/A |
| MCP config | ~/.claude.json | .cursor/mcp.json | ? | ? |

### Implementation

En v1, seul `ClaudeBackend` est implemente. L'interface `Backend` est definie pour permettre l'ajout de `CursorBackend` sans refactoring.

Un package declare ses backends supportes dans `renkei.json`. Si l'utilisateur est sur Cursor et que le package ne supporte que Claude, `rk install` previent et refuse (ou installe avec `--force`).

---

## 10. Plan d'evolution

### Phase 1 — CLI v1

- Initialiser le projet Rust (Cargo)
- Implementer le parsing de `renkei.json` (serde)
- Implementer `rk install` (git clone + deploiement)
- Implementer `rk list`
- Implementer `rk doctor`
- Implementer `rk package`
- Implementer le lockfile
- Implementer `ClaudeBackend`
- CI/CD : compilation croisee + publication GitHub Releases
- Migrer les workflows existants (renkei-old) en packages Renkei

### Phase 2 — Registry + commandes avancees

- Registry HTTP centralise (index + storage)
- `rk publish`, `rk search`, `rk install @scope/name`
- `rk update`, `rk uninstall`
- `rk init` (scaffolding interactif)
- `rk diff`, `rk reset`, `rk fork`
- `CursorBackend`
- Auth (`rk login` / `rk logout`)

### Phase 3 — Ecosystem

- Site web public (closed source)
- Stats, profils auteur, moderation
- Registries prives (scope entreprise)
- Self-update (`rk self-update`)

---

## 11. Risques

| Risque | Severite | Mitigation |
|--------|----------|------------|
| **Sur-ingenierie** | Haute | Scope minimal en v1. Chaque feature justifiee par un besoin concret. |
| **Adoption nulle** | Haute | Valider avec les premiers utilisateurs (early adopters) avant d'investir dans le registry. |
| **Evolution rapide des outils IA** | Moyenne | L'interface Backend isole du changement. Un seul point d'adaptation par outil. |
| **Skills format instable** | Moyenne | Surveiller les changelogs Claude Code / Cursor. Adapter rapidement. |
| **Rust learning curve** | Faible | Le scope de la CLI est bien defini. Pas de concurrence, pas de async complexe. |
| **Competition native** | Faible | Renkei est multi-outils et oriente workflow, pas composant. Complementaire aux stores natifs. |

---

## 12. Licensing

| Composant | Licence |
|-----------|---------|
| CLI `rk` | Open source |
| Site web registry | Closed source |
| Packages individuels | Au choix du createur |
| Scope `@renkei/` | Reserve aux packages officiels |
