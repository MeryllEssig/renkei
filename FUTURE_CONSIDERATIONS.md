# Renkei — Plan strategique

> De "toolkit Claude Code pour ticket-to-MR" a "plateforme universelle de workflows agentiques"

---

## 1. Vision

Renkei devient **la workbench de reference pour concevoir, composer et deployer des workflows agentiques** — quel que soit l'outil IA sous-jacent (Claude Code, Cursor, Codex, Gemini), quel que soit le contexte (interne, client), et quel que soit le niveau technique de l'utilisateur qui l'utilise.

Un utilisateur installe Renkei, choisit ses workflows, et peut :
- **Utiliser** les workflows existants (issue-solving, mr-review, etc.)
- **Composer** de nouveaux workflows a partir de patterns agentiques referencees
- **Deployer** ses workflows dans n'importe quel outil IA supportant MCP

Le mot-cle est **workflow agentique** : une sequence orchestree d'actions (LLM, outils, humain) qui resout un probleme concret.

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

### Menaces

- **Sur-ingenierie** : Abstraire pour 4 outils avant d'avoir valide le besoin reel. Le YAGNI est un risque reel.
- **Maintenance x4** : Chaque outil IA a son format, ses quirks, ses breaking changes. Supporter 4 backends, c'est 4x le travail d'integration.
- **Course aux features des outils IA** : Claude Code evolue vite (skills, agents, hooks changent). Gemini ADK aussi. Rester a jour est un travail permanent.
- **Le piege du DSL** : Creer un langage de definition de workflow custom est tentant mais dangereux (courbe d'apprentissage, maintenance, adoption).

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

**Realite** : Developper un workflow agentique necessite de comprendre :
- Le prompt engineering (comment instruire un LLM)
- Le tool design (quoi exposer, quoi cacher)
- Les patterns agentiques (quand utiliser reflection vs. routing vs. parallelization)
- Le framework Renkei (conventions, installer, registry)

**Verdict** : On peut abaisser la barriere, pas l'eliminer. Il faut :
1. Des **templates** de workflows pour les cas courants
2. Une **documentation** pedagogique (pas juste technique)
3. Un **skill-creator ameliore** qui guide la creation
4. Des **exemples commentes** de chaque pattern

### Objection 3 : "Implementer les 21 patterns du livre"

**Realite** : Certains patterns sont triviaux a implementer (Prompt Chaining, Tool Use), d'autres sont des sujets de recherche a part entiere (Learning and Adaptation, Exploration and Discovery).

**Verdict** : Prioriser par valeur pratique immediate. Voir le mapping detaille en section 8.

### Objection 4 : "Livraison client"

**Realite** : Livrer a un client implique :
- Packaging (comment distribuer ? npm ? archive ?)
- Isolation des credentials (pas de fuite entre projets client)
- Audit trail (qui a fait quoi, quand)
- Support (documentation, onboarding, maintenance)
- Licensing (le code est "Private" — quelle licence pour le client ?)

**Verdict** : Il faut distinguer deux modes de livraison :
1. **Renkei-as-a-toolkit** : Le client installe Renkei et ses propres workflows. On livre le framework.
2. **Renkei-as-a-solution** : On livre des workflows pre-configures pour un besoin client specifique. On livre le produit.

Le mode 1 necessite de la documentation. Le mode 2 necessite du packaging.

---

## 4. Architecture cible

### Couches

```
┌──────────────────────────────────────────────────────────┐
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

**Objectif** : Remplir les couches manquantes, generaliser ce qui existe.

### Reorganisation du repo

```
renkei/
├── install.ts                    # Point d'entree (inchange)
├── installer/                    # Systeme d'installation
│   ├── workflows.ts              # Config centrale
│   ├── backends/                 # NOUVEAU : adaptateurs par outil IA
│   │   ├── claude.ts             # Claude Code (skills, agents, hooks, mcp)
│   │   ├── cursor.ts             # Cursor (rules, mcp)
│   │   ├── codex.ts              # Codex (AGENTS.md, mcp)
│   │   └── types.ts              # Interface commune Backend
│   └── ...
├── mcp/                          # Serveurs MCP (INCHANGE — deja portable)
│   ├── redmine-mcp/
│   ├── gitlab-mcp/
│   ├── project-registry-mcp/
│   └── anonymizer/
├── workflows/                    # NOUVEAU : definitions de workflows
│   ├── issue-solving/
│   │   ├── workflow.ts           # Definition structuree du workflow
│   │   ├── steps/                # Etapes individuelles
│   │   └── tests/                # Tests du workflow
│   ├── mr-review/
│   ├── security-audit/           # Exemple de nouveau workflow
│   └── _templates/               # Templates pour creer un workflow
├── patterns/                     # NOUVEAU : implementations des patterns agentiques
│   ├── routing.ts                # Pattern: routing conditionnel
│   ├── reflection.ts             # Pattern: boucle generate-evaluate-refine
│   ├── parallelization.ts        # Pattern: execution parallele + aggregation
│   ├── hitl.ts                   # Pattern: human-in-the-loop gates
│   └── ...
├── skills/                       # CLAUDE-SPECIFIQUE : compilation des workflows en SKILL.md
├── agents/                       # CLAUDE-SPECIFIQUE : compilation en agents Claude
├── rules/                        # NOUVEAU : compilation pour Cursor (.mdc rules)
├── tools/                        # CLI tools (inchange)
├── hooks/                        # CLAUDE-SPECIFIQUE (inchange)
├── templates/                    # NOUVEAU : registry.yaml, workflow templates
│   └── registry.example.yaml
├── doc/
│   ├── INSTALL.md
│   ├── GETTING_STARTED.md        # NOUVEAU : onboarding utilisateurs
│   ├── PATTERNS_GUIDE.md         # NOUVEAU : guide pratique des patterns
│   └── CLIENT_DELIVERY.md        # NOUVEAU : guide de livraison client
├── CLAUDE.md
├── PATTERNS.md                   # Reference theorique (inchange)
└── FUTURE_CONSIDERATIONS.md      # Ce fichier
```

**Principes de reorganisation** :
1. `mcp/` reste la couche universelle — ne change pas
2. `workflows/` est la nouvelle couche abstraite (remplace le role de `skills/` comme source de verite)
3. `skills/`, `agents/`, `rules/` deviennent des **cibles de compilation** — generees a partir de `workflows/`
4. `patterns/` fournit des building blocks reutilisables
5. `templates/` aide les utilisateurs a demarrer

### La question du "workflow definition format"

**Option A : TypeScript-as-DSL (recommandee)**

```typescript
// workflows/mr-review/workflow.ts
import { defineWorkflow } from "@renkei/core";
import { routing } from "../../patterns/routing";
import { reflection } from "../../patterns/reflection";

export default defineWorkflow({
  name: "mr-review",
  description: "Review a GitLab merge request",
  triggers: ["review la MR", "regarde la MR", "analyse la MR"],

  inputs: {
    mr: { type: "string", description: "MR number or URL" },
  },

  mcpDependencies: ["renkei-gitlab", "renkei-project-registry"],

  steps: [
    {
      id: "resolve",
      pattern: "tool-use",
      action: "resolve_project",
      description: "Identifier le projet et les credentials GitLab",
    },
    {
      id: "fetch-mr",
      pattern: "tool-use",
      action: "get_merge_request",
      description: "Recuperer les details de la MR",
    },
    {
      id: "fetch-changes",
      pattern: "parallelization",
      actions: ["get_mr_changes", "get_mr_discussions", "get_mr_pipelines"],
      description: "Recuperer diffs, discussions et pipelines en parallele",
    },
    {
      id: "review",
      pattern: "reflection",
      config: { maxIterations: 2 },
      description: "Analyser le code, critiquer, raffiner l'analyse",
    },
    {
      id: "output",
      pattern: "routing",
      routes: {
        "has-blocking-issues": "post-draft-notes",
        "clean": "summary-only",
      },
    },
  ],

  output: {
    format: "markdown",
    language: "fr",
    sections: ["blocking", "should-fix", "consider"],
  },
});
```

**Avantages** : Type, IDE autocompletion, testable, pas de nouveau langage a apprendre.
**Inconvenients** : Plus verbose que du YAML, necessite de comprendre TypeScript.

**Option B : YAML declaratif**

```yaml
name: mr-review
triggers: ["review la MR", "regarde la MR"]
mcp: [renkei-gitlab, renkei-project-registry]
steps:
  - id: resolve
    pattern: tool-use
    action: resolve_project
  - id: fetch
    pattern: parallelization
    actions: [get_mr_changes, get_mr_discussions]
  - id: review
    pattern: reflection
    max_iterations: 2
output:
  format: markdown
  language: fr
```

**Avantages** : Simple, accessible, pas besoin de TypeScript.
**Inconvenients** : Pas type, pas de logique conditionnelle complexe, un schema a maintenir.

**Option C : Hybrid (recommandee pour la V1)**

YAML pour la definition declarative, TypeScript pour la logique custom. Les cas simples restent en YAML, les cas complexes ont un `handler.ts`.

**Mon avis** : Commencer par **Option A (TypeScript)** pour les workflows internes, mais fournir un **generateur interactif** (comme skill-creator) pour que les utilisateurs non-TypeScript puissent creer des workflows via conversation.

---

## 5. Plan d'evolution

### Phase 1 — Fondations (4-6 semaines)

**Objectif** : Poser les bases sans casser l'existant.

#### 1.1 Abstraction du backend installer

Creer une interface `Backend` dans `installer/backends/types.ts` :

```typescript
interface Backend {
  name: string;
  detectInstalled(): boolean;
  installSkill(workflow: WorkflowDef, targetDir: string): void;
  installAgent(agent: AgentDef, targetDir: string): void;
  registerMcp(mcp: McpDef): void;
  installHook?(hook: HookDef): void;
}
```

Implementer `ClaudeBackend` en extrayant la logique actuelle de `configure.ts`. Ajouter `CursorBackend` comme premier backend alternatif (le plus simple : generer un `.mdc` rule file a partir d'un workflow).

#### 1.2 Registry template et validation

- Creer `templates/registry.example.yaml` avec des commentaires explicatifs
- Ajouter une commande `./install.ts --init` qui copie le template vers `~/.renkei/registry.yaml`
- Rendre `validate_registry` plus explicite (messages d'erreur actionnables)

#### 1.3 Modulariser redmine-mcp

Aligner sur le modele de gitlab-mcp : `src/tools/`, `src/client.ts`, `src/formatters.ts`.

#### 1.4 Observabilite minimale

Ajouter un systeme de logs structure dans les MCPs :

```typescript
// Chaque tool call log dans ~/.renkei/logs/YYYY-MM-DD.jsonl
{ ts: "...", mcp: "redmine", tool: "get_ticket", params: { ticket_id: 123 }, duration_ms: 450, status: "ok" }
```

Un outil CLI `renkei-logs` pour lire et filtrer ces logs.

#### 1.5 Documentation onboarding

Creer `doc/GETTING_STARTED.md` : guide pas-a-pas pour un utilisateur qui decouvre Renkei. De l'installation a la creation de son premier workflow.

---

### Phase 2 — Workflow Engine (6-8 semaines)

**Objectif** : Permettre de definir des workflows abstraits et de les compiler vers les formats cibles.

#### 2.1 Le dossier `workflows/`

Migrer les skills existants vers des definitions de workflow structurees. Chaque workflow a :
- `workflow.ts` ou `workflow.yaml` : definition
- `README.md` : documentation utilisateur
- `tests/` : tests automatises (mock des MCP tools)

#### 2.2 Compilateur workflow → skill/rule

Un script `renkei compile` qui :
1. Lit les definitions de `workflows/`
2. Genere `skills/<name>/SKILL.md` pour Claude Code
3. Genere `rules/<name>.mdc` pour Cursor
4. Genere les sections pertinentes pour Codex/Gemini

Le skill-creator actuel devient un **wrapper** autour de ce compilateur + un mode interactif.

#### 2.3 Pattern library

Implementer les premiers patterns comme modules reutilisables dans `patterns/` :

**Priorite 1** (deja implicitement utilises) :
- `tool-use` — Appel d'outil MCP avec gestion d'erreur
- `routing` — Branchement conditionnel basé sur l'input ou le resultat
- `parallelization` — Execution parallele de sous-taches + aggregation
- `reflection` — Boucle generate → evaluate → refine
- `planning` — Decomposition d'objectif en sous-etapes

**Priorite 2** (valeur immediate) :
- `hitl` — Gate d'approbation humaine avant action critique
- `exception-handling` — Retry, fallback, degradation gracieuse
- `memory` — Persistence de contexte entre etapes du workflow

**Priorite 3** (avance) :
- `rag` — Enrichissement par base de connaissances (le schema existe deja dans registry)
- `multi-agent` — Topologies de coordination (supervisor, hierarchical)
- `guardrails` — Validation input/output, filtrage de contenu

#### 2.4 Tests de workflow

Framework de test qui :
- Mock les appels MCP (pas besoin d'une instance Redmine/GitLab pour tester)
- Verifie que les steps s'executent dans l'ordre
- Valide les outputs contre un schema attendu
- Mesure le nombre de tokens consommes (estimation)

---

### Phase 3 — Ecosystem (8-12 semaines)

**Objectif** : Rendre Renkei auto-suffisant et partageable.

#### 3.1 Marketplace interne

Un repertoire partage (Git repo, registre npm, ou simple dossier reseau) ou les utilisateurs publient leurs workflows :

```bash
renkei publish mr-review          # Publie le workflow
renkei search "security"          # Cherche des workflows
renkei install @user/sec-audit  # Installe un workflow tiers
```

#### 3.2 Evaluation framework

Generaliser le systeme d'evaluation de skill-creator a tous les workflows :
- Benchmark automatise (execution sur des cas de test)
- Metriques : precision, recall, tokens, latence, cout
- Regression testing : comparer versions du workflow
- Dashboard simple (HTML statique ou terminal)

#### 3.3 Nouveaux MCPs

En fonction des besoins :
- `jira-mcp` — Pour les clients Jira (le skill issue-fetching le reference deja)
- `asana-mcp` — Idem pour Asana
- `confluence-mcp` / `notion-mcp` — Pour le knowledge retrieval (RAG)
- `slack-mcp` — Pour les notifications et l'inter-agent communication

#### 3.4 Patterns avances

- **Resource-Aware Optimization** : Router vers haiku/sonnet/opus selon la complexite de la tache
- **Goal Setting and Monitoring** : Suivi de progression pour les workflows longs
- **Inter-Agent Communication** : Protocole pour que les workflows se parlent
- **Learning and Adaptation** : Feedback loop pour ameliorer les workflows au fil du temps

#### 3.5 Mode client

Un `renkei package` qui :
- Cree une archive auto-installable avec les workflows selectionnes
- Inclut une registry.yaml pre-configuree pour le client
- Exclut les workflows internes
- Genere une documentation client specifique

---

## 6. Propositions detaillees

### 6.1 Le systeme de backend multi-outil

**Principe** : Chaque outil IA est un "backend" avec ses propres conventions. Renkei definit une interface commune et chaque backend l'implemente.

**Ce que chaque backend doit savoir faire** :

| Capacite | Claude Code | Cursor | Codex | Gemini |
|----------|-------------|--------|-------|--------|
| Installer un MCP | `claude mcp add` | `.cursor/mcp.json` | `codex mcp add` ? | Extension config |
| Installer une "skill" | `~/.claude/skills/X/SKILL.md` | `.cursor/rules/X.mdc` | `AGENTS.md` section | ADK config |
| Installer un agent | `~/.claude/agents/X.md` | N/A | N/A | ADK agent config |
| Installer un hook | `settings.json.hooks` | N/A | N/A | N/A |
| Config globale | `~/.claude.json` | `~/.cursor/` | `~/.codex/` | `~/.config/gcloud/` |

**Implementation progressive** :
1. Extraire `ClaudeBackend` de l'installer actuel (0 effort, juste du refactoring)
2. Ajouter `CursorBackend` (faible effort : generer des `.mdc` files)
3. `CodexBackend` et `GeminiBackend` quand le besoin se materialise

### 6.2 Le compilateur de workflows

**Entree** : Un workflow defini en TypeScript (`workflows/X/workflow.ts`)
**Sorties** :

Pour Claude Code :
```markdown
---
name: renkei-X
description: ...
---
# Instructions du workflow
(genere a partir des steps, patterns, et output du workflow.ts)
```

Pour Cursor :
```markdown
---
description: ...
globs: ["**/*"]
---
# Instructions du workflow
(adapte pour le format Cursor rules)
```

Le compilateur est un script TypeScript qui importe les definitions de workflow et genere les fichiers cibles. Pas de magie — c'est du template rendering type.

### 6.3 Observabilite

**Trois niveaux** :

1. **MCP-level** : Chaque appel d'outil est logge (tool, params, duree, status)
2. **Workflow-level** : Chaque execution de workflow est tracee (steps executes, decisions prises, tokens consommes)
3. **Platform-level** : Agregation par jour/semaine (cout total, workflows les plus utilises, taux d'erreur)

**Implementation** :
- Niveau 1 : Wrapper autour de `registerTool` qui log automatiquement
- Niveau 2 : Le compilateur injecte des points de trace dans les skills generes
- Niveau 3 : Outil CLI `renkei stats` qui lit les logs

### 6.4 Security model pour les clients

**Problemes a resoudre** :
- Un utilisateur ne doit pas voir les credentials d'un autre client
- Un workflow client ne doit pas acceder aux systemes internes
- L'audit trail doit montrer qui a fait quoi

**Solutions** :
- `registry.yaml` par projet (pas global) pour les deployments client
- Les env vars de credentials sont scopes par projet dans le shell (pas dans Renkei)
- Les logs d'observabilite fournissent l'audit trail
- Le mode `--project` de l'installer isole deja les skills/agents par projet

### 6.5 Versioning des composants

**Probleme** : Aujourd'hui, `./install.ts --all` ecrase tout. Pas de diff, pas de changelog, pas de rollback.

**Solution** :
- Ajouter un champ `version` dans chaque workflow/MCP/tool (`package.json` pour les MCPs, frontmatter pour les workflows)
- L'installer affiche le diff de version avant installation
- Les installations sont taggees dans le cache (version + date)
- `renkei rollback <component>` restaure la version precedente depuis le cache Git

---

## 7. Ce a quoi tu n'as pas pense

### 7.1 Distribution et mise a jour

Comment un utilisateur obtient Renkei ? Aujourd'hui : `git clone` + `./install.ts`. Mais :
- Il faut Bun installe
- Il faut cloner le repo (acces Git necessaire)
- Les mises a jour sont manuelles (`git pull` + `./install.ts --all`)

**Proposition** : Un script d'installation one-liner :
```bash
curl -fsSL https://renkei.internal/install.sh | bash
```
Qui installe Bun si necessaire, clone le repo, et lance l'installer. Pour les mises a jour : `renkei update` qui fait `git pull` + reinstallation.

### 7.2 Configuration en couches

Aujourd'hui `registry.yaml` est unique et global. Mais en contexte equipe/client :

```
~/.renkei/registry.yaml          # Config personnelle (mes projets, mes credentials)
<projet>/.renkei/registry.yaml   # Config projet (partageee avec l'equipe via Git)
```

Le project-registry-mcp devrait merger ces couches (projet override global). Cela permet de committer la config projet sans exposer les credentials personnelles.

### 7.3 Dry run / preview mode

Avant d'executer un workflow qui modifie des systemes externes (post de notes Redmine, creation de MR), il serait utile de pouvoir previsualiser :

```
[DRY RUN] Would post draft note on MR !42:
  File: src/auth.ts:15
  Comment: "Missing null check on user.token"
```

Implementation : Un flag `--dry-run` dans les MCPs qui retourne l'action prevue sans l'executer.

### 7.4 Rate limiting intelligent

Les APIs GitLab/Redmine ont des rate limits. Un workflow qui fait 50 appels en parallele va se faire bloquer.

**Proposition** : Un rate limiter dans le client HTTP partage par MCP, configurable dans `registry.yaml` :
```yaml
gitlab:
  production:
    rate_limit: 10/s     # Max 10 requetes par seconde
    retry_after: true     # Respecter le header Retry-After
```

### 7.5 Couts et budget

Les workflows multi-agent consomment des tokens. Un workflow mal ecrit peut couter cher.

**Proposition** :
- Tracker les tokens par workflow (via l'observabilite)
- Alerter quand un workflow depasse un seuil configurable
- Le pattern `resource-aware-optimization` route vers des modeles moins chers pour les taches simples
- Dashboard dans `renkei stats` : "Cette semaine, mr-review a consomme ~$2.40 sur 15 executions"

### 7.6 Composabilite des workflows

Un workflow devrait pouvoir appeler un autre workflow comme sous-etape :

```typescript
steps: [
  { id: "fetch", workflow: "issue-fetching", params: { ticket: "$input.ticket" } },
  { id: "branch", workflow: "branching", params: { name: "$fetch.result.title" } },
  { id: "solve", pattern: "planning", ... },
]
```

Cela evite la duplication et permet de construire des workflows complexes par composition.

### 7.7 Mode offline / air-gappe

Certains clients n'ont pas internet (defense, sante, finance). Renkei doit pouvoir fonctionner :
- Sans acces a npm/bun registry (pre-bundler les node_modules)
- Sans LLM cloud (support de modeles locaux via Ollama/llama.cpp)
- Sans MCP distant (tout local)

L'anonymizer supporte deja un LLM local — le pattern est la.

### 7.8 Internationalisation

Les skills actuels repondent en francais (hardcode). Pour les clients internationaux :
- Le language de sortie devrait etre configurable dans `registry.yaml` ou par workflow
- Les templates de commit, MR, review devraient etre localisables

### 7.9 Le naming "Renkei"

"Renkei" (連携) signifie "coordination, collaboration" en japonais — parfait pour un orchestrateur de workflows. Mais :
- Le README dit "MCP servers for driving [...] through Claude Code" — trop reducteur
- Le branding devrait refleter la vision : "Agentic workflow platform" ou "Framework de workflows agentiques"
- Le prefixe `renkei-` sur les skills est bien — il cree une identite

### 7.10 Governance multi-utilisateur

Quand plusieurs utilisateurs contribuent des workflows :
- Qui review les workflows avant publication ?
- Comment gerer les conflits de version ?
- Comment deprecier un workflow obsolete ?

**Proposition** : Les workflows vivent dans un repo Git avec des PRs classiques. Le "marketplace" est juste un remote Git avec une convention de nommage.

---

## 8. Mapping des 21 patterns agentiques

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

**Resume des priorites** :
- **P1 (deja utilise, a formaliser)** : Chaining, Routing, Parallelization, Reflection, Evaluation → 5 patterns
- **P2 (valeur immediate)** : Planning, Multi-Agent, Memory, Exception Handling, HITL, RAG, Resource-Aware, Guardrails → 8 patterns
- **P3 (avance/recherche)** : Learning, Goal Monitoring, A2A, Reasoning, Prioritization, Exploration → 6 patterns
- **FAIT** : Tool Use, MCP → 2 patterns

---

## 9. Decisions a prendre

Ces questions meritent une reponse avant de commencer :

### Decision 1 : Workflow format

**TypeScript-only** (Option A) vs. **YAML + TypeScript** (Option C hybride) ?

→ Impact tout le reste. Si TypeScript-only, les utilisateurs non-dev sont exclus. Si hybride, il faut maintenir deux parsers.

### Decision 2 : Profondeur du multi-outil

**Claude-first + Cursor** (pragmatique) vs. **4 outils des le depart** (ambitieux) ?

→ Je recommande Claude-first + Cursor. Codex et Gemini quand le besoin se materialise.

### Decision 3 : Compilation vs. interpretation

Les workflows sont-ils **compiles** en skills/rules (statique, simple) ou **interpretes** a l'execution (dynamique, complexe) ?

→ La compilation est plus simple, plus debuggable, et suffit pour 90% des cas. L'interpretation est necessaire pour les patterns dynamiques (routing LLM-based). Je recommande compilation par defaut, interpretation opt-in.

### Decision 4 : Scope du "marketplace"

**Repo Git partage** (simple, Git-native) vs. **Registre npm** (scalable, versionne) vs. **Dossier reseau** (zero infra) ?

→ Commencer par un repo Git. Migrer vers npm si l'adoption explose.

### Decision 5 : Open source ?

Renkei est "Private" aujourd'hui. Mais si l'objectif est la livraison client et l'adoption par les utilisateurs, le modele de distribution compte.

Options :
- **Private** : Repo interne, acces par invitation. Simple mais limitant.
- **Open-core** : Le framework est open source, les workflows metier sont prives. Bon pour l'adoption et le recrutement.
- **Full open source** : Tout est public. Maximum d'adoption, minimum de controle.

→ Cette decision est strategique, pas technique. Mais elle impacte la distribution (section 7.1).

### Decision 6 : Retrocompatibilite

Les skills actuels continuent-ils de fonctionner "as-is" pendant la migration vers les workflows ?

→ Oui, obligatoirement. Les skills existants restent dans `skills/` et fonctionnent. Les workflows les remplacent progressivement. Pas de big bang.

---

## 10. Synthese

Renkei a une base solide : des MCPs bien concus, un installer ergonomique, des skills qui encodent de vrais workflows utiles, et une architecture de securite (credentials) exemplaire.

Le gap entre l'etat actuel et la vision est significatif mais franchissable. Les trois mouvements cles :

1. **Abstraire** : Separer la definition du workflow de sa materialisation (skill Claude, rule Cursor, etc.)
2. **Formaliser** : Transformer les patterns agentiques implicites en building blocks explicites et composables
3. **Outiller** : Donner aux utilisateurs les moyens de creer, tester et partager leurs propres workflows

Le risque principal est la sur-ingenierie. La regle d'or : **chaque abstraction doit etre justifiee par au moins deux cas d'usage concrets**. Si un pattern n'a qu'un seul usage, il reste inline dans le workflow.

Renkei ne doit pas devenir un framework theorique parfait que personne n'utilise. Il doit rester ce qu'il est : un outil pragmatique qui resout des problemes reels — mais avec une architecture qui lui permet de grandir.
