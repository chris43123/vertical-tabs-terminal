# Handoff: pausa en el job 003 (2026-10-30)

Para la próxima sesión, humano o Claude. Se borra solo cuando se cumple "Cierre".

## Estado

- `main` tiene los jobs 001 y 002 (receta de lanzamiento + Reset, `⌘⇧R` en macOS).
- **Job 003 (favoritos)**, rama `factory/003-favorite-sessions` en `808dcbc`, 152 tests
  pasan. Ya incluye el rework del humano (la fila del favorito es su pestaña, altura
  ajustable, mismo directorio varias veces, encabezado con "+", nota del README) y el
  arreglo de la ronda 3 (Rename en un favorito abre su diálogo de edición).
- La **ronda 4** quedó a medias: solo existe `round-4/review-security.md` (`SKIPPED`).
  Diseño y código se detuvieron antes de escribir.
- **PR #6** (`factory/board-003`): tablero, spec, revisiones y `human.md` del 003.
  Esta copia del repo está en esa rama, no en `main`.
- **PR #7** (`factory-security-gate`): seguridad condicional, UX + UI unidos en
  `design-reviewer` (sonnet) y el job de capturas con `egui_kittest` en el backlog.
  Mientras no se fusione, el tipo de agente `design-reviewer` no existe en la copia
  principal: lánzalo como agente general (sonnet) que lea
  `.worktrees/factory-security-gate/.claude/agents/design-reviewer.md`.
- Worktrees: `.worktrees/003-favorite-sessions` y `.worktrees/factory-security-gate`.
  En otra máquina no existen; se recrean con `git worktree add .worktrees/<x> <rama>`.

## Pasos

1. `git fetch origin` y `git pull --ff-only` en `factory/board-003`.
2. Ronda 4 del 003: corre diseño y código sobre `round-4/` (seguridad sigue
   `SKIPPED`: nada desde `ac094cb` toca su superficie). Es la última ronda permitida.
3. Approver. Si escala, el humano repite en la app las pruebas manuales del rework:
   `⌘R` en la pestaña de un favorito, la barra de altura con muchos favoritos y zoom
   alto, un favorito con split, tema claro y oscuro. Instala con
   `.worktrees/003-favorite-sessions/scripts/install-local.sh`.
4. Fusiona el 003 con `/factory approve 003-favorite-sessions` (o `merge.sh` si el
   approver aprueba), luego los PR #7 y #6, y vuelve a `main`.
5. Siguiente en el backlog: restaurar pestañas al abrir (job 3 del plan).

## Cierre: cuándo borrar este archivo

1. `factory/board.json` tiene `003-favorite-sessions` en `merged`.
2. `gh pr view 6 --json state -q .state` y `gh pr view 7 ...` devuelven `MERGED`.

Cuando ambos se cumplan, bórralo en su propio commit ("Remove the handoff, its steps
are done") por rama + PR, nunca directo a `main`.
