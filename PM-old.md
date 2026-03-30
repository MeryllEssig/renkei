# Renkei — Product Management

> De "toolkit Claude Code pour ticket-to-MR" a "plateforme universelle de workflows agentiques avec package registry"

---

## 1. Vision

Renkei devient **la workbench de reference pour concevoir, composer, distribuer et deployer des workflows agentiques** — quel que soit l'outil IA sous-jacent (Claude Code, Cursor, Codex, Gemini), quel que soit le contexte (interne, client), et quel que soit le niveau technique de l'utilisateur qui l'utilise.

Le mot-cle est **workflow agentique** : une sequence orchestree d'actions (LLM, outils, humain) qui resout un probleme concret. Le workflow est l'unite fonctionnelle. Le package est l'unite de distribution.

Un utilisateur installe Renkei, choisit ses workflows, et peut :
- **Utiliser** les workflows existants (`rk install @renkei/mr-review`)
- **Composer** de nouveaux workflows a partir de patterns agentiques et de packages existants
- **Partager** ses workflows avec l'equipe ou la communaute (`rk publish`)
- **Deployer** ses workflows dans n'importe quel outil IA supportant MCP

---

## 2. Diagnostic de l'existant

### Ce qui est solide et qu'il faut garder

| Element | Pourquoi c'est bien |
|---------|-------------------|
| **MCP servers** (redmine, gitlab, project-registry) | MCP est le standard inter-outils. C'est la couche la plus portable. |
| **Project-registry concept** | Hub central de config declaratif (YAML). Brillant pour le multi-projet, multi-tracker. |
| **Credential injection pattern** | `api_key_env` par appel, jamais de secret en memoire LLM. Securise et portable. |
| **Architecture Skills → Agents → MCPs** | Separation des responsabilites claire : intent (skill), execution specialisee (agent), acces systeme (MCP). |
| **Installer workflow-based** | UX bien pensee : selectionner des workflows, pas des composants individuels. |
| **TypeScript/Bun runtime** | Rapide, type, un seul langage pour tout. Pas de build step. |
| **Anonymizer** | Library reutilisable, deux phases (regex + LLM NER), degradation gracieuse. |
| **Evaluation dans skill-creator** | Preuve que l'amelioration empirique est possible. A generaliser. |
| **Modularite gitlab-mcp** | Bonne separation (tools/, client.ts, formatters.ts). Modele a suivre. |

### Ce qui pose probleme

| Probleme | Impact | Severite |
|----------|--------|----------|
| **Couplage fort a Claude Code** | Skills (SKILL.md), agents (YAML frontmatter), hooks (events Claude) — rien n'est portable tel quel vers Cursor/Gemini/Codex | Critique |
| **Pas de definition abstraite de "workflow"** | Un workflow est encode dans un SKILL.md en langage naturel. Pas de structure formelle, pas de composition, pas de test automatise | Critique |
| **Pas de systeme de distribution** | Pas de package manager, pas de registry. Partager un workflow = copier des fichiers a la main | Critique |
| **Monolithisme de redmine-mcp** | 640 lignes dans un seul fichier, contrairement a gitlab-mcp qui est modulaire | Faible |
| **Pas de versioning** | Aucun mecanisme de version des composants. `./install.ts --all` ecrase tout sans diff ni changelog | Moyen |
| **Cache CSV fragile** | Pas d'echappement, pas de validation. Un pipe dans une donnee casse le mapping anonymizer | Moyen |
| **Pas d'observabilite** | Aucun log, trace, ou metrique sur l'execution des workflows. Impossible de debugger un workflow complexe | Eleve |
| **Registry.yaml non versionne dans le repo** | Pas de template, pas d'exemple. Un utilisateur ne sait pas comment configurer ses projets | Eleve |
| **Documentation README trop Claude-centric** | "MCP servers for driving a ticket-to-merge-request development workflow through Claude Code" — exclut d'emblee les autres outils | Moyen |

### Opportunites

- **MCP devient un standard de facto** : Claude, Cursor, VS Code, Zed, Windsurf supportent MCP. La couche MCP de Renkei est deja portable.
- **Explosion des outils IA coding** : Le marche est fragmente. Un framework agnostique a une vraie valeur.
- **Les patterns agentiques sont documentes** : Le livre de Gulli fournit un referentiel clair. Renkei peut etre l'implementation de reference.
- **Demande client croissante** : Les clients veulent "de l'IA dans leur workflow dev" mais ne savent pas par ou commencer.
- **Aucun package manager pour workflows agentiques n'existe** : L'espace est vierge. Premier arrive, premier servi.

### Menaces

- **Sur-ingenierie** : Abstraire pour 4 outils avant d'avoir valide le besoin reel. Le YAGNI est un risque reel.
- **Maintenance x4** : Chaque outil IA a son format, ses quirks, ses breaking changes. Supporter 4 backends, c'est 4x le travail d'integration.
- **Course aux features des outils IA** : Claude Code evolue vite (skills, agents, hooks changent). Gemini ADK aussi. Rester a jour est un travail permanent.
- **Le piege du DSL** : Creer un langage de definition de workflow custom est tentant mais dangereux (courbe d'apprentissage, maintenance, adoption).
- **Le piege du registry** : Construire une infra de distribution avant d'avoir du contenu a distribuer. L'ecosysteme npm n'a pas commence par le site web.

---

## 3. Objections et risques — soyons honnetes

### Objection 1 : "Compatible Claude + Gemini + Codex + Cursor"

**Realite** : Ces outils n'ont pas le meme niveau de support.

| Outil | MCP | Skills/Rules | Agents | Hooks | Maturite |
|-------|-----|-------------|--------|-------|----------|
| Claude Code | Natif | SKILL.md | agents/*.md | Oui (events) | Haute |
| Cursor | Oui | .cursor/rules/*.mdc | Non | Non | Moyenne |
| Codex (OpenAI) | Partiel | AGENTS.md | Non | Non | Faible |
| Gemini (ADK) | Via extensions | Config Python | Oui (ADK) | Non | Moyenne |

**Verdict** : Le denominateur commun realiste est **MCP + markdown d'instructions**. Les agents et hooks sont Claude-specifiques. Vouloir une parite totale est une illusion — mais on peut offrir un **degraded graceful** : tout marche sur Claude Code, l'essentiel marche sur Cursor/Codex, le minimum marche sur Gemini.

**Recommandation** : Strategie "Claude-first, others-compatible". MCP est le pont. Les skills sont compilees vers le format cible.

### Objection 2 : "N'importe quel utilisateur peut developper ses workflows"

**Realite** : Developper un workflow agentique necessite de comprendre le prompt engineering, le tool design, les patterns agentiques, et le framework Renkei.

**Verdict** : On peut abaisser la barriere, pas l'eliminer. Il faut :
1. Des **templates** de workflows pour les cas courants
2. Une **documentation** pedagogique (pas juste technique)
3. Un **workflow-creator** interactif qui guide la creation
4. Des **exemples commentes** de chaque pattern
5. Un **registry** ou trouver l'inspiration et reutiliser le travail des autres

### Objection 3 : "Implementer les 21 patterns du livre"

**Realite** : Certains patterns sont triviaux a implementer (Prompt Chaining, Tool Use), d'autres sont des sujets de recherche a part entiere (Learning and Adaptation, Exploration and Discovery).

**Verdict** : Prioriser par valeur pratique immediate. Voir le mapping detaille en section 9.

### Objection 4 : "Livraison client"

**Realite** : Livrer a un client implique packaging, isolation des credentials, audit trail, support, licensing.

**Verdict** : Il faut distinguer deux modes de livraison :
1. **Renkei-as-a-toolkit** : Le client installe Renkei et ses propres workflows. On livre le framework.
2. **Renkei-as-a-solution** : On livre des workflows pre-configures pour un besoin client specifique. On livre le produit.

Le mode 1 necessite de la documentation. Le mode 2 necessite du packaging. Le registry sert les deux.

### Objection 5 : "Un package registry, c'est pas overkill ?"

**Realite** : Un registry c'est de l'infra a maintenir, un protocole de publication, de la moderation, un site web. C'est un produit a part entiere.

**Verdict** : On peut y arriver par paliers :
1. D'abord `rk package` + `rk install` depuis un fichier local ou un repo Git (zero infra)
2. Ensuite un registry simple (S3 + index JSON) pour la distribution interne
3. Enfin un site web public si l'adoption le justifie

Le registry n'est pas le point de depart. C'est la destination. Le point de depart, c'est le format de package et la CLI.

---

## 4. Architecture cible

### Couches

```
┌──────────────────────────────────────────────────────────┐
│  Layer 6 : Package Registry                              │
│  (distribution, versioning, decouverte)                  │
├──────────────────────────────────────────────────────────┤
│  Layer 5 : Workflows utilisateur                         │
│  (issue-solving, mr-review, security-audit, ...)         │
├──────────────────────────────────────────────────────────┤
│  Layer 4 : Patterns agentiques                           │
│  (routing, reflection, parallelization, HITL, RAG, ...)  │
├──────────────────────────────────────────────────────────┤
│  Layer 3 : Primitives de workflow                        │
│  (state, branching, error recovery, approval gates, ...) │
├──────────────────────────────────────────────────────────┤
│  Layer 2 : Outils MCP                                    │
│  (redmine, gitlab, project-registry, anonymizer, ...)    │
├──────────────────────────────────────────────────────────┤
│  Layer 1 : Infrastructure                                │
│  (installer, registry, config, observability, runtime)   │
├──────────────────────────────────────────────────────────┤
│  Layer 0 : Adaptateurs AI Tool                           │
│  (Claude Code, Cursor, Codex, Gemini)                    │
└──────────────────────────────────────────────────────────┘
```

**Etat actuel** :
- Layer 0 : Claude Code uniquement
- Layer 1 : Installer + registry (basique)
- Layer 2 : 4 MCPs (solide)
- Layer 3 : Inexistant (encode dans les skills en langage naturel)
- Layer 4 : Implicite (skills implementent des patterns sans les nommer)
- Layer 5 : 13 skills, 2 agents (fonctionnel mais non composable)
- Layer 6 : Inexistant

---

## 5. Le Package Registry

### Concept fondamental

Un skill seul n'a aucune utilite — c'est un fragment d'un workflow. Un workflow, lui, est une **unite fonctionnelle complete** : il resout un probleme. C'est donc le workflow qui devient l'unite de distribution.

Le registry est un **ecosysteme de workflows agentiques installables**, a la maniere de npm pour les packages Node.

### Deux types de packages

1. **Workflow package** — l'unite principale. Contient : config MCP requise, skills, hooks, fragments CLAUDE.md, metadata. C'est ce qu'un utilisateur installe pour "faire quelque chose".

2. **MCP package** — un serveur MCP seul, publiable et versionne independamment. Dependance partagee entre workflows. Une seule instance tourne, meme si 5 workflows l'utilisent.

### Anatomie d'un workflow package

```
@meryll/mr-review/
├── renkei.package.yaml        # Manifeste (nom, version, description, auteur, deps)
├── workflow.ts                # Definition structuree du workflow
├── skills/                    # Skills Claude Code
├── hooks/                     # Hooks eventuels
├── mcp-config.yaml            # Config MCP requise (pas le code MCP !)
└── README.md
```

Les MCPs ne vivent pas *dans* le workflow. Le workflow declare ses **dependances MCP** (nom + version), et le systeme s'assure qu'elles sont installees globalement. Comme `peerDependencies` dans npm — le workflow dit "j'ai besoin de `@renkei/gitlab-mcp@^1.0`", mais ne l'embarque pas.

### Le manifeste `renkei.package.yaml`

```yaml
name: "@meryll/mr-review"
version: "1.2.0"
description: "Review a GitLab merge request with structured analysis"
author: "meryll"
license: "MIT"
keywords: ["gitlab", "review", "merge-request"]

# Ce que le package deploie
artifacts:
  skills:
    - skills/mr-review/SKILL.md
  hooks:
    - hooks/config/ReviewNotification.json
  claude_md_fragments:
    - fragments/review-conventions.md

# Dependances MCP (installees globalement, pas embarquees)
mcpDependencies:
  "@renkei/gitlab-mcp": "^1.0.0"
  "@renkei/project-registry-mcp": "^1.0.0"

# Dependances workflow (un workflow peut dependre d'un autre)
workflowDependencies:
  "@renkei/issue-fetching": "^1.0.0"

# Variables d'environnement requises (l'utilisateur doit les configurer)
requiredEnv:
  - name: GITLAB_API_KEY
    description: "GitLab personal access token with api scope"
  - name: GITLAB_URL
    description: "GitLab instance URL"

# Compatibilite backend
backends:
  claude: true        # Support complet (skills + agents + hooks)
  cursor: true        # Support partiel (rules seulement)
  codex: false        # Non supporte
  gemini: false       # Non supporte
```

### Scoped packages — resolution du nommage

Deux utilisateurs publient un package avec le meme nom ? **Scoped names** a la npm.

```
@meryll/mr-review          # workflow
@acme-corp/mr-review       # workflow different, meme nom, scope different
@renkei/gitlab-mcp         # MCP officiel (scope reserve)
@jean/jira-mcp             # MCP communautaire
```

Regles :
- Le scope `@renkei/` est reserve aux packages officiels maintenus par le projet
- Chaque utilisateur/organisation a son scope
- Un workflow peut dependre de `@renkei/gitlab-mcp` ou de `@jean/custom-mcp` — le systeme resout

### Architecture de stockage

```
~/.renkei/
├── packages/                          # Cache global (archives telechargees, immutable)
│   └── @meryll/
│       └── mr-review/
│           ├── 1.0.0.tar.gz          # Archive originale (pour reset)
│           └── 1.2.0.tar.gz
├── mcp/                               # MCPs installes (un seul par MCP, partage)
│   ├── gitlab-mcp/
│   └── redmine-mcp/
├── registry.yaml                      # Config locale (credentials, projets)
└── rk.lock                            # Lockfile — versions exactes installees

# Fichiers deployes (la ou le LLM travaille) — MUTABLES :
~/.claude/skills/renkei-*/SKILL.md     # Skills (Claude Code)
~/.claude/agents/*.md                  # Agents (Claude Code)
.cursor/rules/*.mdc                    # Rules (Cursor, project-scope)
```

Le cache est **par version, immutable**. Les fichiers deployes sont **mutables** (le LLM et l'utilisateur les modifient). `rk package` lit les fichiers deployes. `rk reset` restaure depuis le cache.

### Resolution des dependances MCP

Quand on fait `rk install @meryll/mr-review` :

1. Lit `renkei.package.yaml` → voit `mcpDependencies: ["@renkei/gitlab-mcp@^1.0"]`
2. Verifie si le MCP est deja installe dans `~/.renkei/mcp/`
3. Si oui et version compatible → rien a faire (pas de duplication)
4. Si non → le telecharge et l'installe
5. Si conflit de version → signale le conflit (comme npm avec peer deps)
6. Met a jour `rk.lock` avec les versions exactes resolues

**Un MCP = un processus unique**, peu importe combien de workflows l'utilisent. Le registre `~/.claude.json` ne contient qu'une seule entree par MCP.

### Lockfile `rk.lock`

Fige les versions exactes installees pour reproductibilite entre utilisateurs sur un meme projet.

```yaml
# rk.lock — genere automatiquement, ne pas editer
lockfileVersion: 1
packages:
  "@meryll/mr-review@1.2.0":
    resolved: "https://registry.renkei.dev/@meryll/mr-review/1.2.0.tar.gz"
    integrity: "sha256-abc123..."
    mcpDependencies:
      "@renkei/gitlab-mcp": "1.1.3"
      "@renkei/project-registry-mcp": "1.0.2"
  "@renkei/gitlab-mcp@1.1.3":
    resolved: "https://registry.renkei.dev/@renkei/gitlab-mcp/1.1.3.tar.gz"
    integrity: "sha256-def456..."
```

Commitable dans le repo projet pour que toute l'equipe ait les memes versions.

---

## 6. CLI `rk`

### Commandes

| Commande | Role |
|----------|------|
| `rk install <pkg>` | Telecharge + deploie les fichiers aux bons endroits. Resout les deps MCP. |
| `rk uninstall <pkg>` | Supprime les fichiers deployes + nettoie le cache si plus aucun workflow ne depend du MCP. |
| `rk update [pkg]` | Met a jour un ou tous les packages vers les dernieres versions compatibles. |
| `rk reset <pkg> [--file <path>]` | Restaure les fichiers deployes depuis le cache (archive originale). |
| `rk package [--bump patch\|minor\|major]` | Collecte les fichiers declares dans le manifeste, bumpe la version, cree l'archive. |
| `rk publish` | Pousse l'archive sur le registry. Le registry valide (version unique, manifeste coherent). |
| `rk fork <pkg> --scope <scope>` | Coupe le lien avec le package original, cree un nouveau package sous ton scope. |
| `rk search <query>` | Cherche dans le registry (full-text sur nom, description, tags). |
| `rk list` | Liste les packages installes avec leurs versions. |
| `rk info <pkg>` | Details d'un package (description, deps, versions, auteur). |
| `rk init` | Initialise un nouveau workflow dans le dossier courant (scaffolding interactif). |
| `rk diff <pkg>` | Affiche les differences entre les fichiers deployes et l'archive originale. |
| `rk doctor` | Verifie l'integrite de l'installation (deps manquantes, versions incoherentes, env vars). |

### `rk package` — le cycle de publication

Les fichiers installes (skills, configs MCP) vivent dans des dossiers que le LLM peut modifier. L'utilisateur itere, ameliore, et veut remonter ces changements dans un package publiable.

```bash
rk package                 # Collecte, valide, cree l'archive sans bumper
rk package --bump patch    # 0.0.1 → 0.0.2
rk package --bump minor    # 0.0.1 → 0.1.0
rk package --bump major    # 0.0.1 → 1.0.0
```

Le flow :
1. `rk install @meryll/mr-review` → copie dans `~/.renkei/packages/` + deploie les fichiers
2. L'utilisateur (ou le LLM) modifie les fichiers deployes
3. `rk diff @meryll/mr-review` → visualise les modifications
4. `rk package --bump minor` → lit le manifeste, collecte les fichiers **depuis leur emplacement deploye**, cree l'archive versionnee
5. `rk publish` → pousse sur le registry (qui valide : version unique, manifeste coherent, etc.)

Pas de cache intermediaire. Le manifeste est la source de verite sur la composition. Les fichiers deployes sont la source de verite sur le contenu. Le registry est le garde-fou.

### `rk fork` — creer sa propre version

Use case : tu installes `@jean/security-audit`, tu l'adaptes tellement que ce n'est plus le meme workflow. Tu veux couper le lien et en faire ton propre package.

```bash
rk fork @jean/security-audit --scope @meryll
```

1. Copie le manifeste, change le scope → `@meryll/security-audit`
2. Reset la version a `0.1.0`
3. Ajoute un champ `forkedFrom: "@jean/security-audit@1.2.0"` dans le manifeste (tracabilite)
4. Les fichiers deployes restent en place — c'est maintenant ton package

Le terme `fork` est universel — tout developpeur comprend immediatement.

### `rk reset` — annuler les modifications locales

```bash
rk reset @meryll/mr-review                                    # Restaure tout
rk reset @meryll/mr-review --file skills/mr-review/SKILL.md   # Un fichier specifique
```

Restaure les fichiers deployes depuis l'archive originale dans le cache. Indispensable quand le LLM fait une modification non voulue sur un skill.

### `rk doctor` — diagnostic de sante

```bash
$ rk doctor

Checking installation...
  @renkei/mr-review@1.2.0        OK
  @renkei/issue-solving@1.0.3    OK
  @renkei/gitlab-mcp@1.1.3       OK
  @renkei/redmine-mcp@1.0.0      WARN  Modified locally (rk diff to see changes)

Checking dependencies...
  @renkei/project-registry-mcp   MISSING  Required by @renkei/mr-review

Checking environment...
  GITLAB_API_KEY                  OK
  GITLAB_URL                     OK
  REDMINE_API_KEY                MISSING  Required by @renkei/redmine-mcp

2 issues found. Run rk doctor --fix for suggestions.
```

---

## 7. Le site registry

Paliers de deploiement :

### Palier 0 — Local / Git (zero infra)

```bash
rk package --bump minor                    # Cree l'archive localement
rk install ./my-workflow-0.1.0.tar.gz      # Install depuis un fichier local
rk install git@github.com:org/workflows    # Install depuis un repo Git
```

Suffisant pour une equipe de 2-5 personnes. Le "registry" est un repo Git avec des archives.

### Palier 1 — Registry simple (infra minimale)

- Backend : bucket S3/R2 + fichier `index.json` regenere a chaque publish
- `rk publish` uploade l'archive + met a jour l'index
- `rk search` / `rk install` lisent l'index et telecharge depuis le bucket
- Auth : token API simple (genere par `rk login`)

Suffisant pour une equipe de 5-50 personnes.

### Palier 2 — Site web public

- Frontend : Cloudflare Pages (statique) — browse, search, package pages
- Backend : Cloudflare Workers + D1 (SQLite edge) — API publish/search/download
- Storage : R2 pour les archives
- Features : README rendu, stats d'installation, deps visualisees, profils auteur

Pour l'adoption publique. A ne construire que si l'ecosysteme le justifie.

---

## 8. Reorganisation du repo

```
renkei/
├── cli/                         # NOUVEAU : la CLI `rk`
│   ├── package.json
│   ├── src/
│   │   ├── index.ts             # Point d'entree CLI
│   │   ├── commands/            # Commandes (install, publish, fork, reset, etc.)
│   │   ├── registry/            # Client registry (fetch, upload, resolve deps)
│   │   ├── backends/            # Adaptateurs par outil IA
│   │   │   ├── claude.ts        # Claude Code (skills, agents, hooks, mcp)
│   │   │   ├── cursor.ts        # Cursor (rules, mcp)
│   │   │   └── types.ts         # Interface commune Backend
│   │   └── utils/               # Helpers (fs, semver, archive, etc.)
│   └── tsconfig.json
├── mcp/                         # Serveurs MCP (INCHANGE — deja portable)
│   ├── redmine-mcp/
│   ├── gitlab-mcp/
│   ├── project-registry-mcp/
│   └── anonymizer/
├── workflows/                   # NOUVEAU : definitions de workflows
│   ├── issue-solving/
│   │   ├── renkei.package.yaml  # Manifeste
│   │   ├── workflow.ts          # Definition structuree
│   │   ├── skills/              # Artefacts Claude Code
│   │   ├── tests/               # Tests du workflow
│   │   └── README.md
│   ├── mr-review/
│   ├── security-audit/
│   └── _templates/              # Templates pour `rk init`
├── patterns/                    # NOUVEAU : implementations des patterns agentiques
│   ├── routing.ts
│   ├── reflection.ts
│   ├── parallelization.ts
│   ├── hitl.ts
│   └── ...
├── tools/                       # CLI tools annexes (inchange)
├── hooks/                       # Hooks (inchange, integres dans les workflow packages)
├── templates/                   # NOUVEAU : templates de config
│   └── registry.example.yaml
├── doc/
│   ├── INSTALL.md
│   ├── GETTING_STARTED.md       # NOUVEAU : onboarding utilisateurs
│   ├── PATTERNS_GUIDE.md        # NOUVEAU : guide pratique des patterns
│   └── CLIENT_DELIVERY.md       # NOUVEAU : guide de livraison client
├── CLAUDE.md
├── PATTERNS.md
├── PM.md                        # Ce fichier
└── FUTURE_CONSIDERATIONS.md
```

**Principes de reorganisation** :
1. `mcp/` reste la couche universelle — ne change pas
2. `workflows/` est la nouvelle couche abstraite (remplace le role de `skills/` comme source de verite)
3. `skills/`, `agents/`, `rules/` deviennent des **artefacts de workflows** — generes/contenus dans chaque workflow package
4. `patterns/` fournit des building blocks reutilisables
5. `cli/` remplace `installer/` — la CLI `rk` subsume l'installateur actuel
6. `templates/` aide les utilisateurs a demarrer

**Migration** : L'installer actuel (`install.ts` + `installer/`) reste fonctionnel pendant la transition. La CLI `rk` le remplace progressivement. Les skills existants dans `skills/` sont migres dans des workflow packages dans `workflows/`.

---

## 9. Mapping des 21 patterns agentiques

Pour chaque pattern du livre, ou en est Renkei et que faut-il faire :

| # | Pattern | Etat actuel | Priorite | Action |
|---|---------|------------|----------|--------|
| 1 | **Prompt Chaining** | Implicite dans les skills (issue-solving chaine plusieurs etapes) | P1 | Formaliser comme pattern reutilisable dans `patterns/chaining.ts` |
| 2 | **Routing** | project-registry fait du routing basique (resolve_project) | P1 | Generaliser : routing LLM-based, embedding-based, rule-based |
| 3 | **Parallelization** | skill-creator lance des subagents en parallele | P1 | Extraire comme pattern : `parallel(tasks[], aggregator)` |
| 4 | **Reflection** | mr-review fait de la reflexion implicite | P1 | Formaliser : `reflect(generator, evaluator, maxIter)` |
| 5 | **Tool Use** | MCP est exactement ce pattern | FAIT | Deja implemente via MCP. Rien a faire. |
| 6 | **Planning** | issue-solving fait du planning | P2 | Extraire : `plan(goal, constraints) → steps[]` |
| 7 | **Multi-Agent** | 2 agents specialises (redmine, gitlab) en topologie "supervisor" | P2 | Supporter d'autres topologies (hierarchical, network) |
| 8 | **Memory Management** | Claude auto-memory + registry.yaml | P2 | Ajouter une memoire de workflow (persistance inter-etapes et inter-sessions) |
| 9 | **Learning and Adaptation** | Non implemente | P3 | Feedback loop : les evaluations alimentent l'amelioration des workflows |
| 10 | **MCP** | Coeur de l'architecture | FAIT | Deja le fondement. Continuer a enrichir les MCPs. |
| 11 | **Goal Setting and Monitoring** | Non implemente | P3 | Progress tracking pour les workflows longs (% completion, ETA) |
| 12 | **Exception Handling** | Basique dans les MCPs (handleError) | P2 | Workflow-level : retry, fallback, degradation, rollback |
| 13 | **HITL** | Hooks fournissent du feedback audio | P2 | Approval gates : pause le workflow, demande confirmation humaine |
| 14 | **Knowledge Retrieval (RAG)** | Schema dans registry.yaml mais non implemente | P2 | MCP pour base vectorielle + integration dans les workflows |
| 15 | **Inter-Agent Communication (A2A)** | Non implemente | P3 | Protocole pour que les workflows/agents communiquent |
| 16 | **Resource-Aware Optimization** | redmine-agent utilise haiku (choix de modele) | P2 | Systematiser : router vers le modele optimal selon la tache |
| 17 | **Reasoning Techniques** | ultrathink (vague) | P3 | CoT, Tree-of-Thought, ReAct comme patterns composables |
| 18 | **Guardrails** | Regles dans CLAUDE.md (pas d'ops destructives) | P2 | Input/output validation systematique, content filtering |
| 19 | **Evaluation and Monitoring** | skill-creator a des evals | P1 | Generaliser : benchmark tout workflow, regression testing, dashboard |
| 20 | **Prioritization** | Non implemente | P3 | Task ranking dynamique dans les workflows multi-objectifs |
| 21 | **Exploration and Discovery** | Non implemente | P3 | Research workflows autonomes (generation d'hypotheses, experimentation) |

**Resume** :
- **P1 (a formaliser)** : Chaining, Routing, Parallelization, Reflection, Evaluation — 5 patterns
- **P2 (valeur immediate)** : Planning, Multi-Agent, Memory, Exception Handling, HITL, RAG, Resource-Aware, Guardrails — 8 patterns
- **P3 (avance/recherche)** : Learning, Goal Monitoring, A2A, Reasoning, Prioritization, Exploration — 6 patterns
- **FAIT** : Tool Use, MCP — 2 patterns

---

## 10. Plan d'evolution

### Phase 1 — Fondations

**Objectif** : Poser les bases sans casser l'existant.

#### 1.1 CLI `rk` minimale

Creer `cli/` avec les commandes de base :
- `rk install` (depuis fichier local ou repo Git — pas besoin du registry tout de suite)
- `rk list`
- `rk reset`
- `rk package`
- `rk diff`
- `rk doctor`

L'installer actuel (`install.ts`) continue de fonctionner. `rk` le wrappe et le remplace progressivement.

#### 1.2 Format de package

Definir `renkei.package.yaml` (manifeste). Migrer les 13 skills et 2 agents existants en workflow packages dans `workflows/`. Ce sont les premiers packages `@renkei/*`.

#### 1.3 Abstraction du backend installer

Creer une interface `Backend` dans `cli/src/backends/types.ts` :

```typescript
interface Backend {
  name: string;
  detectInstalled(): boolean;
  deploySkill(source: string, name: string): void;
  deployAgent(source: string, name: string): void;
  registerMcp(name: string, command: string, args: string[], env: Record<string, string>): void;
  deployHook?(config: HookConfig): void;
}
```

Implementer `ClaudeBackend` en extrayant la logique actuelle de `configure.ts`. Ajouter `CursorBackend` comme premier backend alternatif.

#### 1.4 Registry template et validation

- Creer `templates/registry.example.yaml` avec des commentaires explicatifs
- Ajouter `rk init --registry` qui copie le template vers `~/.renkei/registry.yaml`
- Rendre `validate_registry` plus explicite (messages d'erreur actionnables)

#### 1.5 Modulariser redmine-mcp

Aligner sur le modele de gitlab-mcp : `src/tools/`, `src/client.ts`, `src/formatters.ts`.

#### 1.6 Observabilite minimale

Chaque tool call MCP logge dans `~/.renkei/logs/YYYY-MM-DD.jsonl` :
```json
{ "ts": "...", "mcp": "redmine", "tool": "get_ticket", "params": { "ticket_id": 123 }, "duration_ms": 450, "status": "ok" }
```

#### 1.7 Documentation onboarding

Creer `doc/GETTING_STARTED.md` : guide pas-a-pas pour un utilisateur qui decouvre Renkei.

---

### Phase 2 — Workflow Engine

**Objectif** : Permettre de definir des workflows abstraits et de les compiler vers les formats cibles.

#### 2.1 Le dossier `workflows/`

Chaque workflow a :
- `renkei.package.yaml` : manifeste
- `workflow.ts` : definition structuree
- `skills/` : artefacts generes
- `tests/` : tests automatises (mock des MCP tools)
- `README.md` : documentation

#### 2.2 Compilateur workflow → skill/rule

Un `rk compile` qui :
1. Lit les definitions de `workflows/`
2. Genere les `skills/<name>/SKILL.md` pour Claude Code
3. Genere les `rules/<name>.mdc` pour Cursor
4. Genere les sections pertinentes pour Codex/Gemini

#### 2.3 Pattern library

Implementer les patterns P1 comme modules reutilisables dans `patterns/` :
- `tool-use` — Appel d'outil MCP avec gestion d'erreur
- `routing` — Branchement conditionnel
- `parallelization` — Execution parallele + aggregation
- `reflection` — Boucle generate → evaluate → refine
- `planning` — Decomposition d'objectif

#### 2.4 Tests de workflow

Framework de test qui :
- Mock les appels MCP
- Verifie que les steps s'executent dans l'ordre
- Valide les outputs contre un schema attendu
- Mesure le nombre de tokens consommes (estimation)

#### 2.5 Evaluation framework

Generaliser le systeme d'evaluation de skill-creator a tous les workflows :
- Benchmark automatise (execution sur des cas de test)
- Metriques : precision, recall, tokens, latence, cout
- Regression testing : comparer versions du workflow
- Dashboard simple (HTML statique ou terminal)

---

### Phase 3 — Ecosystem

**Objectif** : Rendre Renkei auto-suffisant et partageable.

#### 3.1 Registry Palier 1

- `rk publish` → upload vers un bucket S3/R2
- `rk search` / `rk install <pkg>` → resolution depuis le registry distant
- Auth par token API
- `rk login` / `rk logout`

#### 3.2 Commandes avancees

- `rk fork <pkg> --scope <scope>` — couper le lien, creer sa version
- `rk update` — mise a jour intelligente avec respect du lockfile
- `rk audit` — verifier les vulnerabilites des deps

#### 3.3 Nouveaux MCPs

En fonction des besoins :
- `jira-mcp` — Pour les clients Jira
- `asana-mcp` — Pour les clients Asana
- `confluence-mcp` / `notion-mcp` — Pour le knowledge retrieval (RAG)
- `slack-mcp` — Pour les notifications et l'inter-agent communication

#### 3.4 Patterns avances (P2/P3)

- `hitl` — Gate d'approbation humaine
- `exception-handling` — Retry, fallback, degradation
- `memory` — Persistance inter-sessions
- `rag` — Enrichissement par base de connaissances
- `multi-agent` — Topologies de coordination
- `resource-aware` — Routage vers le modele optimal

#### 3.5 Site web registry (Palier 2)

Si l'adoption le justifie. Cloudflare Pages + Workers + D1 + R2.

---

## 11. Ce a quoi tu n'as pas pense

### 11.1 Distribution et mise a jour de Renkei lui-meme

Comment un utilisateur obtient Renkei ? Aujourd'hui : `git clone` + `./install.ts`. Mais il faut Bun, acces Git, mises a jour manuelles.

**Proposition** : Un script d'installation one-liner qui installe Bun si necessaire, clone le repo, et lance l'installer. Pour les mises a jour : `rk self-update`.

### 11.2 Configuration en couches

Aujourd'hui `registry.yaml` est unique et global. En contexte equipe/client :

```
~/.renkei/registry.yaml          # Config personnelle (mes credentials)
<projet>/.renkei/registry.yaml   # Config projet (partagee via Git, sans secrets)
```

Le project-registry-mcp merge ces couches (projet override global). Permet de committer la config projet sans exposer les credentials.

### 11.3 Dry run / preview mode

Avant d'executer un workflow qui modifie des systemes externes, previsualiser :

```
[DRY RUN] Would post draft note on MR !42:
  File: src/auth.ts:15
  Comment: "Missing null check on user.token"
```

Un flag `--dry-run` dans les MCPs qui retourne l'action prevue sans l'executer.

### 11.4 Rate limiting intelligent

Les APIs GitLab/Redmine ont des rate limits. Un rate limiter dans le client HTTP, configurable dans `registry.yaml` :

```yaml
gitlab:
  production:
    rate_limit: 10/s
    retry_after: true
```

### 11.5 Couts et budget

Les workflows multi-agent consomment des tokens. Un workflow mal ecrit peut couter cher.

- Tracker les tokens par workflow (via l'observabilite)
- Alerter quand un workflow depasse un seuil configurable
- Le pattern `resource-aware-optimization` route vers des modeles moins chers
- Dashboard : "Cette semaine, mr-review a consomme ~$2.40 sur 15 executions"

### 11.6 Composabilite des workflows

Un workflow devrait pouvoir appeler un autre workflow comme sous-etape :

```typescript
steps: [
  { id: "fetch", workflow: "@renkei/issue-fetching", params: { ticket: "$input.ticket" } },
  { id: "branch", workflow: "@renkei/branching", params: { name: "$fetch.result.title" } },
  { id: "solve", pattern: "planning", ... },
]
```

Cela evite la duplication et permet de construire des workflows complexes par composition. Le systeme de deps du manifeste (`workflowDependencies`) le rend possible.

### 11.7 Mode offline / air-gappe

Certains clients n'ont pas internet (defense, sante, finance). Renkei doit pouvoir fonctionner :
- Sans acces au registry distant → `rk install` depuis fichier local / repo Git
- Sans npm/bun registry → pre-bundler les node_modules
- Sans LLM cloud → support modeles locaux (Ollama/llama.cpp)
- Sans MCP distant → tout local

L'anonymizer supporte deja un LLM local — le pattern est la.

Un `rk pack` qui cree une archive auto-contenue installable sans reseau.

### 11.8 Internationalisation

Les skills actuels repondent en francais (hardcode). Pour les clients internationaux :
- Le language de sortie configurable dans `registry.yaml` ou par workflow
- Les templates de commit, MR, review localisables

### 11.9 Le naming "Renkei"

"Renkei" (連携) signifie "coordination, collaboration" en japonais — parfait pour un orchestrateur de workflows. Mais :
- Le README dit "MCP servers for driving [...] through Claude Code" — trop reducteur
- Le branding devrait refleter la vision : "Agentic workflow platform"
- Le prefixe `renkei-` sur les skills est bien — il cree une identite
- La CLI `rk` est courte, memorable, tape vite

### 11.10 Governance multi-utilisateur

Quand plusieurs personnes contribuent des workflows :
- Qui review les workflows avant publication ? → Process de PR classique pour `@renkei/*`
- Comment gerer les conflits de version ? → Semver strict + lockfile
- Comment deprecier un workflow obsolete ? → Champ `deprecated` dans le manifeste + warning a l'install
- Comment signaler un package malveillant ? → Moderation sur le registry (report + remove)

### 11.11 Security model pour les clients

- `registry.yaml` par projet (pas global) pour les deployments client
- Les env vars de credentials sont scopes par projet dans le shell
- Les logs d'observabilite fournissent l'audit trail
- Le mode `--project` de l'installer isole les skills/agents par projet
- `rk audit` verifie qu'aucun package n'accede a des scopes non declares

### 11.12 Versioning semantique des workflows

Un workflow qui change ses triggers ou ses outputs casse les habitudes utilisateur. Semver :
- **Patch** : fix interne, meme comportement observable
- **Minor** : nouveau trigger, nouvelle option, retro-compatible
- **Major** : changement de triggers, de structure d'output, de deps MCP

### 11.13 Workflows prives (scope entreprise)

Pour les clients : un registry prive (ou un scope prive sur le registry public) pour publier des workflows internes sans les exposer. Comme les registries npm prives.

```bash
rk config set registry https://registry.acme-corp.internal
rk publish    # Publie sur le registry prive
```

---

## 12. Decisions a prendre

### Decision 1 : Workflow format

**TypeScript-only** vs. **YAML + TypeScript** (hybride) ?

→ Impact tout le reste. Si TypeScript-only, les utilisateurs non-dev sont exclus. Si hybride, deux parsers a maintenir.

Recommandation : TypeScript pour les workflows internes, generateur interactif (`rk init`) pour que les non-TypeScript puissent creer des workflows via conversation.

### Decision 2 : Profondeur du multi-outil

**Claude-first + Cursor** (pragmatique) vs. **4 outils des le depart** (ambitieux) ?

→ Recommandation : Claude-first + Cursor. Codex et Gemini quand le besoin se materialise.

### Decision 3 : Compilation vs. interpretation

Les workflows sont-ils **compiles** en skills/rules (statique) ou **interpretes** a l'execution (dynamique) ?

→ Compilation par defaut, interpretation opt-in pour les patterns dynamiques.

### Decision 4 : Palier de registry initial

**Fichiers locaux + Git** (zero infra) vs. **Registry S3** (infra minimale) ?

→ Commencer par local + Git. Le registry vient en Phase 3.

### Decision 5 : Open source ?

- **Private** : Repo interne, acces par invitation.
- **Open-core** : Framework open source, workflows metier prives.
- **Full open source** : Tout public.

→ Decision strategique, pas technique. Mais elle impacte la distribution et l'adoption.

### Decision 6 : Retrocompatibilite

Les skills actuels continuent-ils de fonctionner "as-is" pendant la migration ?

→ Oui, obligatoirement. Pas de big bang. Migration progressive.

### Decision 7 : Nom de la CLI

`rk` est court et memorable. Mais `renkei` en entier est plus explicite pour les nouveaux utilisateurs.

→ Recommandation : `rk` comme alias court, `renkei` comme commande longue. Les deux fonctionnent.

---

## 13. Synthese

Renkei a une base solide : des MCPs bien concus, un installer ergonomique, des skills qui encodent de vrais workflows utiles, et une architecture de securite (credentials) exemplaire.

Le gap entre l'etat actuel et la vision est significatif mais franchissable. Les quatre mouvements cles :

1. **Abstraire** : Separer la definition du workflow de sa materialisation (skill Claude, rule Cursor, etc.)
2. **Formaliser** : Transformer les patterns agentiques implicites en building blocks explicites et composables
3. **Distribuer** : Le package registry fait de chaque workflow une unite installable, versionnable, partageable
4. **Outiller** : La CLI `rk` donne aux utilisateurs les moyens de creer, tester, publier et partager leurs workflows

Le risque principal est la sur-ingenierie. La regle d'or : **chaque abstraction doit etre justifiee par au moins deux cas d'usage concrets**. Si un pattern n'a qu'un seul usage, il reste inline dans le workflow. Si le registry n'a que 3 packages, un repo Git suffit.

Renkei ne doit pas devenir un framework theorique parfait que personne n'utilise. Il doit rester ce qu'il est : un outil pragmatique qui resout des problemes reels — mais avec une architecture qui lui permet de grandir.
