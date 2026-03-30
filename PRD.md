# PRD — Renkei : Package Manager pour Workflows Agentiques

## Problem Statement

Les développeurs et équipes qui utilisent des outils IA (Claude Code, Cursor, Codex, etc.) produisent des workflows agentiques complexes : skills, hooks, agents spécialisés, configurations MCP, scripts. Ces artefacts sont aujourd'hui :

- **non versionnés** : pas de suivi des versions, impossible de savoir quelle version tourne en production ;
- **non partageables** simplement : copier-coller manuel entre machines, Slack, email, drive ;
- **non portables** : chaque développeur reconfigure à la main pour chaque outil IA ;
- **invisibles** : pas de liste des workflows installés, pas de diagnostic de l'état de santé ;
- **fragiles** : une modification locale d'un skill rompt silencieusement le workflow.

Il n'existe pas de primitive standard pour distribuer un "workflow complet" (skill + hook + agent + config MCP) comme on distribue un package npm ou un plugin homebrew.

---

## Solution

**Renkei** est un package manager CLI (`rk`) écrit en Rust qui permet d'installer, versionner et partager des workflows agentiques. Un workflow est un **package** : un dossier avec un manifeste `renkei.json` décrivant ses artefacts. La CLI déploie chaque artefact au bon emplacement selon le backend détecté (Claude Code, Cursor…), sans configuration manuelle.

```
rk install git@github.com:meryll/mr-review
rk install ./mon-workflow/
rk list
rk doctor
```

Renkei est **agnostique au contenu** des packages — il ne les exécute pas, il les distribue.

---

## User Stories

### Installation et déploiement

1. En tant que développeur, je veux installer un workflow depuis un repo Git (SSH) afin de l'utiliser immédiatement dans mon outil IA sans configuration manuelle.
2. En tant que développeur, je veux installer un workflow depuis un repo Git (HTTPS) afin de pouvoir le faire depuis un environnement sans clé SSH.
3. En tant que développeur, je veux installer une version spécifique d'un workflow via un tag Git (`--tag v1.2.0`) afin de garantir la reproductibilité de mon environnement.
4. En tant que développeur, je veux installer un workflow depuis un dossier local (chemin relatif ou absolu) afin de tester un package en cours de développement sans le publier.
5. En tant que développeur, je veux que `rk install` valide le manifeste `renkei.json` avant tout déploiement afin d'échouer tôt sur une configuration invalide.
6. En tant que développeur, je veux que `rk install` détecte automatiquement le backend installé (Claude Code, Cursor) afin de ne déployer que là où c'est pertinent.
7. En tant que développeur, je veux être prévenu si le package ne supporte pas mon backend avant l'installation afin d'éviter un déploiement partiel ou incohérent.
8. En tant que développeur, je veux pouvoir forcer l'installation d'un package incompatible avec mon backend via `--force` afin de l'installer malgré l'incompatibilité déclarée, en connaissance de cause.
9. En tant que développeur, je veux que les skills soient déployés sous `~/.claude/skills/renkei-<name>/` afin qu'ils soient isolés des skills natifs et facilement identifiables.
10. En tant que développeur, je veux que les hooks soient mergés dans `~/.claude/settings.json` afin qu'ils s'activent automatiquement dans Claude Code.
11. En tant que développeur, je veux que les agents soient déployés dans `~/.claude/agents/` afin d'être disponibles directement depuis Claude Code.
12. En tant que développeur, je veux que les configurations MCP déclarées dans `renkei.json` soient enregistrées dans `~/.claude.json` afin d'activer les serveurs MCP requis automatiquement.
13. En tant que développeur, je veux voir la liste des variables d'environnement requises manquantes après installation afin de les configurer sans chercher dans la documentation.
14. En tant que développeur, je veux pouvoir relancer `rk install` sur un package déjà installé afin de mettre à jour les artefacts déployés vers la nouvelle version.

### Gestion des conflits

15. En tant que développeur, je veux être alerté si deux packages déploient un skill avec le même nom afin d'éviter des écrasements silencieux.
16. En tant que développeur, je veux pouvoir renommer un skill en conflit via un prompt interactif afin de conserver les deux packages côte à côte.
17. En tant que développeur, je veux que le renommage mette à jour le `name` dans le frontmatter du skill afin que la référence reste cohérente.
18. En tant que développeur, je veux que le mapping nom-original → nom-déployé soit persisté dans le cache local afin que les commandes `doctor` et `list` restent exactes après renommage.

### Listing et visibilité

19. En tant que développeur, je veux lister tous les packages installés avec leurs versions et sources (`rk list`) afin d'avoir une vue d'ensemble de mon environnement.
20. En tant que développeur, je veux distinguer les packages installés depuis Git de ceux installés depuis un chemin local dans `rk list` afin de savoir lesquels peuvent être mis à jour automatiquement.

### Diagnostic

21. En tant que développeur, je veux diagnostiquer l'état de mes packages installés (`rk doctor`) afin de détecter les problèmes sans inspecter les fichiers manuellement.
22. En tant que développeur, je veux que `rk doctor` signale les skills modifiés localement afin de savoir lesquels ont divergé de l'original.
23. En tant que développeur, je veux que `rk doctor` liste les variables d'environnement manquantes par package afin de corriger rapidement les gaps de configuration.
24. En tant que développeur, je veux que `rk doctor` vérifie la présence des backends (Claude Code, Cursor) afin de confirmer que les artefacts ont bien un runtime.
25. En tant que développeur, je veux un code de sortie non-nul quand `rk doctor` détecte des problèmes afin de pouvoir l'intégrer dans des scripts CI.

### Création de packages

26. En tant que créateur de workflow, je veux valider que tous les fichiers déclarés dans `renkei.json` existent (`rk package`) afin d'éviter de distribuer un package cassé.
27. En tant que créateur de workflow, je veux générer une archive `<name>-<version>.tar.gz` de mon package afin de le distribuer facilement.
28. En tant que créateur de workflow, je veux bumper automatiquement la version (patch / minor / major) via `--bump` afin de suivre semver sans éditer manuellement le manifeste.
29. En tant que créateur de workflow, je veux voir un résumé des fichiers inclus et la taille de l'archive après `rk package` afin de vérifier le contenu avant distribution.

### Lockfile

30. En tant que développeur, je veux qu'un lockfile `rk.lock` soit généré automatiquement à la racine du projet après chaque installation afin de figer les versions exactes installées dans ce contexte projet.
31. En tant que membre d'une équipe, je veux commiter `rk.lock` dans le repo projet afin que tous le reste de l'équipe travaillent avec les mêmes versions de workflows.
32. En tant que nouveau membre d'une équipe, je veux cloner le projet et lancer `rk install` (sans arguments) afin d'obtenir immédiatement les mêmes workflows que le reste de l'équipe, sans aucune configuration supplémentaire.
33. En tant que développeur, je veux que `rk install` sans arguments lise `rk.lock` et installe les versions exactes déclarées afin de reproduire l'environnement à l'identique.
34. En tant que développeur, je veux que le lockfile inclue l'intégrité (hash SHA-256) de chaque package afin de détecter toute corruption ou altération.

### Phase 1 — Livraison et migration

35. En tant que mainteneur du projet, je veux que la CLI soit compilée en binaires natifs pour Linux / macOS / Windows et publiée automatiquement via GitHub Actions à chaque release afin que les utilisateurs puissent l'installer sans dépendance.
36. En tant que créateur de workflow, je veux migrer les workflows existants (renkei-old) en packages Renkei valides afin de valider le format `renkei.json` sur des cas réels dès la v1.

### Phase 2 — Registry et commandes avancées

37. En tant que créateur de workflow, je veux publier mon package dans un registry centralisé (`rk publish`) afin de le rendre découvrable par d'autres équipes.
38. En tant que développeur, je veux rechercher des packages dans le registry (`rk search <query>`) afin de trouver des workflows existants sans parcourir des repos manuellement.
39. En tant que développeur, je veux installer un package par son nom scopé (`rk install @scope/name`) afin de ne pas avoir à connaître l'URL Git.
40. En tant que développeur, je veux mettre à jour un package vers sa dernière version compatible (`rk update`) afin de bénéficier des améliorations sans reinstaller manuellement.
41. En tant que développeur, je veux désinstaller un package et nettoyer tous ses artefacts déployés (`rk uninstall`) afin de ne pas laisser de résidus.
42. En tant que développeur, je veux obtenir les détails d'un package (description, auteur, versions, dépendances) via `rk info` afin de l'évaluer avant installation.
43. En tant que créateur de workflow, je veux scaffolder interactivement un nouveau package (`rk init`) afin de démarrer avec une structure valide sans l'écrire de zéro.
44. En tant que développeur, je veux voir le diff entre les artefacts déployés et l'archive originale (`rk diff`) afin d'auditer mes modifications locales.
45. En tant que développeur, je veux restaurer les artefacts d'un package depuis l'archive originale (`rk reset`) afin d'annuler mes modifications locales.
46. En tant que créateur de workflow, je veux forker un package existant sous mon scope (`rk fork --scope <s>`) afin de créer une variante indépendante sans modifier l'original.
47. En tant qu'utilisateur, je veux m'authentifier auprès du registry (`rk login` / `rk logout`) afin de publier sous mon scope.
48. En tant que développeur, je veux que les packages Cursor soient déployés dans `.cursor/skills/<name>/` afin d'utiliser mes workflows dans Cursor sans configuration.
49. En tant que créateur de workflow, je veux déclarer un scope organisationnel (`@acme-corp/`) afin d'éviter les collisions de noms entre équipes.

### Phase 3 — Écosystème

50. En tant que développeur, je veux naviguer les packages disponibles sur un site web public afin de découvrir des workflows sans CLI.
51. En tant que créateur de workflow, je veux un profil public affichant mes packages publiés afin de construire ma réputation dans l'écosystème.
52. En tant qu'organisation, je veux un registry privé sous mon scope afin de distribuer des workflows internes sans les exposer publiquement.
53. En tant que développeur, je veux mettre à jour la CLI automatiquement (`rk self-update`) afin de toujours disposer des dernières corrections.
54. En tant qu'administrateur, je veux accéder aux statistiques d'installation de mes packages afin de mesurer leur adoption.

---

## Implementation Decisions

### Langage et distribution
- CLI écrite en **Rust** : binaire natif, zero-dépendance à l'exécution.
- Compilation croisée pour Linux / macOS / Windows via GitHub Actions.
- Distribution via **GitHub Releases** — un seul fichier exécutable.
- Licence open source pour la CLI ; le site web registry sera closed source.

### Workspace

Un repo Git peut contenir plusieurs packages (workspace). Chaque sous-package est dans un dossier à la racine (`./mr-review/`, `./auto-test/`). Un `renkei.json` racine déclare les membres via un champ `workspace` :

```json
{
  "workspace": ["mr-review", "auto-test"]
}
```

Chaque sous-dossier contient son propre `renkei.json` complet et ses dossiers conventionnés.

Pour un repo sans workspace (package simple), les dossiers conventionnés (`skills/`, `hooks/`, `agents/`) sont directement à la racine.

### Manifeste `renkei.json`
- Champs obligatoires : `name` (scopé `@scope/nom`, **obligatoire dès v1**), `version` (semver), `description`, `author`, `license`, `backends`.
- Champs optionnels : `keywords`, `mcp`, `requiredEnv`, `workspace`.
- **Pas de champ `artifacts`** : convention pure. Les dossiers `skills/`, `hooks/`, `agents/` sont la source de vérité. Tout fichier présent dans ces dossiers est un artefact déployé.
- `mcp` déclare les configurations MCP au format natif `command`/`args`/`env` (standard entre Claude et Cursor, pas d'abstraction supplémentaire).
- `requiredEnv` liste les variables d'environnement avec leur description.

```json
{
  "name": "@meryll/mr-review",
  "version": "1.2.0",
  "description": "Review automatique de code",
  "author": "meryll",
  "license": "MIT",
  "backends": ["claude"],
  "mcp": {
    "my-server": {
      "command": "node",
      "args": ["server.js"],
      "env": { "API_KEY": "${API_KEY}" }
    }
  },
  "requiredEnv": {
    "GITHUB_TOKEN": "Required for GitHub API access"
  }
}
```

### Format neutre des artefacts

Tous les artefacts sont écrits dans un format neutre Renkei que chaque backend traduit :

- **Skills et agents** : format markdown + frontmatter (style Claude Code). Ce format est le format neutre Renkei — les autres backends traduisent depuis ce format.
- **Hooks** : format Renkei abstrait avec événements normalisés (voir section Hooks ci-dessous).
- **MCP** : format natif `command`/`args`/`env` directement dans le manifeste (déjà portable entre backends).

```markdown
---
name: review
description: Review code changes
---
Review the code...
```

### Hooks : format et événements

Les fichiers `hooks/*.json` utilisent un format Renkei abstrait avec des événements normalisés. Chaque backend mappe ces événements vers ses propres événements natifs.

```json
[
  {
    "event": "before_tool",
    "matcher": "bash",
    "command": "bash scripts/lint.sh",
    "timeout": 5
  }
]
```

**Mapping des événements Renkei → Claude Code :**

| Event Renkei | Claude Code |
|-------------|-------------|
| `before_tool` | `PreToolUse` |
| `after_tool` | `PostToolUse` |
| `after_tool_failure` | `PostToolUseFailure` |
| `on_notification` | `Notification` |
| `on_session_start` | `SessionStart` |
| `on_session_end` | `SessionEnd` |
| `on_stop` | `Stop` |
| `on_stop_failure` | `StopFailure` |
| `on_subagent_start` | `SubagentStart` |
| `on_subagent_stop` | `SubagentStop` |
| `on_elicitation` | `Elicitation` |

Ce mapping est maintenu dans `ClaudeBackend`. Les autres backends définiront leur propre mapping.

**Tracking** : les hooks déployés sont tracés dans `~/.renkei/install-cache.json`, pas dans le JSON du backend. Le JSON du backend (`settings.json`, etc.) reste 100% natif, sans champs customs. À la désinstallation, Renkei compare avec son cache pour retirer les bonnes entrées.

### Interface Backend
- Un trait `Backend` définit les opérations : `name`, `detect_installed`, `deploy_skill`, `deploy_hook`, `deploy_agent`, `register_mcp`.
- **Tout ce qui est spécifique à un backend doit être abstrait** derrière cette interface.
- `ClaudeBackend` est la seule implémentation en v1. `CursorBackend` sera ajouté en v2 sans refactoring.
- **Détection** : un backend est considéré installé si son dossier de config existe (`~/.claude/` pour Claude, `.cursor/` pour Cursor). Pas de vérification du binaire dans le PATH.

### Matrice de support multi-backend

| Artefact   | Claude Code              | Cursor               | Codex      | Gemini |
|------------|--------------------------|----------------------|------------|--------|
| Skills     | `SKILL.md`               | Skills               | `AGENTS.md`| ?      |
| Hooks      | `settings.json` events   | N/A                  | N/A        | N/A    |
| Agents     | `agents/*.md`            | N/A                  | N/A        | N/A    |
| MCP config | `~/.claude.json`         | `.cursor/mcp.json`   | ?          | ?      |

Codex et Gemini sont dans le radar mais non planifiés. Le format d'artefact varie selon le backend (`AGENTS.md` pour Codex vs `agents/*.md` pour Claude).

### Conventions de déploiement (hardcodées, pas configurables)

| Artefact  | Claude Code                              | Cursor                       |
|-----------|------------------------------------------|------------------------------|
| Skills    | `~/.claude/skills/renkei-<name>/SKILL.md` | `.cursor/skills/<name>/`     |
| Hooks     | Merge dans `~/.claude/settings.json`     | N/A                          |
| Agents    | `~/.claude/agents/<name>.md`             | N/A                          |
| MCP config| Merge dans `~/.claude.json`              | Merge dans `.cursor/mcp.json` |

Le préfixe `renkei-` sur les skills crée un namespace clair et évite les collisions avec les skills natifs.

### Installation : Git

1. `git clone --depth 1` dans un dossier temporaire (`/tmp/rk-xxx/`)
2. Validation du manifeste `renkei.json`
3. Création de l'archive `.tar.gz` dans `~/.renkei/cache/@scope/name/<version>.tar.gz`
4. Déploiement des artefacts depuis l'archive
5. Suppression du clone temporaire

Sans `--tag` ni `--branch`, HEAD de la branche par défaut est utilisé. Le SHA du commit est enregistré dans le lockfile pour la reproductibilité. La version dans `renkei.json` fait foi (trust the manifest) — pas de vérification de cohérence avec les tags Git.

### Installation : locale

- `rk install ./mon-workflow/` crée une **copie** (snapshot archive dans le cache), comme pour Git.
- `rk install --link ./mon-workflow/` crée des **symlinks** pour le développement (modèle `npm link` / `pip install -e`). Les changements dans les fichiers sources sont immédiatement reflétés.

### Installation : sans arguments

- Si `rk.lock` existe dans le répertoire courant → installe les versions exactes du lockfile.
- Si pas de lockfile mais workspace détecté → erreur explicite : "workspace détecté, utilisez `rk install --link .` pour dev".

### Gestion des erreurs : fail-fast + rollback

À la première erreur pendant l'installation, arrêt immédiat et rollback de tous les artefacts déjà déployés. Atomicité garantie : soit tout passe, soit rien ne change.

### Gestion des conflits
- Détection via `install-cache.json` avant tout déploiement.
- **TTY (interactif)** : prompt pour renommer l'artefact en conflit. Le renommage met à jour le champ `name` dans le frontmatter du skill.
- **Non-TTY (CI)** : erreur avec exit code 1.
- **`--force`** : le dernier installé écrase silencieusement.
- Le mapping nom-original → nom-déployé est persisté dans `install-cache.json`.

### Variables d'environnement

Les variables d'environnement requises manquantes déclenchent un **warning** après installation, pas un blocage. `rk doctor` les re-vérifie. L'utilisateur configure après installation.

### Stockage local
- `~/.renkei/cache/@scope/name/<version>.tar.gz` — archives immutables par version.
- `~/.renkei/install-cache.json` — mapping package → artefacts déployés + hooks tracés + renommages.
- `~/.renkei/config.json` — configuration locale (registries, préférences).
- `rk.lock` à la racine du projet — lockfile commitable par projet.

### Lockfile
- Format JSON versionné (`lockfileVersion: 1`).
- Chaque entrée : `version`, `source`, `tag` (optionnel), `resolved` (commit SHA), `integrity` (sha256).
- Généré automatiquement par `rk install`, commitable dans le repo.

```json
{
  "lockfileVersion": 1,
  "packages": {
    "@meryll/mr-review": {
      "version": "1.2.0",
      "source": "git@github.com:meryll/mr-review",
      "tag": "v1.2.0",
      "resolved": "abc123def",
      "integrity": "sha256-..."
    }
  }
}
```

### Diagnostic (`rk doctor`)

Checks en v1 :
- Backends installés (dossier de config existe)
- Fichiers déployés existent toujours
- Variables d'environnement requises présentes
- Skills modifiés localement (hash diff avec archive)
- Hooks toujours présents dans le fichier de config du backend
- MCP configs toujours enregistrées

Pas de check de version distante (registry v2). Code de sortie 0 si tout passe, non-0 sinon.

### Archive (`rk package`)

L'archive `.tar.gz` contient uniquement :
- `renkei.json`
- `skills/`
- `hooks/`
- `agents/`
- `scripts/`

Tout le reste (tests, docs, README, etc.) est exclu.

### Registry v2
- Service HTTP : index `@scope/name` → URL source + metadata.
- `rk publish` envoie l'archive + met à jour l'index.
- Scopes : `@renkei/` réservé aux packages officiels, les autres sont enregistrés à la première publication.
- Auth par token API.

---

## Testing Decisions

**Principe** : tester uniquement les comportements observables depuis l'extérieur, pas les détails d'implémentation internes. Un bon test vérifie ce que la CLI fait (fichiers créés, contenu correct, code de sortie, messages affichés) — pas comment elle le fait.

**Modules à tester :**

- **Parsing du manifeste** : valider que `renkei.json` valide est accepté, que les champs manquants obligatoires provoquent une erreur descriptive, que les types incorrects sont rejetés, que le scope `@scope/name` est requis.
- **Découverte des artefacts par convention** : vérifier que les fichiers dans `skills/`, `hooks/`, `agents/` sont correctement détectés comme artefacts.
- **Déploiement des artefacts** (`ClaudeBackend`) : vérifier que les fichiers sont copiés aux bons chemins après `rk install`, que le merge dans `settings.json` et `~/.claude.json` est correct, que le préfixe `renkei-` est appliqué.
- **Traduction des hooks** : vérifier que le format Renkei abstrait (`before_tool`, etc.) est correctement traduit en événements Claude Code natifs (`PreToolUse`, etc.).
- **Tracking des hooks** : vérifier que les hooks déployés sont enregistrés dans `install-cache.json` et que le rollback les retire correctement.
- **Lockfile** : vérifier que `rk.lock` est créé avec les bonnes versions et hashes, que `rk install` sans arguments installe les versions exactes du lockfile.
- **Détection de backend** : vérifier que `ClaudeBackend::detect_installed` retourne vrai quand Claude Code est présent.
- **Gestion des conflits** : vérifier la détection de collision et le renommage dans `install-cache.json`.
- **`rk doctor`** : vérifier les codes de sortie (0 = sain, non-0 = problèmes), la détection de skills modifiés, les env vars manquantes.
- **`rk package`** : vérifier la création de l'archive, le bump de version dans `renkei.json`, le rejet si des artefacts déclarés sont absents.

---

## Out of Scope

- **Workflow runtime / executor** : Renkei distribue des workflows, il ne les exécute pas.
- **Orchestrateur MCP** : la gestion du cycle de vie des serveurs MCP est laissée à l'outil IA.
- **Pattern library / framework de patterns agentiques** : Renkei est agnostique au contenu.
- **Compilateur workflow → skill** : pas de transformation du contenu des packages.
- **Système de dépendances inter-workflows** : un package ne peut pas déclarer de dépendances vers d'autres packages (v1).
- **Observabilité / métriques d'exécution** : hors périmètre.
- **Interface graphique locale** : la CLI est le seul point d'entrée.

---

## Risks

| Risque | Sévérité | Mitigation |
|--------|----------|------------|
| **Sur-ingénierie** | Haute | Scope minimal en v1. Chaque feature justifiée par un besoin concret. |
| **Adoption nulle** | Haute | Valider avec les premiers utilisateurs (utilisateurs l'équipe) avant d'investir dans le registry. |
| **Évolution rapide des outils IA** | Moyenne | L'interface `Backend` isole du changement. Un seul point d'adaptation par outil. |
| **Format skills instable** | Moyenne | Surveiller les changelogs Claude Code / Cursor. Adapter rapidement. |
| **Rust learning curve** | Faible | Le scope de la CLI est bien défini. Pas de concurrence, pas d'async complexe. |
| **Compétition native** | Faible | Renkei est multi-outils et orienté workflow, pas composant. Complémentaire aux stores natifs. |

---

## Licensing

| Composant | Licence |
|-----------|---------|
| CLI `rk` | Open source |
| Site web registry | Closed source |
| Packages individuels | Au choix du créateur |
| Scope `@renkei/` | Réservé aux packages officiels |

---

## Further Notes

- **Clean break** : le codebase existant (`renkei-old`) sert de référence mais le nouveau Renkei repart de zéro en Rust. Les workflows existants seront packagés en packages Renkei une fois la CLI v1 fonctionnelle — c'est un livrable de Phase 1.
- **Convention over config** : les destinations de déploiement sont hardcodées. L'ajout d'un champ `destination` dans le manifeste est explicitement rejeté — moins de surface d'erreur, moins de décisions pour le créateur de package.
- **Claude-first** : en v1, seul `ClaudeBackend` est implémenté. L'interface `Backend` est la seule concession à la flexibilité future.
- **Validation par les premiers utilisateurs** : avant d'investir dans le registry (v2), valider l'adoption avec les utilisateurs . Si personne n'installe de packages, le registry est prématuré.
- **Le site web (v3) ne doit être construit que si l'écosystème le justifie** — pas de build spéculatif.
- **Scripts dans les packages** : la structure d'un package peut inclure un dossier `scripts/` avec des scripts arbitraires. Ces scripts ne sont pas un type d'artefact nommé dans `artifacts` — ils sont inclus dans l'archive mais leur déploiement n'est pas géré nativement par `rk`. Ce comportement devra être clarifié lors de l'implémentation de `rk package`.
