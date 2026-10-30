# Handoff: PC de casa → PC del trabajo (2026-10-29)

Para la próxima sesión, humano o Claude. Léelo entero antes de tocar nada. Bórralo
(en un commit, y súbelo) cuando todo esté hecho.

## Estado al salir

- Todo lo de la PC de casa está en GitHub. `long.txt` no se subió a propósito: es un
  archivo de prueba.
- **PR #2** (`factory-harness` → `main`), abierto y sin fusionar. Contiene:
  - el factory completo (`factory/`, `.claude/agents/`, `.claude/skills/factory/`);
  - la regla de subir todo (`CLAUDE.md`, `factory/conventions.md`, builder y
    `merge.sh`);
  - el plan `factory/plans/sessions-and-favorites.md` y sus 3 jobs en el backlog.
- `origin/main` está en `ba5d22f` ("Add a vtt config command…"), que vino de la PC del
  trabajo.
- La PC del trabajo tiene **cambios de UI grandes sin subir**: posibles renombres de
  archivos y clases. Por eso el plan habla de responsabilidades y no de archivos.
- El job **001-preview-selection** está en `needs-human`. Su rama
  `factory/001-preview-selection` está en GitHub y sale de un `main` viejo
  (`20d464b`, anterior a `ba5d22f`).

## Pasos, en orden

1. **Sube primero los cambios de la PC del trabajo**: commit en una rama, push y PR.
   Fusiónalos a `main` antes que lo demás. Todo lo siguiente debe partir de ahí.
2. Si allá existen `factory/`, `.claude/` o `CLAUDE.md` sin trackear, muévelos fuera
   del repo y compáralos después. Si no, git no deja hacer pull.
3. Fusiona el **PR #2** y haz `git pull --ff-only` en `main`. Si choca con los cambios
   de UI, el conflicto debería estar solo en `.gitignore`, porque el PR #2 no toca
   código Rust.
4. **Job 001**: lee `factory/jobs/001-preview-selection/decision.md`, sección "For
   the human". Lo que falta decidir:
   - las teclas por defecto dentro del preview (Ctrl+A, Ctrl+C, Esc), contra la regla
     de no inventar atajos;
   - la clave de tema nueva `selection_foreground`;
   - el cambio de color de la selección en la terminal;
   - el hueco del markdown renderizado;
   - los criterios manuales 5–9.

   Ojo:
   - La rama casi seguro choca con `ba5d22f` y con los cambios de UI (`input.rs`,
     selección y paneles). `merge.sh` la deja en `needs-human` si falla. Lo más
     probable es que necesite `/factory rework 001-preview-selection "rebase sobre
     main actual"` antes de `approve`.
   - Las rutas absolutas en `decision.md` son de la PC de casa.
   - El worktree `.worktrees/001-preview-selection` no existe allá. La skill lo
     recrea desde la rama (sección "Resuming across sessions").
5. **Backlog**: `/factory next` toma el primer item sin marcar. Arriba hay 3 items
   viejos. Si quieres empezar por favoritos, mueve las 3 últimas líneas al principio
   **sin cambiar su orden**: el job 1 (receta de lanzamiento + reset) va antes que
   favoritos y restaurar sesión. Si el job 1 no está fusionado, los otros dos se
   detienen solos con BLOCKED.

## A vigilar

- `factory/merge.sh` es nuevo (fetch + fast-forward a `origin/main`, push de `main`,
  borrar la rama remota). Solo se revisó con `bash -n`. Mira con atención la salida
  de su primera ejecución real.
- La regla de `CLAUDE.md`: todo cambio probado se sube sin preguntar, nunca
  force-push, y al empezar la sesión se hace `git fetch` + `pull --ff-only`.
- Decisiones del plan ya cerradas con el usuario. No las reabras:
  - el comando del favorito se teclea en la shell;
  - la restauración deja las pestañas dormidas;
  - el reset usa la versión nueva del favorito si todavía existe;
  - los favoritos son estado por máquina;
  - todo se configura por el CLI de vtt.
